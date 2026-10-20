//! The file-transfer engine on the PC (see `files.rs` for the protocol and the pure helpers).

use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc::UnboundedSender, Notify},
};
use tokio_tungstenite::tungstenite::Message;

use super::{
    devices::PairedDevice,
    files::{
        chunk_count, hex, sanitize_name, unique_path, TransferDirection, TransferStatus,
        TransferView, CHUNK, HISTORY, MAX_SIZE, WINDOW,
    },
    server::{body, envelope, lock, Hub, HubEvent},
};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        storage::now_millis,
    },
    protocol::{
        message_type as kind, Envelope, FileAccept, FileAck, FileCancel, FileChunk, FileDecline,
        FileDone, FileOffer, FileResult,
    },
};

/// How long a sender waits for an ack before giving up.
const ACK_TIMEOUT: Duration = Duration::from_secs(30);
/// Progress events at most this often per transfer (status changes always go out).
const PROGRESS_EVERY_MS: i64 = 200;

struct Incoming {
    device_id: String,
    file: tokio::fs::File,
    part: PathBuf,
    name: String,
    size: i64,
    sha256: String,
    hasher: Sha256,
    next: u32,
    written: i64,
}

struct Outgoing {
    device_id: String,
    path: PathBuf,
    size: i64,
    tx: UnboundedSender<Message>,
    acked: Arc<AtomicU32>,
    wake: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Transfers {
    incoming: tokio::sync::Mutex<HashMap<String, Incoming>>,
    outgoing: Mutex<HashMap<String, Outgoing>>,
    views: Mutex<VecDeque<TransferView>>,
    last_emit: Mutex<HashMap<String, i64>>,
}

fn send<T: serde::Serialize>(tx: &UnboundedSender<Message>, kind: &str, body: T) {
    let _ = tx.send(envelope(kind, body));
}

impl Hub {
    /// Received files go here (`Downloads\Preku`).
    pub fn inbox(&self) -> &Path {
        &self.inbox
    }

    /// Current and recent transfers, newest first.
    pub fn transfers(&self) -> Vec<TransferView> {
        lock(&self.transfers.views).iter().cloned().collect()
    }

    /// A finished received file's path, for "show in folder".
    pub fn received_path(&self, id: &str) -> Option<PathBuf> {
        lock(&self.transfers.views)
            .iter()
            .find(|v| {
                v.id == id
                    && v.direction == TransferDirection::Receive
                    && v.status == TransferStatus::Done
            })
            .map(|v| self.inbox.join(&v.name))
            .filter(|p| p.exists())
    }

    fn upsert_view(&self, view: TransferView) {
        let mut views = lock(&self.transfers.views);
        views.retain(|v| v.id != view.id);
        views.push_front(view);
        // Keep running ones; trim old finished ones.
        while views.len() > HISTORY {
            match views
                .iter()
                .rposition(|v| matches!(v.status, TransferStatus::Done | TransferStatus::Failed))
            {
                Some(i) => {
                    views.remove(i);
                }
                None => break,
            }
        }
        drop(views);
        (self.notify)(HubEvent::Changed);
    }

    fn update_view(&self, id: &str, change: impl FnOnce(&mut TransferView)) {
        let now = now_millis();
        let (status_changed, exists) = {
            let mut views = lock(&self.transfers.views);
            match views.iter_mut().find(|v| v.id == id) {
                Some(view) => {
                    let before = view.status;
                    change(view);
                    view.updated_at = now;
                    (view.status != before, true)
                }
                None => (false, false),
            }
        };
        if !exists {
            return;
        }
        let mut last = lock(&self.transfers.last_emit);
        let due = now - last.get(id).copied().unwrap_or(0) >= PROGRESS_EVERY_MS;
        if status_changed || due {
            last.insert(id.to_owned(), now);
            drop(last);
            (self.notify)(HubEvent::Changed);
        }
    }

    fn fail(&self, id: &str, reason: &str) {
        self.update_view(id, |v| {
            v.status = TransferStatus::Failed;
            v.error = Some(reason.to_owned());
        });
    }

