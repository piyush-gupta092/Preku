//! Text and links shared between a paired phone and this PC (`clipboard.text`). Kept in memory
//! only (the last few, for the Settings list): never written to disk and never logged
//! (Rules.md §4). A received link is never opened by itself.

use std::collections::VecDeque;

use serde::Serialize;
use ts_rs::TS;

use super::files::TransferDirection;
use crate::foundation::errors::{PrekuError, Result};

/// Plenty for a paragraph or a long link; keeps one message far below the frame limit.
pub const MAX_TEXT_CHARS: usize = 20_000;
const KEEP: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedText {
    pub id: String,
    pub text: String,
    /// The whole text is one http(s) address (the UI offers "Open").
    pub is_link: bool,
    pub direction: TransferDirection,
    pub device_name: String,
    #[ts(type = "number")]
    pub at: i64,
}

/// Text worth sending: not blank, not huge, no NUL bytes.
pub fn validate(text: &str) -> Result<String> {
    if text.trim().is_empty() {
        return Err(PrekuError::InvalidInput("There's no text to send".into()));
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(PrekuError::InvalidInput(format!(
            "That's longer than {MAX_TEXT_CHARS} characters: send it as a file instead"
        )));
    }
    Ok(text.replace('\0', ""))
}

/// The address to open, if the whole text is a single well-formed http(s) link.
pub fn link(text: &str) -> Option<reqwest::Url> {
    let text = text.trim();
    if text.len() > 2_000 || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let url = reqwest::Url::parse(text).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then_some(url)
}

/// The last few shared texts, newest first.
#[derive(Debug, Default)]
pub struct Texts(VecDeque<SharedText>);

impl Texts {
    pub fn push(
        &mut self,
        text: String,
        direction: TransferDirection,
        device_name: &str,
        now: i64,
    ) -> SharedText {
        let entry = SharedText {
            id: uuid::Uuid::new_v4().to_string(),
            is_link: link(&text).is_some(),
            text,
            direction,
            device_name: device_name.to_owned(),
            at: now,
        };
        self.0.push_front(entry.clone());
        self.0.truncate(KEEP);
        entry
    }

    pub fn list(&self) -> Vec<SharedText> {
        self.0.iter().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<SharedText> {
        self.0.iter().find(|t| t.id == id).cloned()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_text() {
        assert!(validate("   \n").is_err());
        assert!(validate(&"x".repeat(MAX_TEXT_CHARS + 1)).is_err());
        assert_eq!(validate("hi\0 there").unwrap(), "hi there");
        assert_eq!(
            validate(&"x".repeat(MAX_TEXT_CHARS)).unwrap().len(),
            MAX_TEXT_CHARS
        );
    }

    #[test]
    fn only_plain_web_addresses_count_as_links() {
        assert!(link("https://example.com/a?b=1#c").is_some());
        assert!(link("  http://example.com  ").is_some());
        for not_a_link in [
            "see https://example.com",
            "file:///C:/Windows/system32/calc.exe",
            "javascript:alert(1)",
            "ms-settings:privacy",
            "https://",
            "example.com",
            "https://example.com/\ncalc.exe",
        ] {
            assert!(link(not_a_link).is_none(), "{not_a_link}");
        }
    }

    #[test]
    fn keeps_the_newest_few() {
        let mut texts = Texts::default();
        for i in 0..(KEEP + 5) {
            texts.push(
                format!("note {i}"),
                TransferDirection::Receive,
                "iPhone",
                i as i64,
            );
        }
        let list = texts.list();
        assert_eq!(list.len(), KEEP);
        assert_eq!(list[0].text, format!("note {}", KEEP + 4));
        assert!(texts.get(&list[3].id).is_some());
        assert!(!list[0].is_link);
        let entry = texts.push(
            "https://preku.app".into(),
            TransferDirection::Send,
            "iPhone",
            99,
        );
        assert!(entry.is_link);
        texts.clear();
        assert!(texts.list().is_empty());
    }
}
