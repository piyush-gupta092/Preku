//! The browser companion over real TLS on localhost: the page, pairing with the QR token, the
//! cookie-authenticated WebSocket, and a file from the browser landing in the inbox.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use sha2::Digest as _;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{client::TlsStream, TlsConnector};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, Message},
    WebSocketStream,
};

use super::{
    devices,
    files::hex,
    identity::Identity,
    server::{Hub, HubEvent},
    tls::PinnedServer,
    web::{serve_web, web_fingerprint, WEB_PREFIX},
    web_link,
};
use crate::{
    foundation::storage::{now_millis, Storage},
    protocol::{message_type as kind, Envelope, PROTOCOL_VERSION},
};

struct Fixture {
    hub: Arc<Hub>,
    addr: SocketAddr,
    storage: Arc<Storage>,
    events: Arc<Mutex<Vec<HubEvent>>>,
    inbox: std::path::PathBuf,
}

async fn fixture() -> Fixture {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let inbox = std::env::temp_dir().join(format!("preku-web-inbox-{}", uuid::Uuid::new_v4()));
    let hub = Hub::new(
        storage.clone(),
        Identity::generate("pc-test".into()).unwrap(),
        "Test PC".into(),
        inbox.clone(),
        move |e| sink.lock().unwrap().push(e),
    );
    let (addr, _task) = serve_web(hub.clone(), SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    Fixture {
        hub,
        addr,
        storage,
        events,
        inbox,
    }
}

/// A browser: trusts this PC's certificate (as Safari does after "visit this website") and has
/// no client certificate.
async fn tls(f: &Fixture) -> TlsStream<TcpStream> {
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(PinnedServer::new(&f.hub.identity().fingerprint())))
    .with_no_client_auth();
    let tcp = TcpStream::connect(f.addr).await.unwrap();
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
        .unwrap()
}