    /// Sends a file to a connected phone. Returns the transfer id.
    pub async fn send_file(self: &Arc<Self>, device: &PairedDevice, path: &Path) -> Result<String> {
        let meta = tokio::fs::metadata(path).await?;
        if !meta.is_file() {
            return Err(PrekuError::InvalidInput(
                "Only files can be sent, not folders".into(),
            ));
        }
        let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
        if size > MAX_SIZE {
            return Err(PrekuError::InvalidInput(
                "That file is too large to send".into(),
            ));
        }
        let tx = lock(&self.sessions)
            .get(&device.id)
            .map(|s| s.tx.clone())
            .ok_or_else(|| {
                PrekuError::InvalidInput("Your phone isn't connected right now".into())
            })?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let id = uuid::Uuid::new_v4().to_string();
        self.upsert_view(TransferView {
            id: id.clone(),
            name: name.clone(),
            size,
            bytes: 0,
            direction: TransferDirection::Send,
            status: TransferStatus::Waiting,
            device_name: device.name.clone(),
            error: None,
            updated_at: now_millis(),
        });

        // Hash first (the receiver checks it), off the async threads.
        let hash_path = path.to_owned();
        let sha256 = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let mut file = std::fs::File::open(hash_path)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            Ok(hex(&hasher.finalize()))
        })
        .await
        .map_err(|e| PrekuError::Bridge(e.to_string()))??;

