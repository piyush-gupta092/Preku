//! A small, read-only iCalendar (RFC 5545) reader for the optional calendar (PRD A5): turns
//! an `.ics` file or feed into the occurrences inside a time window, in local time. Pure and
//! dependency-free, so it is fully testable.
//!
//! Understood: folded lines, `VEVENT` with `DTSTART`/`DTEND`/`DURATION` (UTC, floating,
//! `TZID` resolved through the file's own `VTIMEZONE` blocks, and all-day `VALUE=DATE`),
//! `SUMMARY`, `STATUS:CANCELLED`, `RRULE` (DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT,
//! UNTIL, BYDAY, BYMONTHDAY), `EXDATE` and `RECURRENCE-ID` overrides.
//! Not understood (the rule part is ignored): BYSETPOS, BYWEEKNO, BYYEARDAY, hourly and
//! faster frequencies, RDATE.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// A hostile or broken feed can't make us loop or allocate without bound.
const MAX_EVENTS: usize = 20_000;
const MAX_STEPS: usize = 50_000;

/// One event occurrence, in the user's local time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub uid: String,
    pub title: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Zone {
    Utc,
    /// No zone given: the wall time is already local.
    Floating,
    Named(String),
    /// `VALUE=DATE`: a whole day.
    Date,
}

#[derive(Debug, Clone)]
struct When {
    wall: NaiveDateTime,
    zone: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone)]
struct Rule {
    freq: Freq,
    interval: u32,
    count: Option<u32>,
    until: Option<When>,
    /// (ordinal, weekday): ordinal 0 = every such weekday, 2 = the second, -1 = the last.
    by_day: Vec<(i32, Weekday)>,
    by_month_day: Vec<i32>,
}

#[derive(Debug, Clone)]
struct Event {
    uid: String,
    title: String,
    start: When,
    end: Option<When>,
    duration: Option<Duration>,
    rule: Option<Rule>,
    exdates: Vec<When>,
    recurrence_id: Option<When>,
    cancelled: bool,
}

/// One `STANDARD` or `DAYLIGHT` part of a `VTIMEZONE`.
#[derive(Debug, Clone)]
struct Observance {
    offset_s: i32,
    start: NaiveDateTime,
    /// Yearly switch: (month, ordinal, weekday), e.g. the last Sunday of March.
    yearly: Option<(u32, i32, Weekday)>,
}

type Zones = HashMap<String, Vec<Observance>>;

/// Converts a UTC wall time to the user's local wall time.
pub type ToLocal<'a> = &'a dyn Fn(NaiveDateTime) -> NaiveDateTime;

/// The occurrences that overlap `[from, to)` (local time), sorted by start.
pub fn occurrences(
    ics: &str,
    from: NaiveDateTime,
    to: NaiveDateTime,
    to_local: ToLocal<'_>,
) -> Vec<Occurrence> {
    let (events, zones) = parse(ics);
    let local = |when: &When| resolve(when, &zones, to_local);

    // Occurrences replaced or cancelled by a RECURRENCE-ID event.
    let overridden: HashSet<(String, NaiveDateTime)> = events
        .iter()
        .filter_map(|e| Some((e.uid.clone(), local(e.recurrence_id.as_ref()?))))
        .collect();

    let mut out = Vec::new();
    for event in &events {
        if event.cancelled {
            continue;
        }
        let all_day = event.start.zone == Zone::Date;
        let length = match (&event.end, event.duration) {
            (Some(end), _) => end.wall - event.start.wall,
            (None, Some(duration)) => duration,
            (None, None) if all_day => Duration::days(1),
            (None, None) => Duration::zero(),
        }
        .max(Duration::zero());
        let exdates: HashSet<NaiveDateTime> = event.exdates.iter().map(local).collect();

        let starts = match (&event.rule, &event.recurrence_id) {
            (Some(rule), None) => expand(event, rule, &zones, to_local, length, from, to),
            _ => vec![event.start.wall],
        };
        for wall in starts {
            let start = local(&When {
                wall,
                zone: event.start.zone.clone(),
            });
            let end = local(&When {
                wall: wall + length,
                zone: event.start.zone.clone(),
            });
            let is_series = event.rule.is_some() && event.recurrence_id.is_none();
            if exdates.contains(&start)
                || (is_series && overridden.contains(&(event.uid.clone(), start)))
            {
                continue;
            }
            // Zero-length events count at their start instant.
            if start < to && (end > from || (end == start && start >= from)) {
                out.push(Occurrence {
                    uid: event.uid.clone(),
                    title: event.title.clone(),
                    start,
                    end,
                    all_day,
                });
            }
        }
    }
    out.sort_by(|a, b| (a.start, &a.title).cmp(&(b.start, &b.title)));
    out.dedup();
    out
}