struct Reply {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<String> {
        self.head.split("\r\n").find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
        })
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn request(
    f: &Fixture,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: &str,
) -> Reply {
    let mut stream = tls(f).await;
    let mut text = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\n",
        host(f),
        body.len()
    );
    for (k, v) in headers {
        text.push_str(&format!("{k}: {v}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(body);
    stream.write_all(text.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw).await;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    Reply {
        status,
        head,
        body: raw[split + 4..].to_vec(),
    }
}

fn host(f: &Fixture) -> String {
    format!("localhost:{}", f.addr.port())
}

fn origin(f: &Fixture) -> (&'static str, String) {
    ("Origin", format!("https://{}", host(f)))
}

/// Pairs like the page does and returns the cookie's value.
async fn pair(f: &Fixture) -> String {
    let window = f.hub.open_pairing().unwrap();
    let body = serde_json::json!({ "token": window.token(), "deviceName": "iPhone" }).to_string();
    let reply = request(
        f,
        "POST",
        "/pair",
        &[origin(f), ("Content-Type", "application/json".into())],
        &body,
    )
    .await;
    assert_eq!(reply.status, 200);
    let set = reply.header("set-cookie").unwrap();
    set.split(';')
        .next()
        .unwrap()
        .strip_prefix("preku=")
        .unwrap()
        .to_owned()
}

type Ws = WebSocketStream<TlsStream<TcpStream>>;

async fn open_ws(f: &Fixture, cookie: Option<&str>, origin_value: &str) -> Result<Ws, String> {
    let mut req = format!("wss://{}/ws", host(f))
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("Origin", origin_value.parse().unwrap());
    if let Some(cookie) = cookie {
        req.headers_mut()
            .insert("Cookie", format!("preku={cookie}").parse().unwrap());
    }
    tokio_tungstenite::client_async(req, tls(f).await)
        .await
        .map(|(ws, _)| ws)
        .map_err(|e| e.to_string())
}

async fn send(ws: &mut Ws, kind: &str, body: serde_json::Value) {
    let env = serde_json::json!({
        "v": PROTOCOL_VERSION, "id": uuid::Uuid::new_v4().to_string(),
        "type": kind, "ts": now_millis(), "body": body,
    });
    ws.send(Message::text(env.to_string())).await.unwrap();
}

async fn recv(ws: &mut Ws) -> Option<Envelope> {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .ok()??;
        match message.ok()? {
            Message::Text(text) => return serde_json::from_str(text.as_str()).ok(),
            Message::Close(_) => return None,
            _ => {}
        }
    }
}

#[tokio::test]
async fn the_page_is_served_with_strict_headers() {
    let f = fixture().await;
    let page = request(&f, "GET", "/", &[], "").await;
    assert_eq!(page.status, 200);
    assert!(String::from_utf8_lossy(&page.body).contains("/app.js"));
    let csp = page.header("content-security-policy").unwrap();
    assert!(csp.contains("script-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(
        page.header("x-content-type-options").as_deref(),
        Some("nosniff")
    );

    for path in [
        "/app.js",
        "/sha256.js",
        "/app.css",
        "/manifest.webmanifest",
        "/icon.png",
    ] {
        assert_eq!(
            request(&f, "GET", path, &[], "").await.status,
            200,
            "{path}"
        );
    }
    assert_eq!(
        request(&f, "GET", "/../Cargo.toml", &[], "").await.status,
        404
    );
    assert_eq!(request(&f, "GET", "/me", &[], "").await.status, 401);
}

#[tokio::test]
async fn pairing_needs_the_page_origin_and_a_fresh_token() {
    let f = fixture().await;
    let window = f.hub.open_pairing().unwrap();
    let json = ("Content-Type", "application/json".to_owned());
    let good = serde_json::json!({ "token": window.token(), "deviceName": "iPhone" }).to_string();

    // Another site can't pair on the user's behalf.
    let evil = ("Origin", "https://evil.example".to_owned());
    assert_eq!(
        request(&f, "POST", "/pair", &[evil, json.clone()], &good)
            .await
            .status,
        404
    );
    assert_eq!(
        request(&f, "POST", "/pair", std::slice::from_ref(&json), &good)
            .await
            .status,
        404
    );

    let wrong = serde_json::json!({ "token": "nope", "deviceName": "iPhone" }).to_string();
    let reply = request(&f, "POST", "/pair", &[origin(&f), json.clone()], &wrong).await;
    assert_eq!(reply.status, 403);
    assert_eq!(reply.json()["reason"], "invalid");

    let reply = request(&f, "POST", "/pair", &[origin(&f), json.clone()], &good).await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json()["pcName"], "Test PC");
    let set = reply.header("set-cookie").unwrap();
    for flag in ["HttpOnly", "Secure", "SameSite=Strict"] {
        assert!(set.contains(flag), "cookie lacks {flag}: {set}");
    }
    let secret = set
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("preku=")
        .unwrap()
        .to_owned();
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, HubEvent::Paired(n) if n == "iPhone")));

    // The PC keeps only a hash of the secret.
    let stored = f.storage.with_conn(|c| devices::list(c)).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].cert_fp, web_fingerprint(&secret));
    assert!(stored[0].cert_fp.starts_with(WEB_PREFIX) && !stored[0].cert_fp.contains(&secret));

    // Single use.
    let again = request(&f, "POST", "/pair", &[origin(&f), json], &good).await;
    assert_eq!(again.status, 403);

    let me = request(
        &f,
        "GET",
        "/me",
        &[("Cookie", format!("preku={secret}"))],
        "",
    )
    .await;
    assert_eq!(me.status, 200);
    assert_eq!(me.json()["deviceName"], "iPhone");
    let forged = request(&f, "GET", "/me", &[("Cookie", "preku=forged".into())], "").await;
    assert_eq!(forged.status, 401);
}

