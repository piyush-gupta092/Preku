//! The bridge server (Architecture.md §4.2): WebSocket over mutual TLS. Independent of Tauri, so
//! integration tests run a real server on localhost.
//!
//! Who may talk:
//! - a phone whose client-certificate fingerprint is paired → `system.hello` → `system.welcome`;
//! - an unknown certificate only while a pairing window is open, and only with the QR's
//!   one-time token (`pair.request`) → `pair.accepted` (its fingerprint is now pinned);
//! - anyone else gets `pair.rejected` and is disconnected.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, PoisonError,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::{mpsc, Semaphore},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use super::{
    devices::{self, PairedDevice},
    files::TransferDirection,
    identity::{fingerprint, Identity},
    pairing::{self, PairingWindow},
    texts::{self, SharedText, Texts},
    tls,
};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        storage::{now_millis, Storage},
    },
    protocol::{
        message_type as kind, ClipboardText, DeviceStatus, Envelope, Hello, PairAccepted,
        PairRejected, PairRequest, Pong, Welcome, PROTOCOL_VERSION,
    },
};

/// How long a new connection may stay silent before its first message.
const FIRST_MESSAGE: Duration = Duration::from_secs(10);
/// The phone pings every 15 s; three missed means it's gone.
const IDLE: Duration = Duration::from_secs(45);
/// At most this many connections at once (a LAN app; this only limits abuse).
const MAX_CONNECTIONS: usize = 8;

/// What the module (and the UI) hears from the hub.
#[derive(Debug, Clone, PartialEq)]
pub enum HubEvent {
    /// Devices, sessions or the pairing window changed.
    Changed,
    Paired(String),
    /// A file from the phone was saved (name, full path).
    FileReceived {
        name: String,
        path: std::path::PathBuf,
    },
    /// Text or a link arrived (the id in `Hub::texts`; the content stays out of events).
    TextReceived {
        id: String,
    },
}

pub(super) struct Session {
    serial: u64,
    pub(super) tx: mpsc::UnboundedSender<Message>,
    status: Option<DeviceStatus>,
}

pub struct Hub {
    pub(super) storage: Arc<Storage>,
    identity: Identity,
    pc_name: String,
    pub(super) pairing: Mutex<Option<PairingWindow>>,
    pub(super) sessions: Mutex<HashMap<String, Session>>,
    serial: AtomicU64,
    pub(super) notify: Box<dyn Fn(HubEvent) + Send + Sync>,
    /// Where received files go (`DownloadsPreku`).
    pub(super) inbox: std::path::PathBuf,
    pub(super) transfers: super::transfer::Transfers,
    /// The last few texts shared either way (memory only).
    pub(super) texts: Mutex<Texts>,
}

pub(super) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A device as the UI shows it.
#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub device: PairedDevice,
    pub online: bool,
    pub status: Option<DeviceStatus>,
}

impl Hub {
    pub fn new(
        storage: Arc<Storage>,
        identity: Identity,
        pc_name: String,
        inbox: std::path::PathBuf,
        notify: impl Fn(HubEvent) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            storage,
            identity,
            pc_name,
            pairing: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            serial: AtomicU64::new(0),
            notify: Box::new(notify),
            inbox,
            transfers: super::transfer::Transfers::default(),
            texts: Mutex::new(Texts::default()),
        })
    }

    pub fn texts(&self) -> Vec<SharedText> {
        lock(&self.texts).list()
    }

    pub fn text(&self, id: &str) -> Option<SharedText> {
        lock(&self.texts).get(id)
    }

    pub fn clear_texts(&self) {
        lock(&self.texts).clear();
        (self.notify)(HubEvent::Changed);
    }

    /// Sends text or a link to a connected phone.
    pub fn send_text(&self, device: &PairedDevice, text: &str) -> Result<SharedText> {
        let text = texts::validate(text)?;
        let sessions = lock(&self.sessions);
        let session = sessions.get(&device.id).ok_or_else(|| {
            PrekuError::InvalidInput("Your phone isn't connected right now".into())
        })?;
        session
            .tx
            .send(envelope(
                kind::CLIPBOARD_TEXT,
                ClipboardText { text: text.clone() },
            ))
            .map_err(|_| PrekuError::Bridge("the phone just disconnected".into()))?;
        drop(sessions);
        let entry =
            lock(&self.texts).push(text, TransferDirection::Send, &device.name, now_millis());
        (self.notify)(HubEvent::Changed);
        Ok(entry)
    }

    /// Text from a paired phone: remembered for the UI; the module copies it.
    fn on_text(&self, device: &PairedDevice, env: &Envelope) -> Result<()> {
        let message: ClipboardText = body(env)?;
        let text = texts::validate(&message.text)?;
        let entry =
            lock(&self.texts).push(text, TransferDirection::Receive, &device.name, now_millis());
        (self.notify)(HubEvent::TextReceived { id: entry.id });
        Ok(())
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn pc_name(&self) -> &str {
        &self.pc_name
    }

    /// Opens (or renews) the pairing window and returns it.
    pub fn open_pairing(&self) -> Result<PairingWindow> {
        let window = PairingWindow::open(now_millis())?;
        *lock(&self.pairing) = Some(window.clone());
        (self.notify)(HubEvent::Changed);
        Ok(window)
    }

    pub fn close_pairing(&self) {
        if lock(&self.pairing).take().is_some() {
            (self.notify)(HubEvent::Changed);
        }
    }

    /// The open pairing window, closing it first if it has expired.
    pub fn pairing(&self) -> Option<PairingWindow> {
        let mut window = lock(&self.pairing);
        if window.as_ref().is_some_and(|w| now_millis() > w.expires_at) {
            *window = None;
        }
        window.clone()
    }

    pub fn snapshot(&self) -> Result<Vec<DeviceSnapshot>> {
        let devices = self.storage.with_conn(|conn| devices::list(conn))?;
        let sessions = lock(&self.sessions);
        Ok(devices
            .into_iter()
            .map(|device| {
                let session = sessions.get(&device.id);
                DeviceSnapshot {
                    online: session.is_some(),
                    status: session.and_then(|s| s.status.clone()),
                    device,
                }
            })
            .collect())
    }

    /// Forgets a phone; tells it first if it's connected.
    pub fn unpair(&self, device_id: &str) -> Result<bool> {
        if let Some(session) = lock(&self.sessions).remove(device_id) {
            let _ = session
                .tx
                .send(envelope(kind::UNPAIR, serde_json::json!({})));
            let _ = session.tx.send(Message::Close(None));
        }
        let removed = self
            .storage
            .with_conn(|conn| devices::remove(conn, device_id))?;
        (self.notify)(HubEvent::Changed);
        Ok(removed)
    }

    pub fn has_devices(&self) -> bool {
        self.storage
            .with_conn(|conn| devices::list(conn))
            .is_ok_and(|d| !d.is_empty())
    }
}