fn resolve(when: &When, zones: &Zones, to_local: ToLocal<'_>) -> NaiveDateTime {
    match &when.zone {
        Zone::Utc => to_local(when.wall),
        Zone::Floating | Zone::Date => when.wall,
        Zone::Named(name) => match zones.get(name) {
            Some(parts) => {
                to_local(when.wall - Duration::seconds(i64::from(offset_at(parts, when.wall))))
            }
            // A zone the file doesn't describe: the best guess is the user's own.
            None => when.wall,
        },
    }
}

/// The UTC offset (seconds) a zone uses at this wall time.
fn offset_at(parts: &[Observance], wall: NaiveDateTime) -> i32 {
    let onset = |part: &Observance| -> Option<NaiveDateTime> {
        let Some((month, ordinal, weekday)) = part.yearly else {
            return (part.start <= wall).then_some(part.start);
        };
        [wall.year(), wall.year() - 1]
            .into_iter()
            .filter_map(|year| nth_weekday(year, month, ordinal, weekday))
            .map(|date| date.and_time(part.start.time()))
            .find(|at| *at <= wall && *at >= part.start)
    };
    parts
        .iter()
        .filter_map(|part| Some((onset(part)?, part.offset_s)))
        .max_by_key(|(at, _)| *at)
        .map(|(_, offset)| offset)
        // Before every rule starts: the earliest part's offset.
        .or_else(|| parts.iter().min_by_key(|p| p.start).map(|p| p.offset_s))
        .unwrap_or(0)
}

/// The `ordinal`-th `weekday` of a month (`-1` = the last one).
fn nth_weekday(year: i32, month: u32, ordinal: i32, weekday: Weekday) -> Option<NaiveDate> {
    if ordinal > 0 {
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;
        let shift =
            (7 + weekday.num_days_from_monday() - first.weekday().num_days_from_monday()) % 7;
        let date = first + Duration::days(i64::from(shift) + 7 * i64::from(ordinal - 1));
        (date.month() == month).then_some(date)
    } else if ordinal < 0 {
        let last = last_day(year, month)?;
        let shift =
            (7 + last.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
        let date = last - Duration::days(i64::from(shift) + 7 * i64::from(-ordinal - 1));
        (date.month() == month).then_some(date)
    } else {
        None
    }
}

fn last_day(year: i32, month: u32) -> Option<NaiveDate> {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1).map(|d| d - Duration::days(1))
}