#[tokio::test]
async fn the_socket_needs_the_cookie_and_the_page_origin() {
    let f = fixture().await;
    let secret = pair(&f).await;
    let page = format!("https://{}", host(&f));
    assert!(open_ws(&f, None, &page).await.is_err());
    assert!(open_ws(&f, Some("forged"), &page).await.is_err());
    assert!(open_ws(&f, Some(&secret), "https://evil.example")
        .await
        .is_err());

    let mut ws = open_ws(&f, Some(&secret), &page).await.unwrap();
    let welcome = recv(&mut ws).await.unwrap();
    assert_eq!(welcome.r#type, kind::WELCOME);
    assert_eq!(welcome.body["pcName"], "Test PC");
    let hub = f.hub.clone();
    for _ in 0..100 {
        if hub.snapshot().unwrap().iter().any(|s| s.online) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(hub.snapshot().unwrap()[0].online);

    send(&mut ws, "system.ping", serde_json::json!({})).await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::PONG);

    // "Forget this PC" removes the pairing; the cookie stops working.
    send(&mut ws, kind::UNPAIR, serde_json::json!({})).await;
    assert!(recv(&mut ws).await.is_none());
    assert!(f
        .storage
        .with_conn(|c| devices::list(c))
        .unwrap()
        .is_empty());
    assert_eq!(
        request(
            &f,
            "GET",
            "/me",
            &[("Cookie", format!("preku={secret}"))],
            ""
        )
        .await
        .status,
        401
    );
}

#[tokio::test]
async fn a_file_from_the_browser_lands_in_the_inbox() {
    let f = fixture().await;
    let secret = pair(&f).await;
    let mut ws = open_ws(&f, Some(&secret), &format!("https://{}", host(&f)))
        .await
        .unwrap();
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::WELCOME);

    let bytes: Vec<u8> = (0..(super::files::CHUNK + 999))
        .map(|i| (i % 253) as u8)
        .collect();
    let id = uuid::Uuid::new_v4().to_string();
    let sha = hex(&sha2::Sha256::digest(&bytes));
    send(&mut ws, kind::FILE_OFFER, serde_json::json!({ "transferId": id, "name": "IMG_0001.HEIC", "size": bytes.len(), "sha256": sha })).await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::FILE_ACCEPT);
    for (index, chunk) in bytes.chunks(super::files::CHUNK).enumerate() {
        let data = base64::engine::general_purpose::STANDARD.encode(chunk);
        send(
            &mut ws,
            kind::FILE_CHUNK,
            serde_json::json!({ "transferId": id, "index": index, "data": data }),
        )
        .await;
        assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::FILE_ACK);
    }
    send(
        &mut ws,
        kind::FILE_DONE,
        serde_json::json!({ "transferId": id }),
    )
    .await;
    let result = recv(&mut ws).await.unwrap();
    assert_eq!(result.r#type, kind::FILE_RESULT);
    assert_eq!(result.body["ok"], true);
    assert_eq!(std::fs::read(f.inbox.join("IMG_0001.HEIC")).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&f.inbox);
}

#[test]
fn the_web_link_keeps_the_token_out_of_requests() {
    let link = web_link("192.168.1.20", 47824, "tok_en-1");
    assert_eq!(link, "https://192.168.1.20:47824/#pair=tok_en-1");
}

#[test]
fn the_browser_sha256_matches_node() {
    // The page's own SHA-256 (it can't use WebCrypto to stream). Skipped without Node.
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/sha256.test.mjs");
    match std::process::Command::new("node").arg(&script).output() {
        Ok(out) => assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => eprintln!("node not found; skipping"),
    }
}

#[tokio::test]
async fn text_and_links_travel_both_ways() {
    let f = fixture().await;
    let secret = pair(&f).await;
    let mut ws = open_ws(&f, Some(&secret), &format!("https://{}", host(&f)))
        .await
        .unwrap();
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::WELCOME);

    // Phone → PC.
    send(
        &mut ws,
        kind::CLIPBOARD_TEXT,
        serde_json::json!({ "text": "https://example.com/a?b=1" }),
    )
    .await;
    let hub = f.hub.clone();
    for _ in 0..100 {
        if !hub.texts().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let received = hub.texts();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].text, "https://example.com/a?b=1");
    assert!(received[0].is_link);
    assert_eq!(received[0].device_name, "iPhone");
    let id = received[0].id.clone();
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, HubEvent::TextReceived { id: got } if *got == id)));

    // Blank or oversized text is ignored, and the session stays up.
    send(
        &mut ws,
        kind::CLIPBOARD_TEXT,
        serde_json::json!({ "text": "  " }),
    )
    .await;
    let huge = "x".repeat(super::texts::MAX_TEXT_CHARS + 1);
    send(
        &mut ws,
        kind::CLIPBOARD_TEXT,
        serde_json::json!({ "text": huge }),
    )
    .await;
    send(&mut ws, "system.ping", serde_json::json!({})).await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::PONG);
    assert_eq!(hub.texts().len(), 1);

    // PC → phone.
    let device = hub.snapshot().unwrap().remove(0).device;
    hub.send_text(&device, "meet at 5, gate B").unwrap();
    let message = recv(&mut ws).await.unwrap();
    assert_eq!(message.r#type, kind::CLIPBOARD_TEXT);
    assert_eq!(message.body["text"], "meet at 5, gate B");
    assert_eq!(hub.texts().len(), 2);
    assert!(hub.send_text(&device, "   ").is_err());

    // Not connected: a clear error, nothing remembered.
    drop(ws);
    for _ in 0..100 {
        if !hub.snapshot().unwrap()[0].online {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(hub.send_text(&device, "later").is_err());
    assert_eq!(hub.texts().len(), 2);
}
