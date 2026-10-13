//! Zeit und Kennungsquellen: `Timestamp`, `Clock` (System/fest/manuell), `IdSource`.
//!
//! Zeit gibt es im Kern nur über [`Clock`]; Crawl-IDs nur über [`IdSource`] (M1-Plan 2.8).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::error::ModelError;
use crate::ids::CrawlId;

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

/// Ermittelt den lokalen UTC-Offset und merkt ihn sich für [`Timestamp::to_export`].
///
/// Auf Unix-Systemen liefert `time` den lokalen Offset nur, solange der Prozess noch einen einzigen Thread
/// hat. Programme (`os-cli`, App) rufen diese Funktion deshalb ganz am Anfang von `main` auf, bevor eine
/// Laufzeit Threads startet. Ist der Offset nicht ermittelbar, gilt UTC. Mehrfache Aufrufe sind harmlos.
pub fn init_local_offset() {
    LOCAL_OFFSET.get_or_init(|| UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));
}

/// Zeitpunkt, intern immer UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Timestamp = Timestamp(OffsetDateTime::UNIX_EPOCH);

    /// Jetzt (Systemuhr). Nur über [`SystemClock`] im Produktivcode verwenden.
    pub fn now_utc() -> Self {
        Timestamp(OffsetDateTime::now_utc())
    }

    /// Aus Unix-Millisekunden.
    pub fn from_unix_millis(ms: i64) -> Self {
        let nanos = i128::from(ms) * 1_000_000;
        Timestamp(
            OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap_or(OffsetDateTime::UNIX_EPOCH),
        )
    }

    /// Unix-Millisekunden (abgerundet).
    pub fn unix_millis(&self) -> i64 {
        (self.0.unix_timestamp_nanos() / 1_000_000) as i64
    }

    /// Um eine Dauer später.
    pub fn plus(&self, d: Duration) -> Self {
        Timestamp(self.0 + d)
    }

    /// Datenbankform `2026-10-13T07:35:00.123Z` (UTC, immer Millisekunden).
    pub fn to_db(&self) -> String {
        let t = self.0.to_offset(UtcOffset::UTC);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            t.year(),
            u8::from(t.month()),
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
            t.millisecond()
        )
    }

    /// Liest die Datenbankform (und jede andere RFC-3339-Angabe; das Ergebnis ist UTC).
    pub fn from_db(s: &str) -> Result<Self, ModelError> {
        OffsetDateTime::parse(s, &Rfc3339)
            .map(|t| Timestamp(t.to_offset(UtcOffset::UTC)))
            .map_err(|_| ModelError::InvalidTimestamp(s.to_string()))
    }

    /// Exportform (06 §0): ISO 8601 mit lokalem Offset, ohne Sekundenbruchteile, z. B. `2026-10-13T09:35:00+02:00`.
    /// Der Offset kommt aus [`init_local_offset`]; ohne Aufruf wird er bei Bedarf ermittelt. Ist er nicht
    /// ermittelbar, gilt UTC (`+00:00`).
    pub fn to_export(&self) -> String {
        let offset = LOCAL_OFFSET
            .get()
            .copied()
            .or_else(|| UtcOffset::current_local_offset().ok())
            .unwrap_or(UtcOffset::UTC);
        self.to_export_with(offset)
    }

    /// Exportform mit ausdrücklich gegebenem Offset (deterministisch testbar).
    pub fn to_export_with(&self, offset: UtcOffset) -> String {
        let t = self.0.to_offset(offset);
        let (h, m, _) = offset.as_hms();
        let sign = if offset.is_negative() { '-' } else { '+' };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
            t.year(),
            u8::from(t.month()),
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
            sign,
            h.unsigned_abs(),
            m.unsigned_abs()
        )
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Timestamp::UNIX_EPOCH
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_db())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        Timestamp::from_db(&s).map_err(serde::de::Error::custom)
    }
}