/// Wall-time starts of a repeating event that could overlap `[from, to)`.
fn expand(
    event: &Event,
    rule: &Rule,
    zones: &Zones,
    to_local: ToLocal<'_>,
    length: Duration,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> Vec<NaiveDateTime> {
    let first = event.start.wall;
    let time = first.time();
    let zone = &event.start.zone;
    let local = |wall: NaiveDateTime| {
        resolve(
            &When {
                wall,
                zone: zone.clone(),
            },
            zones,
            to_local,
        )
    };
    let until = rule.until.as_ref().map(|u| resolve(u, zones, to_local));
    let interval = i64::from(rule.interval.max(1));

    let mut out = Vec::new();
    let mut made = 0u32;
    let mut steps = 0usize;
    // `period` counts intervals from the first one; each yields its candidate days.
    let mut period = 0i64;
    'outer: loop {
        let mut days: Vec<NaiveDate> = match rule.freq {
            Freq::Daily => vec![first.date() + Duration::days(period * interval)],
            Freq::Weekly => {
                let monday = first.date()
                    - Duration::days(i64::from(first.weekday().num_days_from_monday()))
                    + Duration::weeks(period * interval);
                if rule.by_day.is_empty() {
                    vec![monday + Duration::days(i64::from(first.weekday().num_days_from_monday()))]
                } else {
                    rule.by_day
                        .iter()
                        .map(|(_, day)| {
                            monday + Duration::days(i64::from(day.num_days_from_monday()))
                        })
                        .collect()
                }
            }
            Freq::Monthly | Freq::Yearly => {
                let months = if rule.freq == Freq::Monthly {
                    period * interval
                } else {
                    12 * period * interval
                };
                let index = i64::from(first.year()) * 12 + i64::from(first.month0()) + months;
                let (year, month) = (
                    (index.div_euclid(12)) as i32,
                    index.rem_euclid(12) as u32 + 1,
                );
                if !rule.by_day.is_empty() {
                    rule.by_day
                        .iter()
                        .filter_map(|(ordinal, day)| nth_weekday(year, month, *ordinal, *day))
                        .collect()
                } else if !rule.by_month_day.is_empty() {
                    rule.by_month_day
                        .iter()
                        .filter_map(|&d| {
                            if d > 0 {
                                NaiveDate::from_ymd_opt(year, month, d as u32)
                            } else {
                                last_day(year, month)
                                    .map(|last| last + Duration::days(i64::from(d) + 1))
                            }
                        })
                        .collect()
                } else {
                    // A day that doesn't exist that month (the 31st, 29 Feb) is skipped.
                    NaiveDate::from_ymd_opt(year, month, first.day())
                        .into_iter()
                        .collect()
                }
            }
        };
        days.sort();
        days.dedup();
        for day in days {
            steps += 1;
            if steps > MAX_STEPS {
                break 'outer;
            }
            let wall = day.and_time(time);
            if wall < first {
                continue;
            }
            let start = local(wall);
            if until.is_some_and(|until| start > until) || rule.count.is_some_and(|n| made >= n) {
                break 'outer;
            }
            made += 1;
            if start >= to {
                // Later ones only start later, unless a COUNT still needs counting (it doesn't
                // matter any more: nothing after this is in the window).
                break 'outer;
            }
            if local(wall + length) > from || start >= from {
                out.push(wall);
            }
        }
        period += 1;
        if period > MAX_STEPS as i64 {
            break;
        }
    }
    out
}

// ------------------------------------------------------------------ parsing

struct Line {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

/// Unfolds and splits `NAME;PARAM=x:value` lines (a `:` inside quotes belongs to the param).
fn lines(ics: &str) -> Vec<Line> {
    let mut unfolded: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match raw.strip_prefix([' ', '\t']) {
            Some(rest) if !unfolded.is_empty() => {
                if let Some(last) = unfolded.last_mut() {
                    last.push_str(rest);
                }
            }
            _ => unfolded.push(raw.to_owned()),
        }
    }
    unfolded
        .iter()
        .filter_map(|line| {
            let mut quoted = false;
            let colon = line.char_indices().find_map(|(i, c)| {
                if c == '"' {
                    quoted = !quoted;
                }
                (c == ':' && !quoted).then_some(i)
            })?;
            let (head, value) = (&line[..colon], &line[colon + 1..]);
            let mut parts = head.split(';');
            let name = parts.next()?.trim().to_ascii_uppercase();
            let params = parts
                .filter_map(|p| p.split_once('='))
                .map(|(k, v)| {
                    (
                        k.trim().to_ascii_uppercase(),
                        v.trim().trim_matches('"').to_owned(),
                    )
                })
                .collect();
            Some(Line {
                name,
                params,
                value: value.to_owned(),
            })
        })
        .collect()
}