        lock(&self.transfers.outgoing).insert(
            id.clone(),
            Outgoing {
                device_id: device.id.clone(),
                path: path.to_owned(),
                size,
                tx: tx.clone(),
                acked: Arc::new(AtomicU32::new(0)),
                wake: Arc::new(Notify::new()),
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
        send(
            &tx,
            kind::FILE_OFFER,
            FileOffer {
                transfer_id: id.clone(),
                name,
                size,
                mime: None,
                sha256,
            },
        );
        Ok(id)
    }

    /// Stops a transfer from this side.
    pub async fn cancel_transfer(&self, id: &str) {
        if let Some(out) = lock(&self.transfers.outgoing).remove(id) {
            out.cancelled.store(true, Ordering::SeqCst);
            out.wake.notify_one();
            send(
                &out.tx,
                kind::FILE_CANCEL,
                FileCancel {
                    transfer_id: id.to_owned(),
                },
            );
        }
        let incoming = self.transfers.incoming.lock().await.remove(id);
        if let Some(inc) = incoming {
            drop(inc.file);
            let _ = tokio::fs::remove_file(&inc.part).await;
            if let Some(tx) = lock(&self.sessions)
                .get(&inc.device_id)
                .map(|s| s.tx.clone())
            {
                send(
                    &tx,
                    kind::FILE_CANCEL,
                    FileCancel {
                        transfer_id: id.to_owned(),
                    },
                );
            }
        }
        self.fail(id, "cancelled");
    }

    /// The phone disconnected: everything in flight with it fails (and partial files go away).
    pub(super) async fn transfers_disconnected(&self, device_id: &str) {
        let dropped: Vec<String> = {
            let mut out = lock(&self.transfers.outgoing);
            let ids: Vec<String> = out
                .iter()
                .filter(|(_, o)| o.device_id == device_id)
                .map(|(id, _)| id.clone())
                .collect();
            for id in &ids {
                if let Some(o) = out.remove(id) {
                    o.cancelled.store(true, Ordering::SeqCst);
                    o.wake.notify_one();
                }
            }
            ids
        };
        let mut incoming = self.transfers.incoming.lock().await;
        let ids: Vec<String> = incoming
            .iter()
            .filter(|(_, i)| i.device_id == device_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            if let Some(inc) = incoming.remove(id) {
                drop(inc.file);
                let _ = tokio::fs::remove_file(&inc.part).await;
            }
        }
        drop(incoming);
        for id in dropped.iter().chain(&ids) {
            self.fail(id, "disconnected");
        }
    }

    /// Handles one `file.*` message from a phone.
    pub(super) async fn on_file_message(
        self: &Arc<Self>,
        device: &PairedDevice,
        env: &Envelope,
        tx: &UnboundedSender<Message>,
    ) -> Result<()> {
        match env.r#type.as_str() {
            t if t == kind::FILE_OFFER => self.on_offer(device, body(env)?, tx).await,
            t if t == kind::FILE_CHUNK => self.on_chunk(body(env)?, tx).await,
            t if t == kind::FILE_DONE => self.on_done(device, body::<FileDone>(env)?, tx).await,
            t if t == kind::FILE_ACCEPT => {
                let accept: FileAccept = body(env)?;
                self.start_pump(&accept.transfer_id);
                Ok(())
            }
            t if t == kind::FILE_ACK => {
                let ack: FileAck = body(env)?;
                let progress = lock(&self.transfers.outgoing)
                    .get(&ack.transfer_id)
                    .map(|o| {
                        o.acked.fetch_max(ack.index + 1, Ordering::SeqCst);
                        o.wake.notify_one();
                        ((i64::from(ack.index) + 1) * CHUNK as i64).min(o.size)
                    });
                if let Some(bytes) = progress {
                    self.update_view(&ack.transfer_id, |v| v.bytes = bytes);
                }
                Ok(())
            }
            t if t == kind::FILE_RESULT => {
                let result: FileResult = body(env)?;
                lock(&self.transfers.outgoing).remove(&result.transfer_id);
                if result.ok {
                    self.update_view(&result.transfer_id, |v| {
                        v.status = TransferStatus::Done;
                        v.bytes = v.size;
                    });
                } else {
                    self.fail(
                        &result.transfer_id,
                        result.reason.as_deref().unwrap_or("error"),
                    );
                }
                Ok(())
            }
            t if t == kind::FILE_DECLINE => {
                let decline: FileDecline = body(env)?;
                if let Some(o) = lock(&self.transfers.outgoing).remove(&decline.transfer_id) {
                    o.cancelled.store(true, Ordering::SeqCst);
                }
                self.fail(&decline.transfer_id, &decline.reason);
                Ok(())
            }
            t if t == kind::FILE_CANCEL => {
                let cancel: FileCancel = body(env)?;
                if let Some(o) = lock(&self.transfers.outgoing).remove(&cancel.transfer_id) {
                    o.cancelled.store(true, Ordering::SeqCst);
                    o.wake.notify_one();
                }
                let incoming = self
                    .transfers
                    .incoming
                    .lock()
                    .await
                    .remove(&cancel.transfer_id);
                if let Some(inc) = incoming {
                    drop(inc.file);
                    let _ = tokio::fs::remove_file(&inc.part).await;
                }
                self.fail(&cancel.transfer_id, "cancelled");
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn on_offer(
        &self,
        device: &PairedDevice,
        offer: FileOffer,
        tx: &UnboundedSender<Message>,
    ) -> Result<()> {
        let decline = |reason: &str| {
            send(
                tx,
                kind::FILE_DECLINE,
                FileDecline {
                    transfer_id: offer.transfer_id.clone(),
                    reason: reason.to_owned(),
                },
            )
        };
        if offer.size < 0 || offer.size > MAX_SIZE {
            decline("too_large");
            return Ok(());
        }
        let name = sanitize_name(&offer.name);
        let opened = async {
            tokio::fs::create_dir_all(&self.inbox).await?;
            let part = self.inbox.join(format!("{}.part", offer.transfer_id));
            let file = tokio::fs::File::create(&part).await?;
            Ok::<_, std::io::Error>((part, file))
        }
        .await;
        let Ok((part, file)) = opened else {
            decline("error");
            return Ok(());
        };
        self.transfers.incoming.lock().await.insert(
            offer.transfer_id.clone(),
            Incoming {
                device_id: device.id.clone(),
                file,
                part,
                name: name.clone(),
                size: offer.size,
                sha256: offer.sha256.to_ascii_lowercase(),
                hasher: Sha256::new(),
                next: 0,
                written: 0,
            },
        );
        self.upsert_view(TransferView {
            id: offer.transfer_id.clone(),
            name,
            size: offer.size,
            bytes: 0,
            direction: TransferDirection::Receive,
            status: TransferStatus::Active,
            device_name: device.name.clone(),
            error: None,
            updated_at: now_millis(),
        });
        send(
            tx,
            kind::FILE_ACCEPT,
            FileAccept {
                transfer_id: offer.transfer_id,
            },
        );
        Ok(())
    }

    async fn on_chunk(&self, chunk: FileChunk, tx: &UnboundedSender<Message>) -> Result<()> {
        let mut incoming = self.transfers.incoming.lock().await;
        let Some(inc) = incoming.get_mut(&chunk.transfer_id) else {
            return Ok(());
        };
        let data = base64::engine::general_purpose::STANDARD.decode(chunk.data.as_bytes());
        let problem = match &data {
            Err(_) => Some("error"),
            Ok(_) if chunk.index != inc.next => Some("order"),
            Ok(bytes) if inc.written + bytes.len() as i64 > inc.size => Some("size"),
            Ok(_) => None,
        };
        if let Some(reason) = problem {
            let inc = incoming.remove(&chunk.transfer_id).expect("present");
            drop(incoming);
            drop(inc.file);
            let _ = tokio::fs::remove_file(&inc.part).await;
            send(
                tx,
                kind::FILE_RESULT,
                FileResult {
                    transfer_id: chunk.transfer_id.clone(),
                    ok: false,
                    reason: Some(reason.into()),
                },
            );
            self.fail(&chunk.transfer_id, reason);
            return Ok(());
        }
        let bytes = data.unwrap_or_default();
        if let Err(err) = inc.file.write_all(&bytes).await {
            tracing::warn!(error = %err, "bridge: writing a received file failed");
            let inc = incoming.remove(&chunk.transfer_id).expect("present");
            drop(incoming);
            drop(inc.file);
            let _ = tokio::fs::remove_file(&inc.part).await;
            send(
                tx,
                kind::FILE_RESULT,
                FileResult {
                    transfer_id: chunk.transfer_id.clone(),
                    ok: false,
                    reason: Some("write".into()),
                },
            );
            self.fail(&chunk.transfer_id, "write");
            return Ok(());
        }
        inc.hasher.update(&bytes);
        inc.written += bytes.len() as i64;
        inc.next += 1;
        let written = inc.written;
        drop(incoming);
        send(
            tx,
            kind::FILE_ACK,
            FileAck {
                transfer_id: chunk.transfer_id.clone(),
                index: chunk.index,
            },
        );
        self.update_view(&chunk.transfer_id, |v| v.bytes = written);
        Ok(())
    }

    async fn on_done(
        &self,
        _device: &PairedDevice,
        done: FileDone,
        tx: &UnboundedSender<Message>,
    ) -> Result<()> {
        let Some(mut inc) = self
            .transfers
            .incoming
            .lock()
            .await
            .remove(&done.transfer_id)
        else {
            return Ok(());
        };
        let flushed = inc.file.flush().await.and(inc.file.sync_all().await);
        drop(inc.file);
        let hash = hex(&inc.hasher.finalize());
        let reason = if flushed.is_err() {
            Some("write")
        } else if inc.written != inc.size {
            Some("size")
        } else if hash != inc.sha256 {
            Some("checksum")
        } else {
            None
        };
        let saved = match reason {
            Some(_) => None,
            None => {
                let target = unique_path(&self.inbox, &inc.name);
                tokio::fs::rename(&inc.part, &target)
                    .await
                    .ok()
                    .map(|()| target)
            }
        };
        match saved {
            Some(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                send(
                    tx,
                    kind::FILE_RESULT,
                    FileResult {
                        transfer_id: done.transfer_id.clone(),
                        ok: true,
                        reason: None,
                    },
                );
                self.update_view(&done.transfer_id, |v| {
                    v.status = TransferStatus::Done;
                    v.bytes = v.size;
                    v.name = name.clone();
                });
                (self.notify)(HubEvent::FileReceived { name, path });
            }
            None => {
                let _ = tokio::fs::remove_file(&inc.part).await;
                let reason = reason.unwrap_or("write");
                send(
                    tx,
                    kind::FILE_RESULT,
                    FileResult {
                        transfer_id: done.transfer_id.clone(),
                        ok: false,
                        reason: Some(reason.into()),
                    },
                );
                self.fail(&done.transfer_id, reason);
            }
        }
        Ok(())
    }

    /// After `file.accept`: stream the chunks, keeping at most WINDOW unacknowledged.
    fn start_pump(self: &Arc<Self>, id: &str) {
        let Some((path, size, tx, acked, wake, cancelled)) =
            lock(&self.transfers.outgoing).get(id).map(|o| {
                (
                    o.path.clone(),
                    o.size,
                    o.tx.clone(),
                    o.acked.clone(),
                    o.wake.clone(),
                    o.cancelled.clone(),
                )
            })
        else {
            return;
        };
        self.update_view(id, |v| v.status = TransferStatus::Active);
        let (hub, id) = (self.clone(), id.to_owned());
        tokio::spawn(async move {
            let pumped: Result<()> = async {
                let mut file = tokio::fs::File::open(&path).await?;
                let mut buf = vec![0u8; CHUNK];
                for index in 0..chunk_count(size) {
                    while index >= acked.load(Ordering::SeqCst) + WINDOW {
                        if cancelled.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        if tokio::time::timeout(ACK_TIMEOUT, wake.notified())
                            .await
                            .is_err()
                        {
                            return Err(PrekuError::Bridge("the phone stopped answering".into()));
                        }
                    }
                    if cancelled.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    let mut n = 0;
                    while n < CHUNK {
                        let read = file.read(&mut buf[n..]).await?;
                        if read == 0 {
                            break;
                        }
                        n += read;
                    }
                    send(
                        &tx,
                        kind::FILE_CHUNK,
                        FileChunk {
                            transfer_id: id.clone(),
                            index,
                            data: base64::engine::general_purpose::STANDARD.encode(&buf[..n]),
                        },
                    );
                }
                send(
                    &tx,
                    kind::FILE_DONE,
                    FileDone {
                        transfer_id: id.clone(),
                    },
                );
                Ok(())
            }
            .await;
            if let Err(err) = pumped {
                tracing::warn!(error = %err, "bridge: sending a file failed");
                if let Some(o) = lock(&hub.transfers.outgoing).remove(&id) {
                    send(
                        &o.tx,
                        kind::FILE_CANCEL,
                        FileCancel {
                            transfer_id: id.clone(),
                        },
                    );
                }
                hub.fail(&id, "error");
            }
        });
    }
}
