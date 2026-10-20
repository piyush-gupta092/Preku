//! Phone bridge (Phase 6, PRD B1/B5): pairing by QR, mutual-TLS WebSocket, mDNS discovery and
//! the phone's status on the PC. See Architecture.md §4.
//!
//! Privacy and exposure: nothing listens on the network until the user opens a pairing window
//! or a phone is already paired; unpairing the last phone stops the server again.

pub mod devices;
pub mod discovery;
pub mod files;
pub mod identity;
pub mod pairing;
pub mod server;
pub mod texts;
pub mod tls;
mod transfer;
pub mod web;

use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use ts_rs::TS;

use self::{
    discovery::Advertiser,
    files::{TransferStatus, TransferView},
    identity::Identity,
    server::{Hub, HubEvent},
};
use super::{Module, ModuleContext};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::AppEvent,
    },
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

pub const ID: &str = "bridge";
/// Preferred port (stable, so a phone's saved address keeps working); falls back to any.
pub const PORT: u16 = 47823;

pub mod events {
    pub const STATE: &str = "bridge:state";
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PhoneStatus {
    pub battery: u32,
    pub charging: bool,
    /// wifi | cellular | ethernet | none
    pub network: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PhoneView {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub paired_at: i64,
    #[ts(type = "number | null")]
    pub last_seen: Option<i64>,
    pub online: bool,
    pub status: Option<PhoneStatus>,
    /// `app` (the Android app) or `browser` (the web companion, e.g. an iPhone).
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PairingView {
    /// `preku://pair?...` (also shown as text, for pasting into the phone).
    pub link: String,
    /// The same link as an SVG QR code.
    pub qr_svg: String,
    pub address: String,
    /// `https://<pc>:47824/#pair=<token>`: the web companion, for iPhone and other browsers.
    pub web_link: Option<String>,
    pub web_qr_svg: Option<String>,
    #[ts(type = "number")]
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BridgeState {
    pub pc_name: String,
    pub phones: Vec<PhoneView>,
    pub pairing: Option<PairingView>,
    /// The server is up (only while pairing or with a paired phone).
    pub listening: bool,
    pub error: Option<String>,
    /// File transfers, newest first (running ones and the last few finished).
    pub transfers: Vec<TransferView>,
    /// Where received files are saved.
    pub inbox: String,
    /// Text and links shared either way, newest first (kept in memory only).
    pub texts: Vec<texts::SharedText>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Net {
    server: Option<(SocketAddr, tokio::task::JoinHandle<()>)>,
    /// The browser companion (`web.rs`), started and stopped with `server`.
    web: Option<(SocketAddr, tokio::task::JoinHandle<()>)>,
    mdns: Option<Advertiser>,
    pairing: Option<PairingView>,
    expiry: Option<JoinHandle<()>>,
}

struct Inner {
    ctx: ModuleContext,
    /// Live items currently shown for transfers.
    live: Mutex<std::collections::HashSet<String>>,
    hub: Arc<Hub>,
    net: tokio::sync::Mutex<Net>,
    error: Mutex<Option<String>>,
}

#[derive(Default)]
pub struct BridgeModule {
    running: Mutex<Option<(Arc<Inner>, JoinHandle<()>)>>,
}

impl Module for BridgeModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let identity = Identity::load_or_create(&ctx.storage)?;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HubEvent>();
        let hub = Hub::new(
            ctx.storage.clone(),
            identity,
            discovery::pc_name(),
            inbox_dir(),
            move |event| {
                let _ = tx.send(event);
            },
        );
        let inner = Arc::new(Inner {
            ctx,
            live: Mutex::new(std::collections::HashSet::new()),
            hub,
            net: tokio::sync::Mutex::new(Net::default()),
            error: Mutex::new(None),
        });
        let events = {
            let inner = inner.clone();
            tauri::async_runtime::spawn(async move {
                if inner.hub.has_devices() {
                    inner.ensure_listening().await;
                }
                inner.emit_state().await;
                while let Some(event) = rx.recv().await {
                    if let HubEvent::Paired(name) = &event {
                        tracing::info!(phone = %name, "bridge: phone paired");
                        inner.ctx.publish(AppEvent::Pip(PipSignal::Celebration));
                        // The QR has done its job.
                        inner.net.lock().await.pairing = None;
                    }
                    if let HubEvent::FileReceived { name, .. } = &event {
                        tracing::debug!(file = %name, "bridge: file received");
                        inner.ctx.publish(AppEvent::Pip(PipSignal::Success));
                    }
                    if let HubEvent::TextReceived { id } = &event {
                        inner.text_arrived(id).await;
                    }
                    inner.sync_live_items();
                    inner.stop_if_idle().await;
                    inner.emit_state().await;
                }
            })
        };
        *lock(&self.running) = Some((inner, events));
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some((inner, events)) = lock(&self.running).take() {
            events.abort();
            tauri::async_runtime::spawn(async move {
                let mut net = inner.net.lock().await;
                shutdown(&mut net);
            });
        }
        Ok(())
    }
}

fn shutdown(net: &mut Net) {
    if let Some((_, task)) = net.server.take() {
        task.abort();
    }
    if let Some((_, task)) = net.web.take() {
        task.abort();
    }
    if let Some(mdns) = net.mdns.take() {
        mdns.stop();
    }
    if let Some(expiry) = net.expiry.take() {
        expiry.abort();
    }
    net.pairing = None;
}

/// `DownloadsPreku` (or `~/Downloads/Preku`).
fn inbox_dir() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap_or_default();
    std::path::PathBuf::from(home)
        .join("Downloads")
        .join("Preku")
}

impl Inner {
    /// Transfers in the notch: a progress ring while running, a short note when finished.
    fn sync_live_items(&self) {
        let transfers = self.hub.transfers();
        let mut shown = lock(&self.live);
        let mut keep = std::collections::HashSet::new();
        for t in &transfers {
            let id = format!("bridge.transfer.{}", t.id);
            let up = t.direction == files::TransferDirection::Send;
            let item = match t.status {
                TransferStatus::Waiting | TransferStatus::Active => {
                    let mut item = LiveItem::new(
                        &id,
                        ID,
                        LivePriority::Action,
                        t.name.clone(),
                        Some(if up { "↑".into() } else { "↓".into() }),
                    );
                    item.progress = Some(if t.size > 0 {
                        (t.bytes as f32 / t.size as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    });
                    item
                }
                // Finished recently: say so for a few seconds.
                TransferStatus::Done | TransferStatus::Failed
                    if now_ms() - t.updated_at < 10_000 =>
                {
                    let mark = if t.status == TransferStatus::Done {
                        if up {
                            "✓"
                        } else {
                            "📥"
                        }
                    } else {
                        "⚠"
                    };
                    LiveItem::new(
                        &id,
                        ID,
                        LivePriority::Info,
                        format!("{mark} {}", t.name),
                        None,
                    )
                    .expires_in(8_000)
                }
                _ => continue,
            };
            keep.insert(id.clone());
            self.ctx.publish(AppEvent::LiveUpsert(item));
        }
        for gone in shown.difference(&keep) {
            self.ctx.publish(AppEvent::LiveRemove { id: gone.clone() });
        }
        *shown = keep;
    }

    /// Text from the phone goes straight onto this PC's clipboard (that is why it was sent)
    /// and the notch says so. Links are only copied, never opened.
    async fn text_arrived(&self, id: &str) {
        let Some(entry) = self.hub.text(id) else {
            return;
        };
        let text = entry.text.clone();
        let copied = tauri::async_runtime::spawn_blocking(move || {
            arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text))
        })
        .await;
        if !matches!(copied, Ok(Ok(()))) {
            tracing::warn!("bridge: couldn't copy the received text");
        }
        self.ctx.publish(AppEvent::LiveUpsert(
            LiveItem::new(
                format!("bridge.text.{id}"),
                ID,
                LivePriority::Info,
                format!("📋 {}", entry.device_name),
                None,
            )
            .with_kind("text")
            .expires_in(8_000),
        ));
        self.ctx.publish(AppEvent::Pip(PipSignal::Success));
    }

    async fn ensure_listening(&self) {
        let mut net = self.net.lock().await;
        if net.server.is_some() {
            return;
        }
        let preferred = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT));
        let any = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let started = match server::serve(self.hub.clone(), preferred).await {
            Ok(ok) => Ok(ok),
            Err(_) => server::serve(self.hub.clone(), any).await,
        };
        match started {
            Ok((addr, task)) => {
                tracing::info!(port = addr.port(), "bridge: listening");
                let identity = self.hub.identity();
                match Advertiser::start(&identity.pc_id, self.hub.pc_name(), addr.port()) {
                    Ok(mdns) => net.mdns = Some(mdns),
                    // Discovery is a convenience: the QR address still works without it.
                    Err(err) => tracing::warn!(error = %err, "bridge: mDNS unavailable"),
                }
                net.server = Some((addr, task));
                *lock(&self.error) = None;
                let web = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, web::WEB_PORT));
                let started = match web::serve_web(self.hub.clone(), web).await {
                    Ok(ok) => Ok(ok),
                    Err(_) => web::serve_web(self.hub.clone(), any).await,
                };
                match started {
                    Ok(ok) => net.web = Some(ok),
                    // The Android app still works; only the iPhone page is missing.
                    Err(err) => tracing::warn!(error = %err, "bridge: web companion unavailable"),
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "bridge: could not listen");
                *lock(&self.error) = Some("Couldn't open the network port for the phone.".into());
            }
        }
    }

    /// Stops listening when there's nobody to talk to (no phone, no pairing window).
    async fn stop_if_idle(&self) {
        if self.hub.pairing().is_none() && !self.hub.has_devices() {
            let mut net = self.net.lock().await;
            if net.server.is_some() {
                tracing::info!("bridge: no phones, stopped listening");
            }
            shutdown(&mut net);
        }
    }

    async fn state(&self) -> Result<BridgeState> {
        let phones = self
            .hub
            .snapshot()?
            .into_iter()
            .map(|s| PhoneView {
                kind: if s.device.cert_fp.starts_with(web::WEB_PREFIX) {
                    "browser"
                } else {
                    "app"
                }
                .into(),
                id: s.device.id,
                name: s.device.name,
                paired_at: s.device.paired_at,
                last_seen: s.device.last_seen,
                online: s.online,
                status: s.status.map(|st| PhoneStatus {
                    battery: st.battery.min(100),
                    charging: st.charging,
                    network: st.network,
                }),
            })
            .collect();
        let net = self.net.lock().await;
        Ok(BridgeState {
            pc_name: self.hub.pc_name().to_owned(),
            phones,
            pairing: net.pairing.clone().filter(|_| self.hub.pairing().is_some()),
            listening: net.server.is_some(),
            error: lock(&self.error).clone(),
            transfers: self.hub.transfers(),
            inbox: self.hub.inbox().display().to_string(),
            texts: self.hub.texts(),
        })
    }

    async fn emit_state(&self) {
        match self.state().await {
            Ok(state) => self.ctx.emit(events::STATE, &state),
            Err(err) => tracing::warn!(error = %err, "bridge: failed to build state"),
        }
    }
}