fn param<'a>(line: &'a Line, name: &str) -> Option<&'a str> {
    line.params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.trim().chars().take(200).collect()
}

fn date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.get(..8)?, "%Y%m%d").ok()
}

fn date_time(text: &str) -> Option<NaiveDateTime> {
    let text = text.trim().trim_end_matches('Z');
    if text.len() == 8 {
        return date(text).map(|d| d.and_time(NaiveTime::MIN));
    }
    NaiveDateTime::parse_from_str(text.get(..15)?, "%Y%m%dT%H%M%S").ok()
}

fn when(value: &str, tzid: Option<&str>, is_date: bool) -> Option<When> {
    let value = value.trim();
    let zone = if is_date || value.len() == 8 {
        Zone::Date
    } else if value.ends_with('Z') {
        Zone::Utc
    } else if let Some(tzid) = tzid {
        Zone::Named(tzid.to_owned())
    } else {
        Zone::Floating
    };
    Some(When {
        wall: date_time(value)?,
        zone,
    })
}

fn when_of(line: &Line) -> Option<When> {
    when(
        &line.value,
        param(line, "TZID"),
        param(line, "VALUE") == Some("DATE"),
    )
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

/// `2TU`, `-1FR`, `MO` → (ordinal, weekday).
fn by_day(text: &str) -> Vec<(i32, Weekday)> {
    text.split(',')
        .filter_map(|item| {
            let item = item.trim();
            let split = item.len().checked_sub(2)?;
            let ordinal = if split == 0 {
                0
            } else {
                item.get(..split)?.parse().ok()?
            };
            Some((ordinal, weekday(item.get(split..)?)?))
        })
        .collect()
}

fn rule(text: &str, tzid: Option<&str>) -> Option<Rule> {
    let parts: HashMap<String, &str> = text
        .split(';')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim()))
        .collect();
    let freq = match *parts.get("FREQ")? {
        "DAILY" => Freq::Daily,
        "WEEKLY" => Freq::Weekly,
        "MONTHLY" => Freq::Monthly,
        "YEARLY" => Freq::Yearly,
        _ => return None,
    };
    Some(Rule {
        freq,
        interval: parts
            .get("INTERVAL")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        count: parts.get("COUNT").and_then(|v| v.parse().ok()),
        until: parts.get("UNTIL").and_then(|v| when(v, tzid, false)),
        by_day: parts.get("BYDAY").map(|v| by_day(v)).unwrap_or_default(),
        by_month_day: parts
            .get("BYMONTHDAY")
            .map(|v| v.split(',').filter_map(|d| d.trim().parse().ok()).collect())
            .unwrap_or_default(),
    })
}

/// `PT1H30M`, `P1D`, `P2W`.
fn duration(text: &str) -> Option<Duration> {
    let text = text.trim().strip_prefix('P')?;
    let mut total = Duration::zero();
    let mut number = String::new();
    let mut in_time = false;
    for c in text.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                total += match (unit, in_time) {
                    ('W', false) => Duration::weeks(n),
                    ('D', false) => Duration::days(n),
                    ('H', true) => Duration::hours(n),
                    ('M', true) => Duration::minutes(n),
                    ('S', true) => Duration::seconds(n),
                    _ => return None,
                };
            }
        }
    }
    Some(total)
}

/// `+0530`, `-0800`, `+013000` → seconds.
fn utc_offset(text: &str) -> Option<i32> {
    let text = text.trim();
    let sign = match text.chars().next()? {
        '-' => -1,
        '+' => 1,
        _ => return None,
    };
    let digits = text.get(1..)?;
    let hours: i32 = digits.get(0..2)?.parse().ok()?;
    let minutes: i32 = digits.get(2..4)?.parse().ok()?;
    let seconds: i32 = digits.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(sign * (hours * 3600 + minutes * 60 + seconds))
}

