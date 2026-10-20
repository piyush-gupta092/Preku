//! The browser companion for phones without the Preku app (iPhone): a tiny HTTPS server on
//! [`WEB_PORT`] that serves one page and a WebSocket speaking the same bridge protocol.
//!
//! - `GET /`, `/app.js`, `/app.css`, `/manifest.webmanifest`, `/icon.png`: the page (no
//!   secrets in it).
//! - `POST /pair` `{token, deviceName}`: redeems the QR's one-time token and sets a long-lived,
//!   HttpOnly, Secure, SameSite=Strict session cookie. The PC stores only its SHA-256.
//! - `GET /ws` (WebSocket upgrade): authenticated by that cookie, then the usual session
//!   (`server::run_session`), so file sharing works exactly as with the Android app.
//!
//! Same certificate as the bridge (self-signed: Safari asks once to trust it). Requests are
//! small and bounded; anything else gets 404. Only runs while the bridge is listening.

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Semaphore,
    time::timeout,
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tokio_tungstenite::{
    tungstenite::{handshake::derive_accept_key, protocol::Role},
    WebSocketStream,
};

use super::{
    devices::{self, PairedDevice},
    files::hex,
    pairing,
    server::{envelope, lock, run_session, Hub, HubEvent},
    tls,
};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        storage::now_millis,
    },
    protocol::{message_type as kind, Welcome},
};

pub const WEB_PORT: u16 = 47824;
const COOKIE: &str = "preku";
const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 1024;
const MAX_CONNECTIONS: usize = 16;
/// Browser devices are stored with this prefix in place of a certificate fingerprint.
pub const WEB_PREFIX: &str = "web:";

const INDEX: &str = include_str!("../../../web/index.html");
const APP_JS: &str = include_str!("../../../web/app.js");
const SHA_JS: &str = include_str!("../../../web/sha256.js");
const APP_CSS: &str = include_str!("../../../web/app.css");
const MANIFEST: &str = include_str!("../../../web/manifest.webmanifest");
const ICON: &[u8] = include_bytes!("../../../icons/128x128@2x.png");

/// A browser device's id in `bridge_devices.cert_fp`: never the secret itself.
pub fn web_fingerprint(secret: &str) -> String {
    format!("{WEB_PREFIX}{}", hex(&Sha256::digest(secret.as_bytes())))
}

fn random_secret() -> Result<String> {
    let mut bytes = [0u8; 32];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| PrekuError::Bridge("no secure random source".into()))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

impl Hub {
    /// Pairs a browser with the QR's one-time token; returns the session secret for its cookie.
    pub fn pair_web(
        &self,
        token: &str,
        name: &str,
    ) -> Result<std::result::Result<String, &'static str>> {
        if let Err(rejection) = pairing::redeem(&mut lock(&self.pairing), token, now_millis()) {
            return Ok(Err(rejection.reason()));
        }
        let secret = random_secret()?;
        let device = self
            .storage
            .with_conn(|conn| devices::add(conn, name, &web_fingerprint(&secret), now_millis()))?;
        (self.notify)(HubEvent::Paired(device.name));
        Ok(Ok(secret))
    }

    fn web_device(&self, secret: &str) -> Option<PairedDevice> {
        self.storage
            .with_conn(|conn| devices::find_by_fp(conn, &web_fingerprint(secret)))
            .ok()
            .flatten()
    }
}

pub async fn serve_web(
    hub: Arc<Hub>,
    addr: SocketAddr,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let acceptor = TlsAcceptor::from(tls::web_server_config(hub.identity())?);
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let task = tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                continue;
            };
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                continue;
            };
            let (hub, acceptor) = (hub.clone(), acceptor.clone());
            tokio::spawn(async move {
                let _permit = permit;
                let Ok(Ok(tls)) = timeout(Duration::from_secs(10), acceptor.accept(tcp)).await
                else {
                    return;
                };
                if let Err(err) = handle(hub, tls).await {
                    tracing::debug!(error = %err, "web companion request ended");
                }
            });
        }
    });
    Ok((local, task))
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

/// Reads one request head (and a small body), with hard limits.
async fn read_request(stream: &mut TlsStream<tokio::net::TcpStream>) -> Result<Option<Request>> {
    let mut buf = Vec::with_capacity(1024);
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return Ok(None);
        }
        let mut chunk = [0u8; 1024];
        let n = timeout(Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .map_err(|_| PrekuError::Bridge("slow request".into()))??;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default().to_owned();
    let path = first
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return Ok(None);
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let mut chunk = [0u8; 1024];
        let n = timeout(Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .map_err(|_| PrekuError::Bridge("slow body".into()))??;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(Some(Request {
        method,
        path,
        headers,
        body,
    }))
}

fn cookie(req: &Request) -> Option<String> {
    req.headers.get("cookie")?.split(';').find_map(|part| {
        let (k, v) = part.trim().split_once('=')?;
        (k == COOKIE).then(|| v.to_owned())
    })
}

/// Same-origin requests only (the page itself), for the POST and the WebSocket.
fn same_origin(req: &Request) -> bool {
    match (req.headers.get("origin"), req.headers.get("host")) {
        (Some(origin), Some(host)) => origin == &format!("https://{host}"),
        _ => false,
    }
}

async fn respond(
    stream: &mut TlsStream<tokio::net::TcpStream>,
    status: &str,
    content_type: &str,
    body: &[u8],
    extra: &str,
) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'self'; img-src 'self' blob: data:; connect-src 'self' wss:; \
         style-src 'self'; script-src 'self'; frame-ancestors 'none'\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    let _ = stream.shutdown().await;
    Ok(())
}

