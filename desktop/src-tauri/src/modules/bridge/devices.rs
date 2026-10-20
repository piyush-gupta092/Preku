//! Paired phones (migration 0007). Identified by their certificate fingerprint.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use ts_rs::TS;

use crate::foundation::errors::Result;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    #[ts(skip)]
    pub cert_fp: String,
    #[ts(type = "number")]
    pub paired_at: i64,
    #[ts(type = "number | null")]
    pub last_seen: Option<i64>,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PairedDevice> {
    Ok(PairedDevice {
        id: r.get(0)?,
        name: r.get(1)?,
        cert_fp: r.get(2)?,
        paired_at: r.get(3)?,
        last_seen: r.get(4)?,
    })
}

const COLUMNS: &str = "id, name, cert_fp, paired_at, last_seen";

pub fn list(conn: &Connection) -> Result<Vec<PairedDevice>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM bridge_devices ORDER BY paired_at"
    ))?;
    let rows = stmt
        .query_map([], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn find_by_fp(conn: &Connection, cert_fp: &str) -> Result<Option<PairedDevice>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM bridge_devices WHERE cert_fp = ?1"),
            [cert_fp],
            row,
        )
        .optional()?)
}

/// Pairs (or re-pairs: a phone that pairs again keeps one entry, with the new name).
pub fn add(conn: &Connection, name: &str, cert_fp: &str, now: i64) -> Result<PairedDevice> {
    let name: String = name.trim().chars().take(60).collect();
    let name = if name.is_empty() {
        "Phone".to_owned()
    } else {
        name
    };
    conn.execute(
        "INSERT INTO bridge_devices (id, name, cert_fp, paired_at, last_seen, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4, ?4)
         ON CONFLICT(cert_fp) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
        params![uuid::Uuid::new_v4().to_string(), name, cert_fp, now],
    )?;
    Ok(find_by_fp(conn, cert_fp)?.expect("just inserted"))
}

pub fn touch(conn: &Connection, id: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE bridge_devices SET last_seen = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.execute("DELETE FROM bridge_devices WHERE id = ?1", [id])? > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    #[test]
    fn pair_find_and_forget() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                assert!(list(conn)?.is_empty());
                let phone = add(conn, "  Pixel 8 ", "fp1", 10)?;
                assert_eq!(phone.name, "Pixel 8");
                assert_eq!(find_by_fp(conn, "fp1")?.unwrap().id, phone.id);
                assert!(find_by_fp(conn, "fp2")?.is_none());
                // Pairing again with the same certificate updates, never duplicates.
                add(conn, "My Pixel", "fp1", 20)?;
                assert_eq!(list(conn)?.len(), 1);
                assert_eq!(list(conn)?[0].name, "My Pixel");
                touch(conn, &phone.id, 30)?;
                assert_eq!(list(conn)?[0].last_seen, Some(30));
                assert!(remove(conn, &phone.id)?);
                assert!(list(conn)?.is_empty());
                Ok(())
            })
            .unwrap();
    }
}
