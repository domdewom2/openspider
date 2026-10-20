//! Lokale Zeit mit Sommerzeit (OUT-161). Die Terminberechnung arbeitet gegen das Merkmal [`Zone`]; Tests
//! verwenden die festen Regeln [`EuZone`] (Europe/Berlin) und [`FixedZone`], der Betrieb [`SystemZone`].
//!
//! Auflösung lokaler Zeiten: Existiert eine Wanduhrzeit nicht (Lücke beim Vorstellen der Uhr), gilt die
//! Zeit mit dem Offset vor der Umstellung (02:30 → 03:30). Existiert sie zweimal (Uhr wird zurückgestellt),
//! gilt das erste Auftreten.

use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, UtcOffset, Weekday};

/// Ergebnis der Auflösung einer lokalen Wanduhrzeit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    /// Eindeutig.
    Single(OffsetDateTime),
    /// Zweimal vorhanden (Rückstellung): erstes und zweites Auftreten.
    Ambiguous(OffsetDateTime, OffsetDateTime),
    /// Nicht vorhanden (Vorstellung): Offsets vor und nach der Umstellung.
    Gap(UtcOffset, UtcOffset),
}

/// Zeitzone.
pub trait Zone: Send + Sync {
    /// Offset zum Zeitpunkt `utc`.
    fn offset_at(&self, utc: OffsetDateTime) -> UtcOffset;

    /// Löst eine lokale Wanduhrzeit auf.
    fn resolve(&self, local: PrimitiveDateTime) -> Resolved {
        let guess = local.assume_utc();
        let before = self.offset_at(guess - Duration::days(1));
        let after = self.offset_at(guess + Duration::days(1));
        let mut found: Vec<OffsetDateTime> = Vec::new();
        let mut offsets = vec![before];
        if after != before {
            offsets.push(after);
        }
        for offset in offsets {
            let utc = local.assume_offset(offset).to_offset(UtcOffset::UTC);
            if self.offset_at(utc) == offset {
                found.push(utc);
            }
        }
        found.sort();
        match found.as_slice() {
            [one] => Resolved::Single(*one),
            [first, second] => Resolved::Ambiguous(*first, *second),
            _ => Resolved::Gap(before, after),
        }
    }

    /// Der Zeitpunkt, an dem eine Aufgabe mit dieser Wanduhrzeit läuft (Regeln im Modulkopf).
    fn resolve_local(&self, local: PrimitiveDateTime) -> OffsetDateTime {
        match self.resolve(local) {
            Resolved::Single(t) | Resolved::Ambiguous(t, _) => t,
            Resolved::Gap(before, _) => local.assume_offset(before).to_offset(UtcOffset::UTC),
        }
    }

    /// Lokales Datum und lokale Uhrzeit zum Zeitpunkt `utc`.
    fn local(&self, utc: OffsetDateTime) -> PrimitiveDateTime {
        let t = utc.to_offset(self.offset_at(utc));
        PrimitiveDateTime::new(t.date(), t.time())
    }
}

/// Fester Offset ohne Sommerzeit.
#[derive(Debug, Clone, Copy)]
pub struct FixedZone(pub UtcOffset);

impl Zone for FixedZone {
    fn offset_at(&self, _utc: OffsetDateTime) -> UtcOffset {
        self.0
    }
}

/// Zone mit den Sommerzeitregeln der EU (Umstellung am letzten Sonntag im März und im Oktober um 01:00 UTC).
/// `EuZone::central_europe()` entspricht Europe/Berlin.
#[derive(Debug, Clone, Copy)]
pub struct EuZone {
    /// Normalzeit.
    pub standard: UtcOffset,
}

impl EuZone {
    /// MEZ/MESZ (Europe/Berlin).
    pub fn central_europe() -> EuZone {
        EuZone {
            standard: UtcOffset::from_hms(1, 0, 0).expect("gültiger Offset"),
        }
    }
}

fn last_sunday(year: i32, month: Month) -> Date {
    let mut day =
        Date::from_calendar_date(year, month, month.length(year)).expect("gültiges Datum");
    while day.weekday() != Weekday::Sunday {
        day = day.previous_day().expect("Datum im Bereich");
    }
    day
}

impl Zone for EuZone {
    fn offset_at(&self, utc: OffsetDateTime) -> UtcOffset {
        let utc = utc.to_offset(UtcOffset::UTC);
        let at = |month| {
            last_sunday(utc.year(), month)
                .with_hms(1, 0, 0)
                .expect("gültige Zeit")
                .assume_utc()
        };
        if utc >= at(Month::March) && utc < at(Month::October) {
            UtcOffset::from_whole_seconds(self.standard.whole_seconds() + 3600).expect("Offset")
        } else {
            self.standard
        }
    }
}

/// Lokale Zeit des Rechners. Das Betriebssystem liefert den Offset je Zeitpunkt, solange der Prozess nur
/// einen Thread hat (Einschränkung der `time`-Bibliothek unter Unix); sonst gilt der beim ersten Aufruf
/// ermittelte Offset ohne Sommerzeitumstellung. Der OS-Planer löst die Termine unabhängig davon in lokaler
/// Zeit aus; die Zone dient der Anzeige des nächsten Laufs und der Validierung.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemZone;

impl Zone for SystemZone {
    fn offset_at(&self, utc: OffsetDateTime) -> UtcOffset {
        if let Ok(offset) = UtcOffset::local_offset_at(utc) {
            return offset;
        }
        cached_offset()
    }
}

fn cached_offset() -> UtcOffset {
    use std::sync::OnceLock;
    static CACHED: OnceLock<UtcOffset> = OnceLock::new();
    *CACHED.get_or_init(|| {
        // `Timestamp::to_export` kennt den beim Programmstart ermittelten Offset (`init_local_offset`).
        let text = os_model::Timestamp::now_utc().to_export();
        let sign = text.chars().rev().nth(5);
        let tail = &text[text.len().saturating_sub(5)..];
        let (h, m) = tail.split_once(':').unwrap_or(("00", "00"));
        let (h, m): (i8, i8) = (h.parse().unwrap_or(0), m.parse().unwrap_or(0));
        let (h, m) = if sign == Some('-') { (-h, -m) } else { (h, m) };
        UtcOffset::from_hms(h, m, 0).unwrap_or(UtcOffset::UTC)
    })
}