/// Starts listening; returns the bound address and the accept task.
pub async fn serve(
    hub: Arc<Hub>,
    addr: SocketAddr,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let acceptor = TlsAcceptor::from(tls::server_config(&hub.identity)?);
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let task = tokio::spawn(async move {
        loop {
            let Ok((tcp, peer)) = listener.accept().await else {
                continue;
            };
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                tracing::warn!(%peer, "bridge: too many connections, refusing");
                continue;
            };
            let (hub, acceptor) = (hub.clone(), acceptor.clone());
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(err) = handle(hub, acceptor, tcp).await {
                    tracing::debug!(%peer, error = %err, "bridge connection ended");
                }
            });
        }
    });
    Ok((local, task))
}

pub(super) fn envelope(kind: &str, body: impl Serialize) -> Message {
    let body = match serde_json::to_value(body) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let env = Envelope {
        v: PROTOCOL_VERSION,
        id: uuid::Uuid::new_v4().to_string(),
        r#type: kind.to_owned(),
        ts: now_millis(),
        body,
    };
    Message::text(serde_json::to_string(&env).unwrap_or_default())
}

pub(super) fn body<T: serde::de::DeserializeOwned>(env: &Envelope) -> Result<T> {
    Ok(serde_json::from_value(serde_json::Value::Object(
        env.body.clone(),
    ))?)
}

/// The next protocol message (skipping pings/pongs at the WebSocket level).
async fn next<S>(stream: &mut S) -> Result<Option<Envelope>>
where
    S: StreamExt<Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    while let Some(message) = stream.next().await {
        match message.map_err(|e| PrekuError::Bridge(e.to_string()))? {
            Message::Text(text) => return Ok(Some(serde_json::from_str(text.as_str())?)),
            Message::Close(_) => return Ok(None),
            _ => {}
        }
    }
    Ok(None)
}

async fn reject<T: AsyncRead + AsyncWrite + Unpin>(
    mut ws: WebSocketStream<T>,
    reason: &str,
) -> Result<()> {
    let _ = ws
        .send(envelope(
            kind::PAIR_REJECTED,
            PairRejected {
                reason: reason.to_owned(),
            },
        ))
        .await;
    let _ = ws.close(None).await;
    Ok(())
}