fn parse(ics: &str) -> (Vec<Event>, Zones) {
    let mut events = Vec::new();
    let mut zones: Zones = HashMap::new();
    let mut event: Option<Event> = None;
    let mut zone_name: Option<String> = None;
    let mut part: Option<Observance> = None;
    // Nested components we don't read (VALARM inside VEVENT…).
    let mut skip_depth = 0usize;

    for line in lines(ics) {
        match (
            line.name.as_str(),
            line.value.trim().to_ascii_uppercase().as_str(),
        ) {
            ("BEGIN", "VEVENT") if event.is_none() && zone_name.is_none() => {
                event = Some(Event {
                    uid: String::new(),
                    title: String::new(),
                    start: When {
                        wall: NaiveDateTime::MIN,
                        zone: Zone::Floating,
                    },
                    end: None,
                    duration: None,
                    rule: None,
                    exdates: Vec::new(),
                    recurrence_id: None,
                    cancelled: false,
                });
                continue;
            }
            ("END", "VEVENT") if skip_depth == 0 => {
                if let Some(done) = event.take() {
                    if done.start.wall != NaiveDateTime::MIN && events.len() < MAX_EVENTS {
                        events.push(done);
                    }
                }
                continue;
            }
            ("BEGIN", "VTIMEZONE") if event.is_none() => {
                zone_name = Some(String::new());
                continue;
            }
            ("END", "VTIMEZONE") => {
                zone_name = None;
                continue;
            }
            ("BEGIN", "STANDARD" | "DAYLIGHT") if zone_name.is_some() => {
                part = Some(Observance {
                    offset_s: 0,
                    start: NaiveDateTime::MIN,
                    yearly: None,
                });
                continue;
            }
            ("END", "STANDARD" | "DAYLIGHT") => {
                if let (Some(done), Some(name)) = (part.take(), &zone_name) {
                    zones.entry(name.clone()).or_default().push(done);
                }
                continue;
            }
            ("BEGIN", _) if event.is_some() => {
                skip_depth += 1;
                continue;
            }
            ("END", _) if event.is_some() && skip_depth > 0 => {
                skip_depth -= 1;
                continue;
            }
            _ => {}
        }
        if skip_depth > 0 {
            continue;
        }

        if let Some(part) = part.as_mut() {
            match line.name.as_str() {
                "TZOFFSETTO" => part.offset_s = utc_offset(&line.value).unwrap_or(0),
                "DTSTART" => part.start = date_time(&line.value).unwrap_or(NaiveDateTime::MIN),
                "RRULE" => {
                    let parts: HashMap<&str, &str> = line
                        .value
                        .split(';')
                        .filter_map(|p| p.split_once('='))
                        .collect();
                    let month = parts.get("BYMONTH").and_then(|m| m.parse().ok());
                    let day = parts
                        .get("BYDAY")
                        .and_then(|d| by_day(d).into_iter().next());
                    if let (Some(month), Some((ordinal, weekday))) = (month, day) {
                        part.yearly = Some((month, ordinal, weekday));
                    }
                }
                _ => {}
            }
        } else if let Some(name) = zone_name.as_mut() {
            if line.name == "TZID" {
                *name = line.value.trim().to_owned();
            }
        } else if let Some(event) = event.as_mut() {
            match line.name.as_str() {
                "UID" => event.uid = line.value.trim().to_owned(),
                "SUMMARY" => event.title = unescape(&line.value),
                "DTSTART" => {
                    if let Some(start) = when_of(&line) {
                        event.start = start;
                    }
                }
                "DTEND" => event.end = when_of(&line),
                "DURATION" => event.duration = duration(&line.value),
                "RRULE" => event.rule = rule(&line.value, param(&line, "TZID")),
                "EXDATE" => {
                    let (tzid, is_date) =
                        (param(&line, "TZID"), param(&line, "VALUE") == Some("DATE"));
                    event
                        .exdates
                        .extend(line.value.split(',').filter_map(|v| when(v, tzid, is_date)));
                }
                "RECURRENCE-ID" => event.recurrence_id = when_of(&line),
                "STATUS" => event.cancelled = line.value.trim().eq_ignore_ascii_case("CANCELLED"),
                _ => {}
            }
        }
    }
    (events, zones)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    /// The user is in India (UTC+5:30, no daylight saving).
    fn india(utc: NaiveDateTime) -> NaiveDateTime {
        utc + Duration::minutes(330)
    }

    fn day(ics: &str, y: i32, m: u32, d: u32) -> Vec<(String, NaiveDateTime, NaiveDateTime)> {
        occurrences(
            ics,
            at(y, m, d, 0, 0),
            at(y, m, d, 0, 0) + Duration::days(1),
            &india,
        )
        .into_iter()
        .map(|o| (o.title, o.start, o.end))
        .collect()
    }

    fn wrap(body: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{}\r\nEND:VCALENDAR\r\n",
            body.trim().replace('\n', "\r\n")
        )
    }

    const NEW_YORK: &str = "BEGIN:VTIMEZONE
