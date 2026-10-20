//! Clipboard history (PRD A3): text and images, the last 50 unpinned items plus pinned ones.
//! Local only — clipboard content is never synced or logged (Rules.md §4).

use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

/// Unpinned items kept; older ones are dropped.
pub const MAX_ITEMS: usize = 50;
const MAX_TEXT_BYTES: usize = 1_000_000;
/// ~ an 8K screenshot; bigger images are skipped.
const MAX_IMAGE_PIXELS: usize = 40_000_000;
const PREVIEW_CHARS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ClipKind {
    Text,
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipItem {
    pub id: String,
    pub kind: ClipKind,
    /// First characters of a text item; empty for images.
    pub preview: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pinned: bool,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Something read from the system clipboard.
#[derive(Debug, Clone, PartialEq)]
pub enum Captured {
    Text(String),
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
}

/// Recording policy. `sensitive` is true when the copying app marked the content as private
/// (password managers do); such content is never stored.
pub fn should_record(captured: &Captured, sensitive: bool) -> bool {
    if sensitive {
        return false;
    }
    match captured {
        Captured::Text(text) => !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES,
        Captured::Image {
            width,
            height,
            rgba,
        } => {
            let pixels = width * height;
            pixels > 0 && pixels <= MAX_IMAGE_PIXELS && rgba.len() == pixels * 4
        }
    }
}

/// FNV-1a: tiny, stable across Rust versions (unlike `DefaultHasher`), fine for de-duplication.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Identifies identical content, so copying the same thing again moves it to the top.
pub fn fingerprint(captured: &Captured) -> String {
    match captured {
        Captured::Text(text) => format!("t:{}:{:016x}", text.len(), fnv1a64(text.as_bytes())),
        Captured::Image {
            width,
            height,
            rgba,
        } => format!("i:{width}x{height}:{:016x}", fnv1a64(rgba)),
    }
}

fn io_err(err: impl std::error::Error + Send + Sync + 'static) -> PrekuError {
    PrekuError::Io(std::io::Error::other(err))
}

fn write_png(path: &Path, width: usize, height: usize, rgba: &[u8]) -> Result<()> {
    let to_u32 = |v: usize| u32::try_from(v).map_err(io_err);
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, to_u32(width)?, to_u32(height)?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba))
        .map_err(io_err)
}

/// Decodes an image this module wrote (always 8-bit RGBA).
pub fn read_png(path: &Path) -> Result<(usize, usize, Vec<u8>)> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info().map_err(io_err)?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(io_err)?;
    buf.truncate(info.buffer_size());
    Ok((info.width as usize, info.height as usize, buf))
}

/// Clipboard history stored in SQLite, with images as PNG files in `image_dir`.
pub struct ClipStore {
    image_dir: PathBuf,
}

impl ClipStore {
    pub fn new(image_dir: PathBuf) -> Self {
        Self { image_dir }
    }

    fn image_path(&self, file: &str) -> PathBuf {
        self.image_dir.join(file)
    }

    /// Stores a capture (or bumps an identical one to the top) and trims old items.
    pub fn record(&self, conn: &Connection, captured: &Captured, now: i64) -> Result<()> {
        let fingerprint = fingerprint(captured);
        let bumped = conn.execute(
            "UPDATE clipboard_items SET created_at = ?2 WHERE fingerprint = ?1",
            params![fingerprint, now],
        )?;
        if bumped == 0 {
            let id = uuid::Uuid::new_v4().to_string();
            match captured {
                Captured::Text(text) => {
                    conn.execute(
                        "INSERT INTO clipboard_items (id, kind, text, fingerprint, created_at)
                         VALUES (?1, 'text', ?2, ?3, ?4)",
                        params![id, text, fingerprint, now],
                    )?;
                }
                Captured::Image {
                    width,
                    height,
                    rgba,
                } => {
                    std::fs::create_dir_all(&self.image_dir)?;
                    let file = format!("{id}.png");
                    write_png(&self.image_path(&file), *width, *height, rgba)?;
                    conn.execute(
                        "INSERT INTO clipboard_items
                           (id, kind, image_file, width, height, fingerprint, created_at)
                         VALUES (?1, 'image', ?2, ?3, ?4, ?5, ?6)",
                        params![id, file, *width as i64, *height as i64, fingerprint, now],
                    )?;
                }
            }
        }
        self.trim(conn)
    }

