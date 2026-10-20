//! End-to-end tests over real TLS on localhost: pairing, reconnecting, status, unpairing, and
//! that unpaired or forged peers are rejected (Phase 6 "done when").

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_rustls::{client::TlsStream, TlsConnector};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use super::{
    devices,
    identity::Identity,
    pairing_link,
    server::{serve, Hub, HubEvent},
    tls::client_config,
};
use crate::{
    foundation::storage::{now_millis, Storage},
    protocol::{message_type as kind, Envelope, PROTOCOL_VERSION},
};

type Ws = WebSocketStream<TlsStream<TcpStream>>;

struct Fixture {
    hub: Arc<Hub>,
    addr: SocketAddr,
    storage: Arc<Storage>,
    events: Arc<Mutex<Vec<HubEvent>>>,
}

async fn fixture() -> Fixture {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let hub = Hub::new(
        storage.clone(),
        Identity::generate("pc-test".into()).unwrap(),
        "Test PC".into(),
        std::env::temp_dir().join(format!("preku-inbox-{}", uuid::Uuid::new_v4())),
        move |e| sink.lock().unwrap().push(e),
    );
    let (addr, _task) = serve(hub.clone(), SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    Fixture {
        hub,
        addr,
        storage,
        events,
    }
}

async fn connect(addr: SocketAddr, server_fp: &str, phone: &Identity) -> std::io::Result<Ws> {
    let config = client_config(server_fp, phone.cert(), phone.key()).unwrap();
    let tcp = TcpStream::connect(addr).await?;
    let name = rustls::pki_types::ServerName::try_from("preku.local").unwrap();
    let tls = TlsConnector::from(config).connect(name, tcp).await?;
    let (ws, _) = tokio_tungstenite::client_async("wss://preku.local/", tls)
        .await
        .map_err(std::io::Error::other)?;
    Ok(ws)
}

async fn send(ws: &mut Ws, kind: &str, body: serde_json::Value) {
    let env = serde_json::json!({
        "v": PROTOCOL_VERSION, "id": uuid::Uuid::new_v4().to_string(),
        "type": kind, "ts": now_millis(), "body": body,
    });
    ws.send(Message::text(env.to_string())).await.unwrap();
}

/// The next protocol message, or None when the server closed the connection.
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

async fn eventually(check: impl Fn() -> bool) {
    for _ in 0..100 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition never became true");
}

#[tokio::test]
async fn pair_reconnect_report_status_and_unpair() {
    let f = fixture().await;
    let fp = f.hub.identity().fingerprint();
    let phone = Identity::generate("phone".into()).unwrap();
    let window = f.hub.open_pairing().unwrap();

    // Pairing with the QR token.
    let mut ws = connect(f.addr, &fp, &phone).await.unwrap();
    send(
        &mut ws,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": window.token(), "deviceName": "Pixel 8" }),
    )
    .await;
    let reply = recv(&mut ws).await.unwrap();
    assert_eq!(reply.r#type, kind::PAIR_ACCEPTED);
    assert_eq!(reply.body["pcId"], "pc-test");
    assert!(f.hub.pairing().is_none(), "the token is single use");
    assert!(f
        .events
        .lock()
        .unwrap()
        .contains(&HubEvent::Paired("Pixel 8".into())));
    drop(ws);

    // Reconnecting later: the certificate alone identifies the phone.
    let mut ws = connect(f.addr, &fp, &phone).await.unwrap();
    send(
        &mut ws,
        kind::HELLO,
        serde_json::json!({ "deviceName": "Pixel 8" }),
    )
    .await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::WELCOME);

    // Status shows up on the PC.
    send(
        &mut ws,
        kind::DEVICE_STATUS,
        serde_json::json!({ "battery": 81, "charging": true, "network": "wifi" }),
    )
    .await;
    let hub = f.hub.clone();
    eventually(|| {
        hub.snapshot().unwrap()[0]
            .status
            .as_ref()
            .is_some_and(|s| s.battery == 81)
    })
    .await;
    let snap = f.hub.snapshot().unwrap();
    assert!(snap[0].online);
    assert!(snap[0].status.as_ref().unwrap().charging);

    // App-level ping.
    send(&mut ws, "system.ping", serde_json::json!({})).await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::PONG);

    // Unpair from the phone's side.
    send(&mut ws, kind::UNPAIR, serde_json::json!({})).await;
    assert!(recv(&mut ws).await.is_none(), "server closes after unpair");
    let storage = f.storage.clone();
    eventually(|| storage.with_conn(|c| devices::list(c)).unwrap().is_empty()).await;
}