TZID:America/New_York
BEGIN:DAYLIGHT
TZOFFSETFROM:-0500
TZOFFSETTO:-0400
DTSTART:19700308T020000
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU
END:DAYLIGHT
BEGIN:STANDARD
TZOFFSETFROM:-0400
TZOFFSETTO:-0500
DTSTART:19701101T020000
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU
END:STANDARD
END:VTIMEZONE";

    #[test]
    fn utc_floating_and_all_day_events() {
        let ics = wrap(
            "BEGIN:VEVENT
UID:a
SUMMARY:Stand-up\\, team
DTSTART:20261012T043000Z
DTEND:20261012T050000Z
END:VEVENT
BEGIN:VEVENT
UID:b
SUMMARY:Lunch with a very long
  folded title
DTSTART:20261012T130000
DURATION:PT45M
END:VEVENT
BEGIN:VEVENT
UID:c
SUMMARY:Holiday
DTSTART;VALUE=DATE:20261012
DTEND;VALUE=DATE:20261013
END:VEVENT",
        );
        let events = occurrences(&ics, at(2026, 10, 12, 0, 0), at(2026, 10, 13, 0, 0), &india);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].title, "Holiday");
        assert!(events[0].all_day);
        assert_eq!(
            (events[0].start, events[0].end),
            (at(2026, 10, 12, 0, 0), at(2026, 10, 13, 0, 0))
        );
        // 04:30 UTC is 10:00 in India.
        assert_eq!(events[1].title, "Stand-up, team");
        assert_eq!(
            (events[1].start, events[1].end),
            (at(2026, 10, 12, 10, 0), at(2026, 10, 12, 10, 30))
        );
        assert_eq!(events[2].title, "Lunch with a very long folded title");
        assert_eq!(events[2].end, at(2026, 10, 12, 13, 45));
        // The next day has none of them.
        assert!(day(&ics, 2026, 10, 13).is_empty());
    }

    #[test]
    fn named_zones_follow_their_daylight_saving() {
        let ics = wrap(&format!(
            "{NEW_YORK}
BEGIN:VEVENT
UID:ny
SUMMARY:Call New York
DTSTART;TZID=America/New_York:20260105T090000
DTEND;TZID=America/New_York:20260105T093000
RRULE:FREQ=WEEKLY;BYDAY=MO
END:VEVENT"
        ));
        // January: New York is UTC-5, so 09:00 there is 19:30 in India.
        assert_eq!(day(&ics, 2026, 1, 5)[0].1, at(2026, 1, 5, 19, 30));
        // July: UTC-4, so the same 09:00 meeting is 18:30 in India.
        assert_eq!(day(&ics, 2026, 7, 6)[0].1, at(2026, 7, 6, 18, 30));
        // Only on Mondays.
        assert!(day(&ics, 2026, 7, 7).is_empty());
    }

    #[test]
    fn repeating_events_respect_count_until_interval_and_exceptions() {
        let ics = wrap(
            "BEGIN:VEVENT
UID:daily
SUMMARY:Daily three times
DTSTART:20261012T090000
DTEND:20261012T091500
RRULE:FREQ=DAILY;COUNT=3
END:VEVENT
BEGIN:VEVENT
UID:biweekly
SUMMARY:Every other week
DTSTART:20261005T150000
DTEND:20261005T160000
RRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH;UNTIL=20261031T000000
EXDATE:20261022T150000
END:VEVENT
BEGIN:VEVENT
UID:biweekly
RECURRENCE-ID:20261019T150000
SUMMARY:Every other week (moved)
DTSTART:20261019T170000
DTEND:20261019T180000
END:VEVENT",
        );
        let titles = |d: u32| {
            day(&ics, 2026, 10, d)
                .into_iter()
                .map(|e| e.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(12), ["Daily three times"]);
        assert_eq!(titles(14), ["Daily three times"]);
        assert!(titles(15).is_empty(), "COUNT=3 is used up");
        assert_eq!(titles(5), ["Every other week"]);
        assert_eq!(titles(8), ["Every other week"]);
        // The 12th and 15th fall in the skipped week: only the daily event (until its count ends).
        assert_eq!(titles(13), ["Daily three times"]);
        // The 19th was moved to 17:00 by its override; the 22nd is an EXDATE.
        assert_eq!(
            day(&ics, 2026, 10, 19),
            [(
                "Every other week (moved)".to_owned(),
                at(2026, 10, 19, 17, 0),
                at(2026, 10, 19, 18, 0)
            )]
        );
        assert!(titles(22).is_empty());
        assert!(day(&ics, 2026, 11, 2).is_empty(), "UNTIL has passed");
    }

    #[test]
    fn monthly_and_yearly_rules() {
        let ics = wrap(
            "BEGIN:VEVENT
UID:m1
SUMMARY:Second Tuesday
DTSTART:20260113T100000
RRULE:FREQ=MONTHLY;BYDAY=2TU
END:VEVENT
BEGIN:VEVENT
UID:m2
SUMMARY:Last Friday
DTSTART:20260130T160000
RRULE:FREQ=MONTHLY;BYDAY=-1FR
END:VEVENT
BEGIN:VEVENT
UID:m3
SUMMARY:On the 31st
DTSTART:20260131T080000
RRULE:FREQ=MONTHLY
END:VEVENT
BEGIN:VEVENT
UID:y1
SUMMARY:Birthday
DTSTART;VALUE=DATE:20200610
RRULE:FREQ=YEARLY
END:VEVENT",
        );
        let titles = |m: u32, d: u32| {
            day(&ics, 2026, m, d)
                .into_iter()
                .map(|e| e.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(10, 13), ["Second Tuesday"]);
        assert!(titles(10, 6).is_empty());
        assert_eq!(titles(10, 30), ["Last Friday"]);
        assert_eq!(titles(10, 31), ["On the 31st"]);
        assert!(titles(11, 30).is_empty(), "November has no 31st");
        assert_eq!(titles(6, 10), ["Birthday"]);
    }

    #[test]
    fn cancelled_events_and_cancelled_occurrences_disappear() {
        let ics = wrap(
            "BEGIN:VEVENT
UID:gone
SUMMARY:Cancelled meeting
STATUS:CANCELLED
DTSTART:20261012T100000
END:VEVENT
BEGIN:VEVENT
UID:series
SUMMARY:Weekly
DTSTART:20261005T100000
RRULE:FREQ=WEEKLY
END:VEVENT
BEGIN:VEVENT
UID:series
RECURRENCE-ID:20261012T100000
STATUS:CANCELLED
SUMMARY:Weekly
DTSTART:20261012T100000
END:VEVENT",
        );
        assert!(day(&ics, 2026, 10, 12).is_empty());
        assert_eq!(day(&ics, 2026, 10, 19).len(), 1);
    }

    #[test]
    fn events_that_span_the_window_are_included() {
        let ics = wrap(
            "BEGIN:VEVENT
UID:trip
SUMMARY:Conference
DTSTART;VALUE=DATE:20261010
DTEND;VALUE=DATE:20261014
END:VEVENT
BEGIN:VEVENT
UID:late
SUMMARY:Night shift
DTSTART:20261011T220000
DTEND:20261012T020000
END:VEVENT",
        );
        let titles: Vec<_> = day(&ics, 2026, 10, 12).into_iter().map(|e| e.0).collect();
        assert_eq!(titles, ["Conference", "Night shift"]);
        assert!(day(&ics, 2026, 10, 14).is_empty(), "DTEND is exclusive");
    }

    #[test]
    fn broken_or_hostile_input_is_harmless() {
        for junk in [
            "",
            "not a calendar",
            "BEGIN:VEVENT\nDTSTART:garbage\nEND:VEVENT",
            "BEGIN:VEVENT\nSUMMARY:no start\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:20261012T100000\nRRULE:FREQ=SECONDLY\nEND:VEVENT\nBEGIN:VEVENT",
        ] {
            let _ = occurrences(junk, at(2026, 10, 12, 0, 0), at(2026, 10, 13, 0, 0), &india);
        }
        // A daily event since 1990 with no end still answers quickly and correctly.
        let old = "BEGIN:VEVENT\nUID:o\nSUMMARY:Old habit\nDTSTART:19900101T070000\nRRULE:FREQ=DAILY\nEND:VEVENT";
        assert_eq!(day(old, 2026, 10, 12).len(), 1);
        // An alarm inside an event isn't mistaken for the event's own fields.
        let alarm = "BEGIN:VEVENT\nUID:x\nSUMMARY:Real\nDTSTART:20261012T100000\nBEGIN:VALARM\nSUMMARY:Alarm text\nTRIGGER:-PT10M\nEND:VALARM\nEND:VEVENT";
        assert_eq!(day(alarm, 2026, 10, 12)[0].0, "Real");
    }

    #[test]
    fn small_parsers() {
        assert_eq!(duration("PT1H30M"), Some(Duration::minutes(90)));
        assert_eq!(duration("P1DT2H"), Some(Duration::hours(26)));
        assert_eq!(duration("P2W"), Some(Duration::weeks(2)));
        assert_eq!(utc_offset("+0530"), Some(19_800));
        assert_eq!(utc_offset("-0800"), Some(-28_800));
        assert_eq!(
            by_day("MO,-1FR,2TU"),
            [(0, Weekday::Mon), (-1, Weekday::Fri), (2, Weekday::Tue)]
        );
        assert_eq!(
            nth_weekday(2026, 3, 2, Weekday::Sun),
            NaiveDate::from_ymd_opt(2026, 3, 8)
        );
        assert_eq!(
            nth_weekday(2026, 10, -1, Weekday::Fri),
            NaiveDate::from_ymd_opt(2026, 10, 30)
        );
        assert_eq!(nth_weekday(2026, 2, 5, Weekday::Mon), None);
    }
}

#[cfg(test)]
mod real_feed {
    use super::*;

    /// `PREKU_ICS=<file> cargo test --lib real_feed -- --ignored --nocapture`: reads a real
    /// calendar file and prints what the coming year looks like.
    #[test]
    #[ignore = "needs PREKU_ICS pointing at a calendar file"]
    fn reads_a_real_calendar_file() {
        let path = std::env::var("PREKU_ICS").expect("set PREKU_ICS");
        let text = std::fs::read_to_string(path).unwrap();
        let from = NaiveDate::from_ymd_opt(2026, 10, 1)
            .unwrap()
            .and_time(NaiveTime::MIN);
        let found = occurrences(&text, from, from + Duration::days(365), &|utc| utc);
        println!("events in file: {}", text.matches("BEGIN:VEVENT").count());
        println!("occurrences in the next year: {}", found.len());
        for o in found.iter().take(8) {
            println!("  {} {} all_day={}", o.start.date(), o.title, o.all_day);
        }
        assert!(!found.is_empty());
    }
}
