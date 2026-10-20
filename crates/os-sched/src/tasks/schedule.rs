//! Terminberechnung `next_run(schedule, now, zone)` (OUT-161).
//!
//! * Einmalig: der Zeitpunkt, solange er in der Zukunft liegt.
//! * Täglich, wöchentlich, monatlich: der nächste Termin nach `now` in lokaler Zeit; ein Termin, den es wegen
//!   der Sommerzeit nicht gibt, läuft eine Stunde später, ein doppelt vorhandener beim ersten Auftreten
//!   (Regeln: [`Zone::resolve_local`]), sodass ein täglicher Lauf je Kalendertag genau einmal stattfindet.
//! * Monatlich mit Starttag 29 – 31: in kürzeren Monaten am letzten Tag des Monats.
//! * Stündlich und alle N Tage [P2]: ab dem Startzeitpunkt im festen Abstand.

use os_model::Timestamp;
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};

use super::model::Schedule;
use super::zone::Zone;

/// Obergrenze der Suche nach dem nächsten Termin (Tage).
const HORIZON_DAYS: i64 = 800;

/// Liest `HH:MM`.
pub fn parse_time(text: &str) -> Option<Time> {
    let (h, m) = text.trim().split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    Time::from_hms(h.parse().ok()?, m.parse().ok()?, 0).ok()
}

/// Liest `YYYY-MM-DD`.
pub fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.trim().splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

/// Liest `YYYY-MM-DDTHH:MM` (lokal).
pub fn parse_local(text: &str) -> Option<PrimitiveDateTime> {
    let (date, time) = text.trim().split_once('T')?;
    Some(PrimitiveDateTime::new(
        parse_date(date)?,
        parse_time(time.get(..5)?)?,
    ))
}

/// Liest einen RFC-3339-Zeitpunkt.
pub fn parse_instant(text: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(text.trim(), &Rfc3339).ok()
}

/// Tag des Monats, an dem ein Termin mit Starttag `day` läuft (kurze Monate: letzter Tag).
pub fn effective_day(day: u8, year: i32, month: Month) -> u8 {
    day.min(month.length(year))
}

fn next_month(year: i32, month: Month) -> (i32, Month) {
    match month {
        Month::December => (year + 1, Month::January),
        m => (year, m.next()),
    }
}

/// Der nächste Lauf nach `now`; `None`, wenn es keinen mehr gibt (abgelaufene einmalige Aufgabe,
/// ungültige Angaben).
pub fn next_run(schedule: &Schedule, now: Timestamp, zone: &dyn Zone) -> Option<Timestamp> {
    let now = now.0;
    let found = match schedule {
        Schedule::Once { at } => parse_instant(at).filter(|t| *t > now),
        Schedule::Hourly { start } => {
            fixed_step(parse_local(start)?, Duration::hours(1), now, zone)
        }
        Schedule::EveryDays { every, start } => {
            if *every == 0 {
                return None;
            }
            let start = parse_local(start)?;
            let mut date = start.date();
            let from = zone.local(now).date().previous_day()?;
            // Auf das letzte Vielfache vor `from` vorspulen.
            let skip = (from - date).whole_days().max(0) / i64::from(*every);
            date = date.checked_add(Duration::days(skip * i64::from(*every)))?;
            let mut result = None;
            for _ in 0..HORIZON_DAYS {
                let candidate = zone.resolve_local(PrimitiveDateTime::new(date, start.time()));
                if candidate > now {
                    result = Some(candidate);
                    break;
                }
                date = date.checked_add(Duration::days(i64::from(*every)))?;
            }
            result
        }
        Schedule::Daily { time, from } => {
            by_day(parse_time(time)?, from.as_deref(), now, zone, |_| true)
        }
        Schedule::Weekly { days, time, from } => {
            if days.is_empty() || days.iter().any(|d| !(1..=7).contains(d)) {
                return None;
            }
            by_day(parse_time(time)?, from.as_deref(), now, zone, |date| {
                days.contains(&date.weekday().number_from_monday())
            })
        }
        Schedule::Monthly { day, time, from } => {
            if !(1..=31).contains(day) {
                return None;
            }
            monthly(*day, parse_time(time)?, from.as_deref(), now, zone)
        }
    };
    found.map(|t| Timestamp(t.to_offset(time::UtcOffset::UTC)))
}

fn fixed_step(
    start: PrimitiveDateTime,
    step: Duration,
    now: OffsetDateTime,
    zone: &dyn Zone,
) -> Option<OffsetDateTime> {
    let first = zone.resolve_local(start);
    if first > now {
        return Some(first);
    }
    let elapsed = now - first;
    let n = elapsed.whole_seconds() / step.whole_seconds() + 1;
    Some(first + step * i32::try_from(n).ok()?)
}

fn from_date(from: Option<&str>) -> Option<Option<Date>> {
    match from {
        None => Some(None),
        Some(text) => parse_date(text).map(Some),
    }
}

fn by_day(
    time: Time,
    from: Option<&str>,
    now: OffsetDateTime,
    zone: &dyn Zone,
    accept: impl Fn(Date) -> bool,
) -> Option<OffsetDateTime> {
    let from = from_date(from)?;
    // Einen Tag zurück beginnen: Wegen Offsets und Umstellungen kann der Termin des Vortags noch bevorstehen.
    let mut date = zone.local(now).date().previous_day()?;
    for _ in 0..HORIZON_DAYS {
        if from.is_none_or(|f| date >= f) && accept(date) {
            let candidate = zone.resolve_local(PrimitiveDateTime::new(date, time));
            if candidate > now {
                return Some(candidate);
            }
        }
        date = date.next_day()?;
    }
    None
}

fn monthly(
    day: u8,
    time: Time,
    from: Option<&str>,
    now: OffsetDateTime,
    zone: &dyn Zone,
) -> Option<OffsetDateTime> {
    let from = from_date(from)?;
    let today = zone.local(now).date();
    let (mut year, mut month) = (today.year(), today.month());
    for _ in 0..40 {
        let date = Date::from_calendar_date(year, month, effective_day(day, year, month)).ok()?;
        if from.is_none_or(|f| date >= f) {
            let candidate = zone.resolve_local(PrimitiveDateTime::new(date, time));
            if candidate > now {
                return Some(candidate);
            }
        }
        (year, month) = next_month(year, month);
    }
    None
}