async fn handle(hub: Arc<Hub>, acceptor: TlsAcceptor, tcp: tokio::net::TcpStream) -> Result<()> {
    let tls = timeout(FIRST_MESSAGE, acceptor.accept(tcp))
        .await
        .map_err(|_| PrekuError::Bridge("TLS handshake timed out".into()))??;
    let client_fp = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|cert| fingerprint(cert))
        .ok_or_else(|| PrekuError::Bridge("no client certificate".into()))?;
    let mut ws = tokio_tungstenite::accept_async(tls)
        .await
        .map_err(|e| PrekuError::Bridge(e.to_string()))?;

    let Ok(first) = timeout(FIRST_MESSAGE, next(&mut ws)).await else {
        return reject(ws, "timeout").await;
    };
    let Some(first) = first? else { return Ok(()) };

    let known = hub
        .storage
        .with_conn(|conn| devices::find_by_fp(conn, &client_fp))?;
    let device = if first.r#type == kind::PAIR_REQUEST {
        let request: PairRequest = body(&first)?;
        let verdict = pairing::redeem(&mut lock(&hub.pairing), &request.token, now_millis());
        if let Err(rejection) = verdict {
            tracing::info!(reason = rejection.reason(), "bridge: pairing refused");
            (hub.notify)(HubEvent::Changed);
            return reject(ws, rejection.reason()).await;
        }
        let device = hub
            .storage
            .with_conn(|conn| devices::add(conn, &request.device_name, &client_fp, now_millis()))?;
        ws.send(envelope(
            kind::PAIR_ACCEPTED,
            PairAccepted {
                pc_id: hub.identity.pc_id.clone(),
                pc_name: hub.pc_name.clone(),
            },
        ))
        .await
        .map_err(|e| PrekuError::Bridge(e.to_string()))?;
        (hub.notify)(HubEvent::Paired(device.name.clone()));
        device
    } else if let (Some(device), true) = (known, first.r#type == kind::HELLO) {
        let _hello: Hello = body(&first)?;
        ws.send(envelope(
            kind::WELCOME,
            Welcome {
                pc_id: hub.identity.pc_id.clone(),
                pc_name: hub.pc_name.clone(),
            },
        ))
        .await
        .map_err(|e| PrekuError::Bridge(e.to_string()))?;
        device
    } else {
        // An unknown certificate outside pairing, or a known one skipping the hello.
        return reject(ws, "unknown_device").await;
    };

    run_session(hub, device, ws).await
}

pub(super) async fn run_session<T: AsyncRead + AsyncWrite + Unpin>(
    hub: Arc<Hub>,
    device: PairedDevice,
    ws: WebSocketStream<T>,
) -> Result<()> {
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let own_tx = tx.clone();
    let serial = hub.serial.fetch_add(1, Ordering::SeqCst);
    // A reconnect replaces an older session of the same phone.
    if let Some(old) = lock(&hub.sessions).insert(
        device.id.clone(),
        Session {
            serial,
            tx,
            status: None,
        },
    ) {
        let _ = old.tx.send(Message::Close(None));
    }
    let _ = hub
        .storage
        .with_conn(|conn| devices::touch(conn, &device.id, now_millis()));
    (hub.notify)(HubEvent::Changed);

    let result: Result<()> = async {
        loop {
            tokio::select! {
                outgoing = rx.recv() => match outgoing {
                    Some(message) => {
                        let closing = matches!(message, Message::Close(_));
                        sink.send(message).await.map_err(|e| PrekuError::Bridge(e.to_string()))?;
                        if closing { return Ok(()); }
                    }
                    None => return Ok(()),
                },
                incoming = timeout(IDLE, stream.next()) => {
                    let Ok(incoming) = incoming else {
                        return Err(PrekuError::Bridge("phone went quiet".into()));
                    };
                    let Some(message) = incoming else { return Ok(()) };
                    let text = match message.map_err(|e| PrekuError::Bridge(e.to_string()))? {
                        Message::Text(text) => text,
                        Message::Close(_) => return Ok(()),
                        _ => continue, // WebSocket pings keep the session alive.
                    };
                    let env: Envelope = serde_json::from_str(text.as_str())?;
                    match env.r#type.as_str() {
                        t if t == kind::DEVICE_STATUS => {
                            let status: DeviceStatus = body(&env)?;
                            if let Some(session) = lock(&hub.sessions).get_mut(&device.id) {
                                session.status = Some(status);
                            }
                            (hub.notify)(HubEvent::Changed);
                        }
                        "system.ping" => {
                            sink.send(envelope(kind::PONG, Pong { reply_to: env.id.clone() }))
                                .await
                                .map_err(|e| PrekuError::Bridge(e.to_string()))?;
                        }
                        t if t == kind::UNPAIR => {
                            hub.storage.with_conn(|conn| devices::remove(conn, &device.id))?;
                            let _ = sink.send(Message::Close(None)).await;
                            return Ok(());
                        }
                        t if t == kind::CLIPBOARD_TEXT => {
                            // Never log the content (Rules.md §4).
                            if hub.on_text(&device, &env).is_err() {
                                tracing::debug!("bridge: ignored a text that was empty or too long");
                            }
                        }
                        t if t.starts_with("file.") => {
                            if let Err(err) = hub.on_file_message(&device, &env, &own_tx).await {
                                tracing::warn!(error = %err, "bridge: file transfer problem");
                            }
                        }
                        other => tracing::debug!(kind = other, "bridge: ignoring message"),
                    }
                }
            }
        }
    }
    .await;

    {
        let mut sessions = lock(&hub.sessions);
        if sessions.get(&device.id).is_some_and(|s| s.serial == serial) {
            sessions.remove(&device.id);
        }
    }
    hub.transfers_disconnected(&device.id).await;
    let _ = hub
        .storage
        .with_conn(|conn| devices::touch(conn, &device.id, now_millis()));
    (hub.notify)(HubEvent::Changed);
    result
}