    /// Drops unpinned items beyond MAX_ITEMS, including their image files.
    fn trim(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare(
            "SELECT id, image_file FROM clipboard_items WHERE pinned = 0
             ORDER BY created_at DESC LIMIT -1 OFFSET ?1",
        )?;
        let old: Vec<(String, Option<String>)> = stmt
            .query_map([MAX_ITEMS as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, file) in old {
            self.remove(conn, &id, file.as_deref())?;
        }
        Ok(())
    }

    fn remove(&self, conn: &Connection, id: &str, image_file: Option<&str>) -> Result<()> {
        conn.execute("DELETE FROM clipboard_items WHERE id = ?1", [id])?;
        if let Some(file) = image_file {
            // Already gone is fine; anything else is worth a warning but not a failure.
            if let Err(err) = std::fs::remove_file(self.image_path(file)) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(error = %err, "failed to delete clipboard image");
                }
            }
        }
        Ok(())
    }

    /// Pinned first, then newest first.
    pub fn list(&self, conn: &Connection) -> Result<Vec<ClipItem>> {
        let mut stmt = conn.prepare(
            "SELECT id, kind, COALESCE(substr(text, 1, ?1), ''), width, height, pinned, created_at
             FROM clipboard_items ORDER BY pinned DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([PREVIEW_CHARS], |row| {
            let kind: String = row.get(1)?;
            Ok(ClipItem {
                id: row.get(0)?,
                kind: if kind == "image" {
                    ClipKind::Image
                } else {
                    ClipKind::Text
                },
                preview: row.get(2)?,
                width: row.get(3)?,
                height: row.get(4)?,
                pinned: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Full content of an item, for copying it back.
    pub fn content(&self, conn: &Connection, id: &str) -> Result<Captured> {
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT text, image_file FROM clipboard_items WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match row {
            Some((Some(text), _)) => Ok(Captured::Text(text)),
            Some((None, Some(file))) => {
                let (width, height, rgba) = read_png(&self.image_path(&file))?;
                Ok(Captured::Image {
                    width,
                    height,
                    rgba,
                })
            }
            _ => Err(PrekuError::InvalidInput(format!("No clipboard item {id}"))),
        }
    }

    /// `data:image/png;base64,…` for showing an image item in the UI.
    pub fn image_data_url(&self, conn: &Connection, id: &str) -> Result<String> {
        let file: Option<String> = conn
            .query_row(
                "SELECT image_file FROM clipboard_items WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let file = file.ok_or_else(|| PrekuError::InvalidInput(format!("No image {id}")))?;
        let bytes = std::fs::read(self.image_path(&file))?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }

    pub fn set_pinned(&self, conn: &Connection, id: &str, pinned: bool) -> Result<()> {
        conn.execute(
            "UPDATE clipboard_items SET pinned = ?2 WHERE id = ?1",
            params![id, pinned],
        )?;
        self.trim(conn)
    }

    pub fn delete(&self, conn: &Connection, id: &str) -> Result<()> {
        let file: Option<String> = conn
            .query_row(
                "SELECT image_file FROM clipboard_items WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        self.remove(conn, id, file.as_deref())
    }

    /// Removes everything that isn't pinned.
    pub fn clear_unpinned(&self, conn: &Connection) -> Result<()> {
        let mut stmt =
            conn.prepare("SELECT id, image_file FROM clipboard_items WHERE pinned = 0")?;
        let items: Vec<(String, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, file) in items {
            self.remove(conn, &id, file.as_deref())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    fn text(s: &str) -> Captured {
        Captured::Text(s.into())
    }

    fn with_store(test: impl FnOnce(&ClipStore, &Connection)) {
        let dir = std::env::temp_dir().join(format!("preku-clip-{}", uuid::Uuid::new_v4()));
        let store = ClipStore::new(dir.clone());
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                test(&store, conn);
                Ok(())
            })
            .unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn policy_skips_sensitive_empty_and_malformed_content() {
        assert!(should_record(&text("hello"), false));
        assert!(!should_record(&text("hunter2"), true), "password managers");
        assert!(!should_record(&text("   \n"), false));
        assert!(!should_record(
            &text(&"x".repeat(MAX_TEXT_BYTES + 1)),
            false
        ));
        let bad_image = Captured::Image {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        };
        assert!(!should_record(&bad_image, false));
    }

    #[test]
    fn same_content_moves_to_top_instead_of_duplicating() {
        with_store(|store, conn| {
            store.record(conn, &text("a"), 1).unwrap();
            store.record(conn, &text("b"), 2).unwrap();
            store.record(conn, &text("a"), 3).unwrap();
            let previews: Vec<_> = store
                .list(conn)
                .unwrap()
                .into_iter()
                .map(|i| i.preview)
                .collect();
            assert_eq!(previews, ["a", "b"]);
        });
    }

    #[test]
    fn keeps_fifty_unpinned_plus_pinned() {
        with_store(|store, conn| {
            store.record(conn, &text("keep me"), 0).unwrap();
            let pinned_id = store.list(conn).unwrap()[0].id.clone();
            store.set_pinned(conn, &pinned_id, true).unwrap();
            for i in 1..=60 {
                store.record(conn, &text(&format!("item {i}")), i).unwrap();
            }
            let items = store.list(conn).unwrap();
            assert_eq!(items.len(), MAX_ITEMS + 1);
            assert_eq!(items[0].preview, "keep me", "pinned stays on top");
            assert_eq!(items[1].preview, "item 60");
            assert!(items.iter().all(|i| i.preview != "item 10"));

            store.clear_unpinned(conn).unwrap();
            assert_eq!(store.list(conn).unwrap().len(), 1);
        });
    }

    #[test]
    fn images_round_trip_through_png_files() {
        with_store(|store, conn| {
            let rgba: Vec<u8> = (0..2 * 3 * 4).map(|v| v as u8).collect();
            let image = Captured::Image {
                width: 2,
                height: 3,
                rgba: rgba.clone(),
            };
            store.record(conn, &image, 1).unwrap();
            let item = &store.list(conn).unwrap()[0];
            assert_eq!(item.kind, ClipKind::Image);
            assert_eq!((item.width, item.height), (Some(2), Some(3)));

            assert_eq!(store.content(conn, &item.id).unwrap(), image);
            assert!(store
                .image_data_url(conn, &item.id)
                .unwrap()
                .starts_with("data:image/png;base64,"));

            store.delete(conn, &item.id).unwrap();
            assert!(store.list(conn).unwrap().is_empty());
        });
    }

    #[test]
    fn preview_is_truncated_but_content_is_complete() {
        with_store(|store, conn| {
            let long = "y".repeat(1000);
            store.record(conn, &text(&long), 1).unwrap();
            let item = &store.list(conn).unwrap()[0];
            assert_eq!(item.preview.len(), PREVIEW_CHARS as usize);
            assert_eq!(store.content(conn, &item.id).unwrap(), text(&long));
        });
    }
}
