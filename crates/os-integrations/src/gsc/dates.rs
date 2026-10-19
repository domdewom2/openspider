//! Zeitraum und Datenstand der Search-Analytics-Abfrage (INTEG-111).
//!
//! Google stellt Tagesdaten in US-Pazifikzeit bereit, finale Daten typischerweise mit zwei bis drei Tagen Verzug, und
//! hält höchstens 16 Monate vor. Der letzte vollständig verfügbare Tag ist daher „heute − 2“ (America/Los_Angeles);
//! der früheste abfragbare Tag ist „heute − 16 Monate“.

use os_model::Timestamp;
use time::{Date, Duration, Month};

use super::options::DateSpan;

/// Ein aufgelöster Zeitraum (beide Daten einschließlich).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub start: Date,
    pub end: Date,
    /// Der Beginn wurde auf das früheste mögliche Datum begrenzt (Hinweis im Tab).
    pub clamped: bool,
}

impl DateRange {
    /// Zeitraumhinweis für den Tab, z. B. `2026-09-17 – 2026-10-16`.
    pub fn label(&self) -> String {
        format!("{} – {}", self.start, self.end)
    }
}

/// Heutiges Datum in US-Pazifikzeit.
pub fn pacific_today(now: Timestamp) -> Date {
    crate::queue::pacific_date(now)
}

/// Der letzte vollständig verfügbare Tag: heute − 2 (America/Los_Angeles).
pub fn last_complete_day(now: Timestamp) -> Date {
    pacific_today(now) - Duration::days(2)
}

/// Frühestes Datum, das Search Console noch liefert: heute − 16 Monate.
pub fn earliest_date(now: Timestamp) -> Date {
    minus_months(pacific_today(now), 16)
}

/// Löst den Zeitraum zum Zeitpunkt `now` auf und begrenzt einen zu frühen Beginn.
pub fn resolve(span: DateSpan, now: Timestamp) -> DateRange {
    let end = last_complete_day(now);
    let (start, end) = match span {
        DateSpan::LastDays(n) => (end - Duration::days(i64::from(n.max(1)) - 1), end),
        DateSpan::LastMonths(n) => (minus_months(pacific_today(now), n), end),
        DateSpan::Custom { start, end } => (start, end),
    };
    let earliest = earliest_date(now);
    if start < earliest {
        DateRange {
            start: earliest,
            end: end.max(earliest),
            clamped: true,
        }
    } else {
        DateRange {
            start,
            end,
            clamped: false,
        }
    }
}

/// `date` minus `months` Kalendermonate; ein Tag, den es im Zielmonat nicht gibt, wird auf dessen Monatsende gesetzt.
fn minus_months(date: Date, months: u8) -> Date {
    let total = date.year() * 12 + i32::from(u8::from(date.month())) - 1 - i32::from(months);
    let year = total.div_euclid(12);
    let month = Month::try_from((total.rem_euclid(12) + 1) as u8).expect("Monat 1 – 12");
    let day = date.day().min(month.length(year));
    Date::from_calendar_date(year, month, day).expect("gültiges Datum")
}