#[tokio::test]
async fn unpairing_from_the_pc_tells_the_phone() {
    let f = fixture().await;
    let fp = f.hub.identity().fingerprint();
    let phone = Identity::generate("phone".into()).unwrap();
    let window = f.hub.open_pairing().unwrap();
    let mut ws = connect(f.addr, &fp, &phone).await.unwrap();
    send(
        &mut ws,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": window.token(), "deviceName": "P" }),
    )
    .await;
    recv(&mut ws).await.unwrap();

    let id = f.hub.snapshot().unwrap()[0].device.id.clone();
    assert!(f.hub.unpair(&id).unwrap());
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::UNPAIR);
    assert!(!f.hub.has_devices());
}

#[tokio::test]
async fn unknown_phones_are_rejected_outside_pairing() {
    let f = fixture().await;
    let fp = f.hub.identity().fingerprint();
    let stranger = Identity::generate("stranger".into()).unwrap();
    let mut ws = connect(f.addr, &fp, &stranger).await.unwrap();
    send(
        &mut ws,
        kind::HELLO,
        serde_json::json!({ "deviceName": "Evil" }),
    )
    .await;
    let reply = recv(&mut ws).await.unwrap();
    assert_eq!(reply.r#type, kind::PAIR_REJECTED);
    assert_eq!(reply.body["reason"], "unknown_device");
    assert!(recv(&mut ws).await.is_none(), "then disconnected");
    assert!(!f.hub.has_devices());
}

#[tokio::test]
async fn wrong_tokens_are_rejected() {
    let f = fixture().await;
    let fp = f.hub.identity().fingerprint();
    f.hub.open_pairing().unwrap();
    let phone = Identity::generate("phone".into()).unwrap();
    let mut ws = connect(f.addr, &fp, &phone).await.unwrap();
    send(
        &mut ws,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": "guess", "deviceName": "P" }),
    )
    .await;
    let reply = recv(&mut ws).await.unwrap();
    assert_eq!(reply.body["reason"], "invalid");
    assert!(!f.hub.has_devices());
    assert!(
        f.hub.pairing().is_some(),
        "a bad guess doesn't end the real window"
    );
}

#[tokio::test]
async fn a_forged_phone_cannot_impersonate_a_paired_one() {
    let f = fixture().await;
    let fp = f.hub.identity().fingerprint();
    let real = Identity::generate("real".into()).unwrap();
    let window = f.hub.open_pairing().unwrap();
    let token = window.token().to_owned();
    let mut ws = connect(f.addr, &fp, &real).await.unwrap();
    send(
        &mut ws,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": token, "deviceName": "Pixel 8" }),
    )
    .await;
    recv(&mut ws).await.unwrap();

    // Same name, replayed token, but a different key: rejected.
    let forged = Identity::generate("forged".into()).unwrap();
    let mut evil = connect(f.addr, &fp, &forged).await.unwrap();
    send(
        &mut evil,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": token, "deviceName": "Pixel 8" }),
    )
    .await;
    assert_eq!(recv(&mut evil).await.unwrap().body["reason"], "not_pairing");
    let mut evil = connect(f.addr, &fp, &forged).await.unwrap();
    send(
        &mut evil,
        kind::HELLO,
        serde_json::json!({ "deviceName": "Pixel 8" }),
    )
    .await;
    assert_eq!(
        recv(&mut evil).await.unwrap().body["reason"],
        "unknown_device"
    );
    assert_eq!(f.hub.snapshot().unwrap().len(), 1);
}

#[tokio::test]
async fn the_phone_refuses_a_pc_with_the_wrong_fingerprint() {
    let f = fixture().await;
    let phone = Identity::generate("phone".into()).unwrap();
    let imposter_fp = Identity::generate("other-pc".into()).unwrap().fingerprint();
    assert!(connect(f.addr, &imposter_fp, &phone).await.is_err());
}

#[test]
fn pairing_links_carry_everything_the_phone_needs() {
    let link = pairing_link(
        "192.168.1.20",
        47823,
        "ab12",
        "tok_en",
        "pc-1",
        "Desk PC #2",
    );
    assert_eq!(
        link,
        "preku://pair?host=192.168.1.20&port=47823&fp=ab12&token=tok_en&id=pc-1&name=Desk%20PC%20%232"
    );
}

// ---- File sharing ----

use base64::Engine as _;
use sha2::Digest as _;

use super::files::{TransferStatus, CHUNK};

/// Pairs a fresh phone and returns its connection.
async fn paired_phone(f: &Fixture) -> Ws {
    let fp = f.hub.identity().fingerprint();
    let phone = Identity::generate("phone".into()).unwrap();
    let window = f.hub.open_pairing().unwrap();
    let mut ws = connect(f.addr, &fp, &phone).await.unwrap();
    send(
        &mut ws,
        kind::PAIR_REQUEST,
        serde_json::json!({ "token": window.token(), "deviceName": "Pixel 8" }),
    )
    .await;
    assert_eq!(recv(&mut ws).await.unwrap().r#type, kind::PAIR_ACCEPTED);
    ws
}

fn sha(bytes: &[u8]) -> String {
    super::files::hex(&sha2::Sha256::digest(bytes))
}

/// The phone sends `bytes` named `name`; returns the PC's `file.result`.
async fn phone_sends(ws: &mut Ws, name: &str, bytes: &[u8], claimed_sha: &str) -> Envelope {
    let id = uuid::Uuid::new_v4().to_string();
    send(ws, kind::FILE_OFFER, serde_json::json!({ "transferId": id, "name": name, "size": bytes.len(), "sha256": claimed_sha })).await;
    assert_eq!(recv(ws).await.unwrap().r#type, kind::FILE_ACCEPT);
    for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
        let data = base64::engine::general_purpose::STANDARD.encode(chunk);
        send(
            ws,
            kind::FILE_CHUNK,
            serde_json::json!({ "transferId": id, "index": index, "data": data }),
        )
        .await;
        let ack = recv(ws).await.unwrap();
        assert_eq!(ack.r#type, kind::FILE_ACK);
        assert_eq!(ack.body["index"], index);
    }
    send(ws, kind::FILE_DONE, serde_json::json!({ "transferId": id })).await;
    recv(ws).await.unwrap()
}

#[tokio::test]
async fn the_phone_sends_a_file_to_the_pc() {
    let f = fixture().await;
    let mut ws = paired_phone(&f).await;
    let bytes: Vec<u8> = (0..(CHUNK * 2 + 1234)).map(|i| (i % 251) as u8).collect();

    let result = phone_sends(&mut ws, "../../evil/holiday.jpg", &bytes, &sha(&bytes)).await;
    assert_eq!(result.r#type, kind::FILE_RESULT);
    assert_eq!(result.body["ok"], true);

    // Saved under a safe name in the inbox, byte for byte; no .part left behind.
    let saved = f.hub.inbox().join("holiday.jpg");
    assert_eq!(std::fs::read(&saved).unwrap(), bytes);
    assert!(std::fs::read_dir(f.hub.inbox()).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".part")));
    let view = &f.hub.transfers()[0];
    assert_eq!(view.status, TransferStatus::Done);
    assert_eq!(view.bytes, bytes.len() as i64);
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, HubEvent::FileReceived { name, .. } if name == "holiday.jpg")));

    // The same name again doesn't overwrite.
    let again = phone_sends(&mut ws, "holiday.jpg", b"second", &sha(b"second")).await;
    assert_eq!(again.body["ok"], true);
    assert_eq!(
        std::fs::read(f.hub.inbox().join("holiday (1).jpg")).unwrap(),
        b"second"
    );
    let _ = std::fs::remove_dir_all(f.hub.inbox());
}

