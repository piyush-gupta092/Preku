//! Watched targets: GitHub repositories (CI) and URLs (uptime), stored in `watch_targets`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

/// Uptime checks run at most this often (Rules.md §5).
pub const MIN_UPTIME_INTERVAL_S: u32 = 30;
pub const GITHUB_INTERVAL_S: u32 = 60;
const MAX_TARGETS_PER_KIND: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TargetKind {
    Github,
    Uptime,
}

impl TargetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Uptime => "uptime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchTarget {
    pub id: String,
    pub kind: TargetKind,
    /// `owner/repo` or an http(s) URL.
    pub target: String,
    pub interval_s: u32,
}

/// `owner/repo`, also accepting a pasted `https://github.com/owner/repo(.git)` URL.
pub fn normalize_repo(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('/');
    let path = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("github.com/"))
        .unwrap_or(trimmed);
    let path = path.strip_suffix(".git").unwrap_or(path);
    let valid_part = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match path.split('/').collect::<Vec<_>>()[..] {
        [owner, repo] if valid_part(owner) && valid_part(repo) => Ok(format!("{owner}/{repo}")),
        _ => Err(PrekuError::InvalidInput(
            "Use the form owner/repository".into(),
        )),
    }
}

pub fn normalize_url(input: &str) -> Result<String> {
    let url = input.trim();
    let has_scheme = url.starts_with("https://") || url.starts_with("http://");
    let url = if has_scheme {
        url.to_owned()
    } else {
        format!("https://{url}")
    };
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    if host.is_empty() || host.contains(' ') || url.len() > 500 {
        return Err(PrekuError::InvalidInput(
            "That doesn't look like a web address".into(),
        ));
    }
    Ok(url)
}

pub fn list(conn: &Connection, kind: TargetKind) -> Result<Vec<WatchTarget>> {
    let mut stmt = conn.prepare(
        "SELECT id, target, interval_s FROM watch_targets WHERE kind = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map([kind.as_str()], |row| {
        Ok(WatchTarget {
            id: row.get(0)?,
            kind,
            target: row.get(1)?,
            interval_s: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn add(conn: &Connection, kind: TargetKind, input: &str, now: i64) -> Result<WatchTarget> {
    let (target, interval_s) = match kind {
        TargetKind::Github => (normalize_repo(input)?, GITHUB_INTERVAL_S),
        TargetKind::Uptime => (normalize_url(input)?, MIN_UPTIME_INTERVAL_S * 2),
    };
    if list(conn, kind)?.len() >= MAX_TARGETS_PER_KIND {
        return Err(PrekuError::InvalidInput(format!(
            "You can watch up to {MAX_TARGETS_PER_KIND}"
        )));
    }
    let item = WatchTarget {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        target,
        interval_s,
    };
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO watch_targets (id, kind, target, interval_s, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![item.id, kind.as_str(), item.target, item.interval_s, now],
    )?;
    if inserted == 0 {
        return Err(PrekuError::InvalidInput("Already on the list".into()));
    }
    Ok(item)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM watch_targets WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    #[test]
    fn repo_forms() {
        assert_eq!(
            normalize_repo("tauri-apps/tauri").unwrap(),
            "tauri-apps/tauri"
        );
        assert_eq!(
            normalize_repo(" https://github.com/rust-lang/rust.git/ ").unwrap(),
            "rust-lang/rust"
        );
        assert!(normalize_repo("just-a-name").is_err());
        assert!(normalize_repo("a/b/c").is_err());
        assert!(normalize_repo("bad owner/x").is_err());
    }

    #[test]
    fn url_forms() {
        assert_eq!(normalize_url("example.com").unwrap(), "https://example.com");
        assert_eq!(
            normalize_url("http://localhost:3000/health").unwrap(),
            "http://localhost:3000/health"
        );
        assert!(normalize_url("https://").is_err());
        assert!(normalize_url("not a url").is_err());
    }

    #[test]
    fn add_list_dedupe_delete() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                let a = add(conn, TargetKind::Github, "o/r", 1)?;
                assert!(add(conn, TargetKind::Github, "https://github.com/o/r", 2).is_err());
                add(conn, TargetKind::Uptime, "example.com", 3)?;
                assert_eq!(list(conn, TargetKind::Github)?.len(), 1);
                assert_eq!(list(conn, TargetKind::Uptime)?[0].interval_s, 60);
                delete(conn, &a.id)?;
                assert!(list(conn, TargetKind::Github)?.is_empty());
                Ok(())
            })
            .unwrap();
    }
}
