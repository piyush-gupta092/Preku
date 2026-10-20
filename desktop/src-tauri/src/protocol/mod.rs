//! Bridge protocol types, generated from `protocol/schemas` (Architecture.md §4).
//! Regenerate with `node protocol/codegen/generate.mjs`; never edit `generated.rs` by hand.

mod generated;

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    /// Contract test: every shared example must parse and round-trip without loss.
    #[test]
    fn parses_all_shared_examples() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol/examples");
        let mut count = 0;
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let raw = fs::read_to_string(&path).unwrap();
            let envelope: Envelope = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
            assert_eq!(envelope.v, PROTOCOL_VERSION, "{}", path.display());

            let original: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(serde_json::to_value(&envelope).unwrap(), original);
            assert_body_parses(&envelope);
            count += 1;
        }
        assert!(count > 0, "no examples found in {}", dir.display());
    }

    /// Every body must match its generated type (the same check runs on Android).
    fn assert_body_parses(envelope: &Envelope) {
        fn parse<T: serde::de::DeserializeOwned + serde::Serialize>(env: &Envelope) {
            let body = serde_json::Value::Object(env.body.clone());
            let typed: T = serde_json::from_value(body.clone())
                .unwrap_or_else(|e| panic!("{} body: {e}", env.r#type));
            assert_eq!(serde_json::to_value(typed).unwrap(), body, "{}", env.r#type);
        }
        match envelope.r#type.as_str() {
            message_type::PAIR_REQUEST => parse::<PairRequest>(envelope),
            message_type::PAIR_ACCEPTED => parse::<PairAccepted>(envelope),
            message_type::PAIR_REJECTED => parse::<PairRejected>(envelope),
            message_type::HELLO => parse::<Hello>(envelope),
            message_type::WELCOME => parse::<Welcome>(envelope),
            message_type::DEVICE_STATUS => parse::<DeviceStatus>(envelope),
            message_type::UNPAIR => parse::<Unpair>(envelope),
            message_type::PONG => parse::<Pong>(envelope),
            message_type::FILE_OFFER => parse::<FileOffer>(envelope),
            message_type::FILE_ACCEPT => parse::<FileAccept>(envelope),
            message_type::FILE_DECLINE => parse::<FileDecline>(envelope),
            message_type::FILE_CHUNK => parse::<FileChunk>(envelope),
            message_type::FILE_ACK => parse::<FileAck>(envelope),
            message_type::FILE_DONE => parse::<FileDone>(envelope),
            message_type::FILE_RESULT => parse::<FileResult>(envelope),
            message_type::FILE_CANCEL => parse::<FileCancel>(envelope),
            message_type::CLIPBOARD_TEXT => parse::<ClipboardText>(envelope),
            "system.ping" => {}
            other => panic!("example with unknown type {other}"),
        }
    }
}
