//! The optional calendar (PRD A5): a read-only agenda from calendar links (`.ics` feeds, which
//! is also how Google Calendar and Outlook share a calendar privately) and local `.ics` files.
//!
//! Strictly optional (Rules.md §4): with no calendar added this does nothing at all, and makes
//! no network request. A calendar link can contain a private token, so links live in the OS
//! keychain; settings only keep the name and the host. Events are held in memory only (never
//! written to disk, never logged) and are dropped when a calendar is removed.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use ts_rs::TS;

use super::ics::{self, Occurrence};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::AppEvent,
        secrets,
        storage::now_millis,
    },
    live::{LiveItem, LivePriority},
    modules::ModuleContext,
};

pub const EVENT: &str = "productivity:calendar";
const KEY: &str = "calendar";
const LIVE_ID: &str = "productivity.calendar";
const REFRESH_EVERY: Duration = Duration::from_secs(15 * 60);
const TICK: Duration = Duration::from_secs(30);
const MAX_BYTES: usize = 5 * 1024 * 1024;
const MAX_SOURCES: usize = 8;
/// The "starting now" note stays this long after an event begins.
const NOW_WINDOW_MS: i64 = 5 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceKind {
    /// A calendar link (https / webcal); the address itself is in the keychain.
    Link,
    /// A local `.ics` file.
    File,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarSource {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    /// What to show: the file path, or just the link's host (never the full private link).
    pub location: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CalendarSettings {
    pub sources: Vec<CalendarSource>,
    /// Show the next event in the notch this many minutes before it starts (0 = don't).
    pub remind_min: u32,
    /// Say "Event" instead of the title in the notch (for shared screens).
    pub hide_titles: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            remind_min: 10,
            hide_titles: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgendaEvent {
    pub id: String,
    pub title: String,
    #[ts(type = "number")]
    pub start: i64,
    #[ts(type = "number")]
    pub end: i64,
    pub all_day: bool,
    pub source_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceStatus {
    pub id: String,
    /// When this calendar was last read successfully (unix ms).
    #[ts(type = "number | null")]
    pub last_ok: Option<i64>,
    /// A short, user-readable reason the last refresh failed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarState {
    pub settings: CalendarSettings,
    pub statuses: Vec<SourceStatus>,
    /// Today's events, in order (all-day first).
    pub today: Vec<AgendaEvent>,
    /// The next timed event that hasn't ended (today or tomorrow).
    pub next: Option<AgendaEvent>,
}

/// `webcal://…` is how calendar apps hand out https feeds.
pub fn normalize_link(link: &str) -> Result<reqwest::Url> {
    let link = link.trim();
    let link = match link
        .strip_prefix("webcal://")
        .or_else(|| link.strip_prefix("webcals://"))
    {
        Some(rest) => format!("https://{rest}"),
        None => link.to_owned(),
    };
    let url = reqwest::Url::parse(&link)
        .map_err(|_| PrekuError::InvalidInput("That doesn't look like a calendar link".into()))?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err(PrekuError::InvalidInput(
            "A calendar link starts with https:// or webcal://".into(),
        ));
    }
    Ok(url)
}

fn clean_name(name: &str, fallback: &str) -> String {
    let name: String = name.trim().chars().take(40).collect();
    if name.is_empty() {
        fallback.chars().take(40).collect()
    } else {
        name
    }
}

fn to_ms(local: NaiveDateTime) -> i64 {
    Local
        .from_local_datetime(&local)
        .earliest()
        .map_or(0, |time| time.timestamp_millis())
}

fn utc_to_local(utc: NaiveDateTime) -> NaiveDateTime {
    Local.from_utc_datetime(&utc).naive_local()
}

/// Today's agenda and the next timed event, from each calendar's text. Pure.
pub fn agenda(
    calendars: &[(&str, &str)],
    now: NaiveDateTime,
    to_local: ics::ToLocal<'_>,
    to_ms: &dyn Fn(NaiveDateTime) -> i64,
) -> (Vec<AgendaEvent>, Option<AgendaEvent>) {
    let midnight = now.date().and_time(chrono::NaiveTime::MIN);
    let tomorrow = midnight + chrono::Duration::days(1);
    let window_end = midnight + chrono::Duration::days(2);
    let mut all: Vec<(Occurrence, &str)> = calendars
        .iter()
        .flat_map(|(name, text)| {
            ics::occurrences(text, midnight, window_end, to_local)
                .into_iter()
                .map(move |o| (o, *name))
        })
        .collect();
    all.sort_by(|a, b| {
        (!a.0.all_day, a.0.start, &a.0.title).cmp(&(!b.0.all_day, b.0.start, &b.0.title))
    });
    let view = |(o, name): &(Occurrence, &str)| AgendaEvent {
        id: format!("{}@{}", o.uid, to_ms(o.start)),
        title: if o.title.is_empty() {
            "(no title)".to_owned()
        } else {
            o.title.clone()
        },
        start: to_ms(o.start),
        end: to_ms(o.end),
        all_day: o.all_day,
        source_name: (*name).to_owned(),
    };
    let today = all
        .iter()
        .filter(|(o, _)| o.start < tomorrow)
        .map(view)
        .collect();
    let next = all
        .iter()
        .filter(|(o, _)| !o.all_day && (o.end > now || (o.end == o.start && o.start >= now)))
        .min_by_key(|(o, _)| o.start)
        .map(view);
    (today, next)
}

/// What the notch says about the next event, if it is close enough: (title, subtitle).
pub fn reminder(
    next: &AgendaEvent,
    now_ms: i64,
    settings: &CalendarSettings,
) -> Option<(String, String)> {
    if settings.remind_min == 0 {
        return None;
    }
    let until = next.start - now_ms;
    if until > i64::from(settings.remind_min) * 60_000 || until < -NOW_WINDOW_MS {
        return None;
    }
    let title = if settings.hide_titles {
        "📅".to_owned()
    } else {
        format!("📅 {}", next.title)
    };
    let minutes = (until + 59_999).div_euclid(60_000);
    let when = if minutes <= 0 {
        "now".to_owned()
    } else {
        format!("in {minutes} min")
    };
    Some((title, when))
}

#[derive(Default)]
struct Cached {
    text: Option<String>,
    last_ok: Option<i64>,
    error: Option<String>,
}

pub struct Calendar {
    ctx: ModuleContext,
    settings: Mutex<CalendarSettings>,
    cache: Mutex<HashMap<String, Cached>>,
    last_state: Mutex<Option<CalendarState>>,
    wake: tokio::sync::Notify,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn secret_key(id: &str) -> String {
    format!("calendar.url.{id}")
}

impl Calendar {
    pub fn load(ctx: ModuleContext) -> Arc<Self> {
        let settings = ctx
            .storage
            .settings_get(KEY)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        Arc::new(Self {
            ctx,
            settings: Mutex::new(settings),
            cache: Mutex::new(HashMap::new()),
            last_state: Mutex::new(None),
            wake: tokio::sync::Notify::new(),
        })
    }

    /// Refreshes calendars now and then, and keeps the agenda and the notch note current.
    /// Idle (no timers worth mentioning, no network) while no calendar is added.
    pub fn spawn(self: &Arc<Self>) -> JoinHandle<()> {
        let calendar = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut since_refresh = REFRESH_EVERY;
            loop {
                let has_sources = lock(&calendar.settings).sources.iter().any(|s| s.enabled);
                if has_sources && since_refresh >= REFRESH_EVERY {
                    calendar.refresh().await;
                    since_refresh = Duration::ZERO;
                }
                calendar.publish();
                tokio::select! {
                    () = tokio::time::sleep(TICK) => since_refresh += TICK,
                    // A calendar was added or "Refresh" pressed.
                    () = calendar.wake.notified() => since_refresh = REFRESH_EVERY,
                }
            }
        })
    }

    pub fn stop(&self) {
        self.ctx
            .publish(AppEvent::LiveRemove { id: LIVE_ID.into() });
    }

    async fn refresh(&self) {
        let sources: Vec<CalendarSource> = lock(&self.settings)
            .sources
            .iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect();
        for source in sources {
            let result = match source.kind {
                SourceKind::File => read_file(&source.location).await,
                SourceKind::Link => fetch_link(&source.id).await,
            };
            let mut cache = lock(&self.cache);
            let entry = cache.entry(source.id.clone()).or_default();
            match result {
                Ok(text) => {
                    entry.text = Some(text);
                    entry.last_ok = Some(now_millis());
                    entry.error = None;
                }
                // Keep showing what we had; say what went wrong (never the link or contents).
                Err(reason) => entry.error = Some(reason),
            }
        }
    }

    pub fn state(&self) -> CalendarState {
        let settings = lock(&self.settings).clone();
        let cache = lock(&self.cache);
        let calendars: Vec<(&str, &str)> = settings
            .sources
            .iter()
            .filter(|s| s.enabled)
            .filter_map(|s| Some((s.name.as_str(), cache.get(&s.id)?.text.as_deref()?)))
            .collect();
        let (today, next) = agenda(
            &calendars,
            Local::now().naive_local(),
            &utc_to_local,
            &to_ms,
        );
        let statuses = settings
            .sources
            .iter()
            .map(|s| {
                let cached = cache.get(&s.id);
                SourceStatus {
                    id: s.id.clone(),
                    last_ok: cached.and_then(|c| c.last_ok),
                    error: cached.and_then(|c| c.error.clone()),
                }
            })
            .collect();
        CalendarState {
            settings,
            statuses,
            today,
            next,
        }
    }

    /// Sends the state when it changed, and keeps the notch note in step.
    fn publish(&self) {
        let state = self.state();
        let note = state
            .next
            .as_ref()
            .and_then(|next| reminder(next, now_millis(), &state.settings));
        self.ctx.publish(match note {
            Some((title, when)) => AppEvent::LiveUpsert(
                LiveItem::new(LIVE_ID, super::ID, LivePriority::Action, title, Some(when))
                    .with_kind("calendar"),
            ),
            None => AppEvent::LiveRemove { id: LIVE_ID.into() },
        });
        let mut last = lock(&self.last_state);
        if last.as_ref() != Some(&state) {
            self.ctx.emit(EVENT, &state);
            *last = Some(state);
        }
    }

    fn save(&self, settings: &CalendarSettings) -> Result<()> {
        self.ctx
            .storage
            .settings_set(KEY, &serde_json::to_value(settings)?)
    }

    fn changed(&self) -> CalendarState {
        self.wake.notify_one();
        self.publish();
        self.state()
    }

    fn add(&self, source: CalendarSource) -> Result<CalendarState> {
        let mut settings = lock(&self.settings).clone();
        if settings.sources.len() >= MAX_SOURCES {
            return Err(PrekuError::InvalidInput(format!(
                "You can add up to {MAX_SOURCES} calendars"
            )));
        }
        settings.sources.push(source);
        self.save(&settings)?;
        *lock(&self.settings) = settings;
        Ok(self.changed())
    }

    /// Adds a calendar link. The link goes to the keychain; settings keep only its host.
    pub fn add_link(&self, name: &str, link: &str) -> Result<CalendarState> {
        let url = normalize_link(link)?;
        let host = url.host_str().unwrap_or_default().to_owned();
        let id = uuid::Uuid::new_v4().to_string();
        secrets::set(&secret_key(&id), url.as_str())?;
        let source = CalendarSource {
            name: clean_name(name, &host),
            kind: SourceKind::Link,
            location: host,
            enabled: true,
            id: id.clone(),
        };
        self.add(source).inspect_err(|_| {
            let _ = secrets::delete(&secret_key(&id));
        })
    }

    pub fn add_file(&self, name: &str, path: &std::path::Path) -> Result<CalendarState> {
        let is_ics = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ics"));
        if !is_ics || !path.is_file() {
            return Err(PrekuError::InvalidInput(
                "Choose a calendar file ending in .ics".into(),
            ));
        }
        let fallback = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Calendar");
        self.add(CalendarSource {
            id: uuid::Uuid::new_v4().to_string(),
            name: clean_name(name, fallback),
            kind: SourceKind::File,
            location: path.display().to_string(),
            enabled: true,
        })
    }

    /// Removes a calendar: its saved link and its events are gone at once.
    pub fn remove(&self, id: &str) -> Result<CalendarState> {
        let mut settings = lock(&self.settings).clone();
        settings.sources.retain(|s| s.id != id);
        self.save(&settings)?;
        *lock(&self.settings) = settings;
        lock(&self.cache).remove(id);
        secrets::delete(&secret_key(id))?;
        Ok(self.changed())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<CalendarState> {
        let mut settings = lock(&self.settings).clone();
        let source = settings
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| {
                PrekuError::InvalidInput("That calendar isn't in the list any more".into())
            })?;
        source.enabled = enabled;
        self.save(&settings)?;
        *lock(&self.settings) = settings;
        if !enabled {
            lock(&self.cache).remove(id);
        }
        Ok(self.changed())
    }

    pub fn set_options(&self, remind_min: u32, hide_titles: bool) -> Result<CalendarState> {
        if remind_min > 120 {
            return Err(PrekuError::InvalidInput(
                "The reminder can be up to 120 minutes before".into(),
            ));
        }
        let mut settings = lock(&self.settings).clone();
        settings.remind_min = remind_min;
        settings.hide_titles = hide_titles;
        self.save(&settings)?;
        *lock(&self.settings) = settings;
        self.publish();
        Ok(self.state())
    }

    pub fn refresh_now(&self) -> CalendarState {
        self.changed()
    }
}

async fn read_file(path: &str) -> std::result::Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| "The file can't be read (moved or deleted?)".to_owned())?;
    if bytes.len() > MAX_BYTES {
        return Err("The file is too large".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn fetch_link(id: &str) -> std::result::Result<String, String> {
    let link = secrets::get(&secret_key(id))
        .ok()
        .flatten()
        .ok_or_else(|| {
            "The saved link is missing: remove this calendar and add it again".to_owned()
        })?;
    let client = reqwest::Client::builder()
        .user_agent(concat!("Preku/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Couldn't start the download".to_owned())?;
    // Errors are reduced to a plain reason: a reqwest error text could include the private link.
    let mut response = client.get(link).send().await.map_err(|err| {
        if err.is_timeout() {
            "The calendar took too long to answer"
        } else {
            "Couldn't reach the calendar (offline?)"
        }
        .to_owned()
    })?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 | 404 => {
                "The calendar link was refused: it may have been reset or removed".into()
            }
            code => format!("The calendar answered with an error ({code})"),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The download was interrupted".to_owned())?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("The calendar is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("That link isn't a calendar (.ics) feed".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, d)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    /// Tests pretend local time is UTC, so results don't depend on this PC's time zone.
    fn same(time: NaiveDateTime) -> NaiveDateTime {
        time
    }

    fn ms(time: NaiveDateTime) -> i64 {
        time.and_utc().timestamp_millis()
    }

    const WORK: &str = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:standup
SUMMARY:Stand-up
DTSTART:20261012T100000
DTEND:20261012T101500
RRULE:FREQ=DAILY
END:VEVENT
BEGIN:VEVENT
UID:review
SUMMARY:Design review
DTSTART:20261012T150000
DTEND:20261012T160000
END:VEVENT
END:VCALENDAR";
    const HOME: &str = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:bday
SUMMARY:Asha's birthday
DTSTART;VALUE=DATE:20261012
END:VEVENT
END:VCALENDAR";

    #[test]
    fn agenda_merges_calendars_and_finds_the_next_event() {
        let calendars = [("Work", WORK), ("Home", HOME)];
        let (today, next) = agenda(&calendars, at(12, 9, 0), &same, &ms);
        let titles: Vec<_> = today
            .iter()
            .map(|e| (e.title.as_str(), e.source_name.as_str()))
            .collect();
        assert_eq!(
            titles,
            [
                ("Asha's birthday", "Home"),
                ("Stand-up", "Work"),
                ("Design review", "Work")
            ]
        );
        assert!(today[0].all_day);
        assert_eq!(next.unwrap().title, "Stand-up");

        // During the stand-up it is still "next" (it hasn't ended); after it, the review is.
        assert_eq!(
            agenda(&calendars, at(12, 10, 5), &same, &ms)
                .1
                .unwrap()
                .title,
            "Stand-up"
        );
        assert_eq!(
            agenda(&calendars, at(12, 10, 15), &same, &ms)
                .1
                .unwrap()
                .title,
            "Design review"
        );
        // In the evening, tomorrow's stand-up is next, but it isn't on today's list.
        let (today, next) = agenda(&calendars, at(12, 18, 0), &same, &ms);
        assert_eq!(today.len(), 3);
        let next = next.unwrap();
        assert_eq!(
            (next.title.as_str(), next.start),
            ("Stand-up", ms(at(13, 10, 0)))
        );
        // No calendars: nothing, and no work.
        assert_eq!(agenda(&[], at(12, 9, 0), &same, &ms), (Vec::new(), None));
    }

    #[test]
    fn the_notch_note_appears_only_close_to_the_event() {
        let (_, next) = agenda(&[("Work", WORK)], at(12, 9, 0), &same, &ms);
        let next = next.unwrap();
        let settings = CalendarSettings::default();
        let note = |h: u32, m: u32, settings: &CalendarSettings| {
            reminder(&next, ms(at(12, h, m)), settings)
        };
        assert_eq!(note(9, 49, &settings), None, "more than 10 minutes away");
        assert_eq!(
            note(9, 50, &settings),
            Some(("📅 Stand-up".into(), "in 10 min".into()))
        );
        assert_eq!(
            note(9, 59, &settings),
            Some(("📅 Stand-up".into(), "in 1 min".into()))
        );
        assert_eq!(
            note(10, 0, &settings),
            Some(("📅 Stand-up".into(), "now".into()))
        );
        assert_eq!(
            note(10, 6, &settings),
            None,
            "five minutes in, the note goes"
        );

        let private = CalendarSettings {
            hide_titles: true,
            ..CalendarSettings::default()
        };
        assert_eq!(
            note(9, 55, &private),
            Some(("📅".into(), "in 5 min".into()))
        );
        let off = CalendarSettings {
            remind_min: 0,
            ..CalendarSettings::default()
        };
        assert_eq!(note(9, 59, &off), None);
    }

    #[test]
    fn calendar_links_are_checked() {
        assert_eq!(
            normalize_link(" webcal://calendar.example.com/private/basic.ics ")
                .unwrap()
                .as_str(),
            "https://calendar.example.com/private/basic.ics"
        );
        assert!(
            normalize_link("https://outlook.office365.com/owa/calendar/x/calendar.ics").is_ok()
        );
        for bad in [
            "",
            "calendar.ics",
            "file:///C:/cal.ics",
            "ftp://example.com/a.ics",
            "javascript:alert(1)",
        ] {
            assert!(normalize_link(bad).is_err(), "{bad}");
        }
        assert_eq!(
            clean_name("  ", "calendar.google.com"),
            "calendar.google.com"
        );
        assert_eq!(clean_name(&"x".repeat(80), "f").len(), 40);
    }

    #[test]
    fn settings_start_empty_and_survive_old_json() {
        let settings = CalendarSettings::default();
        assert!(settings.sources.is_empty());
        let old: CalendarSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, settings);
    }
}