/// `preku://pair?…`: everything the phone needs to connect and pin this PC.
pub fn pairing_link(
    address: &str,
    port: u16,
    fingerprint: &str,
    token: &str,
    pc_id: &str,
    pc_name: &str,
) -> String {
    format!(
        "preku://pair?host={address}&port={port}&fp={fingerprint}&token={token}&id={pc_id}&name={}",
        percent_encode(pc_name)
    )
}

/// The web companion's pairing address: the token rides in the fragment, so it never reaches
/// a server log or a Referer; the page posts it to `/pair` and drops it from the address bar.
pub fn web_link(address: &str, port: u16, token: &str) -> String {
    format!("https://{address}:{port}/#pair={token}")
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn qr_svg(text: &str) -> Result<String> {
    use qrcode::render::svg;
    let code =
        qrcode::QrCode::new(text.as_bytes()).map_err(|e| PrekuError::Bridge(format!("qr: {e}")))?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(svg::Color("#14141a"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

impl BridgeModule {
    fn inner(&self) -> Result<Arc<Inner>> {
        lock(&self.running)
            .as_ref()
            .map(|(inner, _)| inner.clone())
            .ok_or_else(|| {
                PrekuError::InvalidInput("The phone module is turned off in Settings".into())
            })
    }

    pub async fn state(&self) -> Result<BridgeState> {
        self.inner()?.state().await
    }

    /// Opens a 2-minute pairing window and returns the QR to show.
    pub async fn start_pairing(&self) -> Result<BridgeState> {
        let inner = self.inner()?;
        inner.ensure_listening().await;
        let (port, web_port) = {
            let net = inner.net.lock().await;
            let port = net
                .server
                .as_ref()
                .map(|(addr, _)| addr.port())
                .ok_or_else(|| PrekuError::Bridge("not listening".into()))?;
            (port, net.web.as_ref().map(|(addr, _)| addr.port()))
        };
        let address = discovery::lan_address()
            .ok_or_else(|| {
                PrekuError::InvalidInput(
                    "This PC isn't on a network. Connect to Wi-Fi and try again.".into(),
                )
            })?
            .to_string();
        let window = inner.hub.open_pairing()?;
        let identity = inner.hub.identity();
        let link = pairing_link(
            &address,
            port,
            &identity.fingerprint(),
            window.token(),
            &identity.pc_id,
            inner.hub.pc_name(),
        );
        let web_link = web_port.map(|p| web_link(&address, p, window.token()));
        let view = PairingView {
            qr_svg: qr_svg(&link)?,
            link,
            web_qr_svg: web_link.as_deref().map(qr_svg).transpose()?,
            web_link,
            address: format!("{address}:{port}"),
            expires_at: window.expires_at,
        };
        {
            let mut net = inner.net.lock().await;
            net.pairing = Some(view);
            if let Some(old) = net.expiry.take() {
                old.abort();
            }
            // Close the window (and the server, if no phone) when the token expires.
            let later = inner.clone();
            net.expiry = Some(tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(pairing::TOKEN_TTL_MS as u64 + 500)).await;
                later.hub.close_pairing();
                later.stop_if_idle().await;
                later.emit_state().await;
            }));
        }
        let state = inner.state().await?;
        inner.ctx.emit(events::STATE, &state);
        Ok(state)
    }

    pub async fn cancel_pairing(&self) -> Result<BridgeState> {
        let inner = self.inner()?;
        inner.hub.close_pairing();
        inner.net.lock().await.pairing = None;
        inner.stop_if_idle().await;
        let state = inner.state().await?;
        inner.ctx.emit(events::STATE, &state);
        Ok(state)
    }

    /// Sends files to the paired phone that is online.
    pub async fn send_files(&self, paths: &[std::path::PathBuf]) -> Result<BridgeState> {
        let inner = self.inner()?;
        let phone = inner
            .hub
            .snapshot()?
            .into_iter()
            .find(|p| p.online)
            .ok_or_else(|| {
                PrekuError::InvalidInput("Your phone isn't connected right now".into())
            })?;
        for path in paths {
            inner.hub.send_file(&phone.device, path).await?;
        }
        inner.state().await
    }

    /// Sends text or a link to the connected phone.
    pub async fn send_text(&self, text: &str) -> Result<BridgeState> {
        let inner = self.inner()?;
        let phone = inner
            .hub
            .snapshot()?
            .into_iter()
            .find(|p| p.online)
            .ok_or_else(|| {
                PrekuError::InvalidInput("Your phone isn't connected right now".into())
            })?;
        inner.hub.send_text(&phone.device, text)?;
        inner.state().await
    }

    /// The address of a shared text that is a plain web link (for "Open").
    pub fn text_link(&self, id: &str) -> Result<String> {
        self.inner()?
            .hub
            .text(id)
            .and_then(|entry| texts::link(&entry.text))
            .map(String::from)
            .ok_or_else(|| PrekuError::InvalidInput("That isn't a web link".into()))
    }

    pub async fn clear_texts(&self) -> Result<BridgeState> {
        let inner = self.inner()?;
        inner.hub.clear_texts();
        inner.state().await
    }

    pub async fn cancel_transfer(&self, id: &str) -> Result<BridgeState> {
        let inner = self.inner()?;
        inner.hub.cancel_transfer(id).await;
        inner.state().await
    }

    /// A received file (by transfer id), for "show in folder". Only files Preku itself saved.
    pub fn received_path(&self, id: &str) -> Result<std::path::PathBuf> {
        self.inner()?
            .hub
            .received_path(id)
            .ok_or_else(|| PrekuError::InvalidInput("That file isn't there any more".into()))
    }

    pub fn inbox(&self) -> Result<std::path::PathBuf> {
        let inbox = self.inner()?.hub.inbox().to_path_buf();
        std::fs::create_dir_all(&inbox)?;
        Ok(inbox)
    }

    pub async fn unpair(&self, device_id: &str) -> Result<BridgeState> {
        let inner = self.inner()?;
        inner.hub.unpair(device_id)?;
        inner.stop_if_idle().await;
        let state = inner.state().await?;
        inner.ctx.emit(events::STATE, &state);
        Ok(state)
    }
}

fn now_ms() -> i64 {
    crate::foundation::storage::now_millis()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod web_tests;