/// Zeitquelle. `now()` ist Wanduhrzeit (Zeitstempel), `monotonic()` eine nie rückwärts laufende Dauer
/// (Antwortzeiten, Raten, Heartbeat).
pub trait Clock: Send + Sync + 'static {
    /// Aktuelle Zeit.
    fn now(&self) -> Timestamp;
    /// Monotone Zeit seit einem beliebigen, festen Ausgangspunkt.
    fn monotonic(&self) -> Duration;
}

/// Echte Systemuhr.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now_utc()
    }

    fn monotonic(&self) -> Duration {
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed()
    }
}

/// Feste Uhr für Tests: jede Abfrage (`now()` oder `monotonic()`) rückt um `step` vor.
#[derive(Debug)]
pub struct FixedClock {
    start: Timestamp,
    step: Duration,
    ticks: AtomicU64,
}

impl FixedClock {
    /// Beginnt bei `start`; die erste Abfrage liefert genau `start` bzw. `Duration::ZERO`.
    pub fn new(start: Timestamp, step: Duration) -> Self {
        FixedClock {
            start,
            step,
            ticks: AtomicU64::new(0),
        }
    }

    fn tick(&self) -> Duration {
        let n = self.ticks.fetch_add(1, Ordering::SeqCst);
        let nanos = self.step.as_nanos().saturating_mul(u128::from(n));
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.start.plus(self.tick())
    }

    fn monotonic(&self) -> Duration {
        self.tick()
    }
}

/// Manuelle Uhr: steht still, bis der Test sie mit [`ManualClock::advance`] vorstellt; `now()` und
/// `monotonic()` rücken gemeinsam vor (M1-Plan 2.8: CRAWL-130/112/126/141).
#[derive(Debug)]
pub struct ManualClock {
    start: Timestamp,
    elapsed: Mutex<Duration>,
}

impl ManualClock {
    /// Beginnt bei `start`, `monotonic()` bei `Duration::ZERO`.
    pub fn new(start: Timestamp) -> Self {
        ManualClock {
            start,
            elapsed: Mutex::new(Duration::ZERO),
        }
    }

    /// Stellt die Uhr um `d` vor.
    pub fn advance(&self, d: Duration) {
        let mut e = self.elapsed.lock().unwrap_or_else(|p| p.into_inner());
        *e += d;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        self.start
            .plus(*self.elapsed.lock().unwrap_or_else(|p| p.into_inner()))
    }

    fn monotonic(&self) -> Duration {
        *self.elapsed.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Quelle für Crawl-IDs (UUID v7, CRAWL-128).
pub trait IdSource: Send + Sync + 'static {
    /// Neue Crawl-ID; der Zeitanteil stammt aus `now`.
    fn new_crawl_id(&self, now: Timestamp) -> CrawlId;
}

/// Zufällige UUID v7.
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomIds;

impl IdSource for RandomIds {
    fn new_crawl_id(&self, now: Timestamp) -> CrawlId {
        let secs = u64::try_from(now.0.unix_timestamp()).unwrap_or(0);
        let ts = uuid::Timestamp::from_unix(uuid::NoContext, secs, now.0.nanosecond());
        CrawlId(uuid::Uuid::new_v7(ts))
    }
}

/// Deterministische UUIDs v7 aus Seed und Zähler (Tests).
#[derive(Debug)]
pub struct SeededIds {
    seed: u64,
    counter: AtomicU64,
}

impl SeededIds {
    /// Gleicher Seed und gleiche Zeit ergeben dieselbe Folge von IDs.
    pub fn new(seed: u64) -> Self {
        SeededIds {
            seed,
            counter: AtomicU64::new(0),
        }
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl IdSource for SeededIds {
    fn new_crawl_id(&self, now: Timestamp) -> CrawlId {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let a = splitmix64(self.seed ^ splitmix64(n));
        let b = splitmix64(a);
        let mut random = [0u8; 10];
        random[..8].copy_from_slice(&a.to_be_bytes());
        random[8..].copy_from_slice(&b.to_be_bytes()[..2]);
        let ms = u64::try_from(now.unix_millis()).unwrap_or(0);
        CrawlId(uuid::Builder::from_unix_timestamp_millis(ms, &random).into_uuid())
    }
}