async fn handle(hub: Arc<Hub>, mut stream: TlsStream<tokio::net::TcpStream>) -> Result<()> {
    let Some(req) = read_request(&mut stream).await? else {
        return Ok(());
    };
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => {
            respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                INDEX.as_bytes(),
                "",
            )
            .await
        }
        ("GET", "/app.js") => {
            respond(
                &mut stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                APP_JS.as_bytes(),
                "",
            )
            .await
        }
        ("GET", "/app.css") => {
            respond(
                &mut stream,
                "200 OK",
                "text/css; charset=utf-8",
                APP_CSS.as_bytes(),
                "",
            )
            .await
        }
        ("GET", "/manifest.webmanifest") => {
            respond(
                &mut stream,
                "200 OK",
                "application/manifest+json",
                MANIFEST.as_bytes(),
                "",
            )
            .await
        }
        ("GET", "/icon.png") => respond(&mut stream, "200 OK", "image/png", ICON, "").await,
        ("GET", "/sha256.js") => {
            respond(
                &mut stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                SHA_JS.as_bytes(),
                "",
            )
            .await
        }
        // Is this browser paired? (The cookie is HttpOnly, so the page can't look itself.)
        ("GET", "/me") => match cookie(&req).and_then(|secret| hub.web_device(&secret)) {
            Some(device) => {
                let reply =
                    serde_json::json!({ "pcName": hub.pc_name(), "deviceName": device.name })
                        .to_string();
                respond(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    reply.as_bytes(),
                    "",
                )
                .await
            }
            None => {
                respond(
                    &mut stream,
                    "401 Unauthorized",
                    "application/json",
                    b"{}",
                    "",
                )
                .await
            }
        },
        ("POST", "/pair") if same_origin(&req) => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct PairBody {
                token: String,
                device_name: String,
            }
            let Ok(body) = serde_json::from_slice::<PairBody>(&req.body) else {
                return respond(
                    &mut stream,
                    "400 Bad Request",
                    "application/json",
                    b"{}",
                    "",
                )
                .await;
            };
            match hub.pair_web(&body.token, &body.device_name)? {
                Ok(secret) => {
                    let reply = serde_json::json!({ "pcName": hub.pc_name() }).to_string();
                    // A year; Secure + HttpOnly + SameSite=Strict, so scripts and other sites can't use it.
                    let set = format!(
                        "Set-Cookie: {COOKIE}={secret}; Max-Age=31536000; Path=/; Secure; HttpOnly; SameSite=Strict\r\n"
                    );
                    respond(
                        &mut stream,
                        "200 OK",
                        "application/json",
                        reply.as_bytes(),
                        &set,
                    )
                    .await
                }
                Err(reason) => {
                    let reply = serde_json::json!({ "reason": reason }).to_string();
                    respond(
                        &mut stream,
                        "403 Forbidden",
                        "application/json",
                        reply.as_bytes(),
                        "",
                    )
                    .await
                }
            }
        }
        ("GET", "/ws") if same_origin(&req) => {
            let device = cookie(&req).and_then(|secret| hub.web_device(&secret));
            let key = req.headers.get("sec-websocket-key").cloned();
            let upgrade = req
                .headers
                .get("upgrade")
                .is_some_and(|u| u.eq_ignore_ascii_case("websocket"));
            let (Some(device), Some(key), true) = (device, key, upgrade) else {
                return respond(
                    &mut stream,
                    "401 Unauthorized",
                    "application/json",
                    b"{\"reason\":\"unknown_device\"}",
                    "",
                )
                .await;
            };
            let accept = derive_accept_key(key.as_bytes());
            let head = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).await?;
            let mut ws = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            use futures_util::SinkExt;
            ws.send(envelope(
                kind::WELCOME,
                Welcome {
                    pc_id: hub.identity().pc_id.clone(),
                    pc_name: hub.pc_name().to_owned(),
                },
            ))
            .await
            .map_err(|e| PrekuError::Bridge(e.to_string()))?;
            run_session(hub, device, ws).await
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"Not found", "").await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_secrets_are_stored_hashed() {
        let fp = web_fingerprint("secret");
        assert!(fp.starts_with(WEB_PREFIX));
        assert!(!fp.contains("secret"));
        assert_eq!(fp.len(), WEB_PREFIX.len() + 64);
    }
}
