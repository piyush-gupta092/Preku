//! Live items: the short, glanceable things the compact pill shows (Architecture.md §3.3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ordered lowest → highest; the pill shows the highest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LivePriority {
    Info,
    Media,
    Timer,
    Action,
    Alert,
    IncomingCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveItem {
    /// Unique per item; publishing the same id again replaces it.
    pub id: String,
    /// Module id, e.g. `productivity`. Also picks the accent color.
    pub module: String,
    pub priority: LivePriority,
    pub title: String,
    pub subtitle: Option<String>,
    /// 0…1 for a progress ring, if any.
    pub progress: Option<f32>,
    /// Unix epoch ms when the item disappears on its own.
    #[ts(type = "number | null")]
    pub expires_at: Option<i64>,
    /// Unix epoch ms.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Optional message key (e.g. `reminder.eyes`). The UI shows its own translated text for
    /// known kinds, so Rust never has to produce words (Rules.md §1, i18n).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub kind: Option<String>,
}

impl LiveItem {
    /// A new item stamped with the current time; set `progress`/`expires_at` afterwards.
    pub fn new(
        id: impl Into<String>,
        module: &str,
        priority: LivePriority,
        title: impl Into<String>,
        subtitle: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            module: module.into(),
            priority,
            title: title.into(),
            subtitle,
            progress: None,
            expires_at: None,
            created_at: crate::foundation::storage::now_millis(),
            kind: None,
        }
    }

    /// Builder-style translation key for the UI.
    pub fn with_kind(mut self, kind: &str) -> Self {
        self.kind = Some(kind.to_owned());
        self
    }

    /// Builder-style expiry, `ms` from now.
    pub fn expires_in(mut self, ms: i64) -> Self {
        self.expires_at = Some(self.created_at + ms);
        self
    }
}

#[derive(Debug, Default)]
pub struct LiveQueue {
    items: Vec<LiveItem>,
}

impl LiveQueue {
    pub fn upsert(&mut self, item: LiveItem) {
        match self
            .items
            .iter_mut()
            .find(|existing| existing.id == item.id)
        {
            Some(existing) => *existing = item,
            None => self.items.push(item),
        }
    }

    /// Returns whether anything was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        self.items.len() != before
    }

    /// Removes every item a module published. Returns whether anything was removed.
    pub fn remove_module(&mut self, module: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.module != module);
        self.items.len() != before
    }

    /// Drops expired items. Returns whether anything was removed.
    pub fn prune(&mut self, now_ms: i64) -> bool {
        let before = self.items.len();
        self.items
            .retain(|item| !matches!(item.expires_at, Some(expires) if expires <= now_ms));
        self.items.len() != before
    }

    /// Highest priority first; newest first within the same priority.
    pub fn sorted(&self) -> Vec<LiveItem> {
        let mut items = self.items.clone();
        items.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(b.created_at.cmp(&a.created_at))
        });
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, priority: LivePriority, created_at: i64) -> LiveItem {
        LiveItem {
            id: id.into(),
            module: "demo".into(),
            priority,
            title: id.into(),
            subtitle: None,
            progress: None,
            expires_at: None,
            created_at,
            kind: None,
        }
    }

    #[test]
    fn sorts_by_priority_then_newest() {
        let mut queue = LiveQueue::default();
        queue.upsert(item("old-info", LivePriority::Info, 1));
        queue.upsert(item("alert", LivePriority::Alert, 2));
        queue.upsert(item("new-info", LivePriority::Info, 3));
        queue.upsert(item("call", LivePriority::IncomingCall, 0));
        let ids: Vec<_> = queue.sorted().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["call", "alert", "new-info", "old-info"]);
    }

    #[test]
    fn upsert_replaces_same_id() {
        let mut queue = LiveQueue::default();
        queue.upsert(item("timer", LivePriority::Timer, 1));
        let mut updated = item("timer", LivePriority::Timer, 1);
        updated.progress = Some(0.5);
        queue.upsert(updated);
        let items = queue.sorted();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].progress, Some(0.5));
    }

    #[test]
    fn prune_and_remove() {
        let mut queue = LiveQueue::default();
        let mut expiring = item("expiring", LivePriority::Info, 1);
        expiring.expires_at = Some(100);
        queue.upsert(expiring);
        queue.upsert(item("forever", LivePriority::Info, 1));

        assert!(!queue.prune(99));
        assert!(queue.prune(100));
        assert!(queue.remove("forever"));
        assert!(!queue.remove("forever"));
        assert!(queue.sorted().is_empty());
    }
}
