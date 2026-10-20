//! File sharing between the paired phone and this PC (Architecture.md §4.3 `file.*`).
//!
//! Sender: `file.offer` (name, size, SHA-256) → receiver `file.accept` → `file.chunk`s in order
//! (192 KiB each, at most [`WINDOW`] unacknowledged) → `file.done` → receiver `file.result`.
//! The receiver writes to a `.part` file, checks size and SHA-256, then renames it into the inbox
//! (`Downloads\Preku`). Paired devices are trusted, so offers are accepted automatically.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

pub const CHUNK: usize = 192 * 1024;
/// Chunks in flight before the sender waits for an ack (bounded memory, steady speed on LAN).
pub const WINDOW: u32 = 4;
/// Largest file accepted (a sanity limit, not a quota).
pub const MAX_SIZE: i64 = 8 * 1024 * 1024 * 1024;
/// How many finished transfers the UI keeps listing.
pub const HISTORY: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransferStatus {
    /// Offered, waiting for the other side.
    Waiting,
    Active,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferView {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: i64,
    #[ts(type = "number")]
    pub bytes: i64,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub device_name: String,
    /// Why it failed (a `file.result`/`file.decline` reason or "disconnected").
    pub error: Option<String>,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// How many chunks a file of `size` bytes takes (an empty file still sends one empty chunk).
pub fn chunk_count(size: i64) -> u32 {
    let size = size.max(0) as u64;
    (size.div_ceil(CHUNK as u64)).max(1) as u32
}

/// A safe file name from whatever the other device sent: no folders, no characters Windows
/// forbids, no reserved device names, not empty, not too long.
pub fn sanitize_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned
        .trim()
        .trim_end_matches(['.', ' '])
        .trim_start_matches('.');
    let mut out: String = trimmed.chars().take(120).collect();
    if out.is_empty() {
        out = "file".to_owned();
    }
    let stem = out
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        out.insert(0, '_');
    }
    out
}

/// `dir/name`, or `dir/name (1).ext`, `(2)`… if that already exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (1..10_000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem} ({}){ext}", uuid::Uuid::new_v4())))
}

/// Lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_the_phone_are_made_safe() {
        assert_eq!(sanitize_name("holiday.jpg"), "holiday.jpg");
        assert_eq!(sanitize_name("../../Windows/system32/evil.dll"), "evil.dll");
        assert_eq!(sanitize_name("C:\\Users\\x\\a.txt"), "a.txt");
        assert_eq!(sanitize_name("what?<is>this*.txt"), "what__is_this_.txt");
        assert_eq!(sanitize_name("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_name("com1"), "_com1");
        assert_eq!(sanitize_name("compass.txt"), "compass.txt");
        assert_eq!(sanitize_name("..."), "file");
        assert_eq!(sanitize_name(".hidden"), "hidden");
        assert_eq!(sanitize_name(""), "file");
        assert_eq!(sanitize_name(&"a".repeat(300)).len(), 120);
    }

    #[test]
    fn never_overwrites_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("preku-unique-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a.txt"));
        std::fs::write(dir.join("a.txt"), b"1").unwrap();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a (1).txt"));
        std::fs::write(dir.join("a (1).txt"), b"2").unwrap();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a (2).txt"));
        std::fs::write(dir.join("README"), b"3").unwrap();
        assert_eq!(unique_path(&dir, "README"), dir.join("README (1)"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn counts_chunks() {
        assert_eq!(chunk_count(0), 1);
        assert_eq!(chunk_count(1), 1);
        assert_eq!(chunk_count(CHUNK as i64), 1);
        assert_eq!(chunk_count(CHUNK as i64 + 1), 2);
        assert_eq!(chunk_count(10 * CHUNK as i64), 10);
    }
}