#[tokio::test]
async fn a_corrupted_file_is_refused_and_removed() {
    let f = fixture().await;
    let mut ws = paired_phone(&f).await;
    let result = phone_sends(&mut ws, "notes.txt", b"hello", &sha(b"something else")).await;
    assert_eq!(result.body["ok"], false);
    assert_eq!(result.body["reason"], "checksum");
    assert!(
        std::fs::read_dir(f.hub.inbox()).unwrap().next().is_none(),
        "nothing kept"
    );
    assert_eq!(f.hub.transfers()[0].status, TransferStatus::Failed);
    let _ = std::fs::remove_dir_all(f.hub.inbox());
}

#[tokio::test]
async fn the_pc_sends_a_file_to_the_phone() {
    let f = fixture().await;
    let mut ws = paired_phone(&f).await;
    let bytes: Vec<u8> = (0..(CHUNK * 5 + 7)).map(|i| (i * 7 % 256) as u8).collect();
    let path = std::env::temp_dir().join(format!("preku-send-{}.bin", uuid::Uuid::new_v4()));
    std::fs::write(&path, &bytes).unwrap();
    let device = f.hub.snapshot().unwrap().remove(0).device;

    let id = f.hub.send_file(&device, &path).await.unwrap();
    let offer = recv(&mut ws).await.unwrap();
    assert_eq!(offer.r#type, kind::FILE_OFFER);
    assert_eq!(offer.body["size"], bytes.len());
    send(
        &mut ws,
        kind::FILE_ACCEPT,
        serde_json::json!({ "transferId": id }),
    )
    .await;

    // Receive in order, acking as we go (the PC never runs more than 4 chunks ahead).
    let mut got = Vec::new();
    loop {
        let message = recv(&mut ws).await.unwrap();
        if message.r#type == kind::FILE_DONE {
            break;
        }
        assert_eq!(message.r#type, kind::FILE_CHUNK);
        let data = base64::engine::general_purpose::STANDARD
            .decode(message.body["data"].as_str().unwrap())
            .unwrap();
        got.extend_from_slice(&data);
        send(
            &mut ws,
            kind::FILE_ACK,
            serde_json::json!({ "transferId": id, "index": message.body["index"] }),
        )
        .await;
    }
    assert_eq!(got, bytes);
    assert_eq!(offer.body["sha256"], sha(&bytes));
    send(
        &mut ws,
        kind::FILE_RESULT,
        serde_json::json!({ "transferId": id, "ok": true }),
    )
    .await;
    let hub = f.hub.clone();
    eventually(|| hub.transfers()[0].status == TransferStatus::Done).await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn sending_needs_a_connected_phone() {
    let f = fixture().await;
    let ws = paired_phone(&f).await;
    let device = f.hub.snapshot().unwrap().remove(0).device;
    drop(ws);
    let hub = f.hub.clone();
    eventually(|| !hub.snapshot().unwrap()[0].online).await;
    let path = std::env::temp_dir().join("preku-unused.txt");
    std::fs::write(&path, b"x").unwrap();
    assert!(f.hub.send_file(&device, &path).await.is_err());
    let _ = std::fs::remove_file(&path);
}
