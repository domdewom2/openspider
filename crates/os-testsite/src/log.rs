//! Request-Log der Testsite: Einträge je Request plus Zähler für Verbindungen und Gleichzeitigkeit.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use time::OffsetDateTime;

/// Ein geloggter Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRecord {
    /// Laufende Nummer ab 1 in Eingangsreihenfolge (seit dem letzten Zurücksetzen).
    pub seq: u64,
    /// Eingang des Requests, RFC 3339 in UTC mit Millisekunden (`2026-01-01T00:00:00.000Z`).
    pub ts: String,
    /// Nummer der TCP-Verbindung (ab 1, über Zurücksetzen hinweg eindeutig); mehrere Requests
    /// über Keep-Alive teilen sich die Nummer.
    pub conn_id: u64,
    /// Methode (`GET`, `HEAD` …).
    pub method: String,
    /// Wert des `Host`-Headers in Kleinbuchstaben ohne Port (leer, wenn nicht gesendet).
    pub host: String,
    /// Request-Ziel wie gesendet, Pfad inklusive Query (`/a?b=1`).
    pub path_and_query: String,
    /// Request-Header in Eingangsreihenfolge, Namen wie gesendet.
    pub headers: Vec<(String, String)>,
    /// Ob der Request über TLS einging.
    pub tls: bool,
}

impl RequestRecord {
    /// Pfad ohne Query.
    pub fn path(&self) -> &str {
        self.path_and_query
            .split_once('?')
            .map_or(self.path_and_query.as_str(), |(p, _)| p)
    }

    /// Alle Werte eines Headers (Name ohne Beachtung der Groß-/Kleinschreibung).
    pub fn header_values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Erster Wert eines Headers.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.header_values(name).into_iter().next()
    }
}

/// Zeitstempel im Format von [`RequestRecord::ts`].
pub(crate) fn now_ts() -> String {
    let t = OffsetDateTime::now_utc();
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

/// Interner Log-Zustand.
#[derive(Default)]
pub(crate) struct LogState {
    entries: Mutex<Vec<RequestRecord>>,
    next_conn: AtomicU64,
    connections: AtomicU64,
    inflight: AtomicUsize,
    max_inflight: AtomicUsize,
}

impl LogState {
    pub(crate) fn new_conn(&self) -> u64 {
        self.connections.fetch_add(1, Ordering::Relaxed);
        self.next_conn.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn push(&self, mut rec: RequestRecord) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        rec.seq = entries.len() as u64 + 1;
        entries.push(rec);
    }

    pub(crate) fn requests(&self) -> Vec<RequestRecord> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(crate) fn connections(&self) -> u64 {
        self.connections.load(Ordering::Relaxed)
    }

    pub(crate) fn max_concurrency(&self) -> usize {
        self.max_inflight.load(Ordering::Relaxed)
    }

    pub(crate) fn reset(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.connections.store(0, Ordering::Relaxed);
        self.max_inflight
            .store(self.inflight.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    /// Markiert einen Request als in Bearbeitung; der Guard beendet ihn beim Drop.
    pub(crate) fn begin(&self) -> InflightGuard<'_> {
        let now = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_inflight.fetch_max(now, Ordering::SeqCst);
        InflightGuard(self)
    }
}

pub(crate) struct InflightGuard<'a>(&'a LogState);

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_format() {
        let ts = now_ts();
        assert_eq!(ts.len(), "2026-01-01T00:00:00.000Z".len(), "{ts}");
        assert!(ts.ends_with('Z'));
    }

    #[test]
    fn concurrency_and_seq() {
        let log = LogState::default();
        {
            let _a = log.begin();
            let _b = log.begin();
        }
        let _c = log.begin();
        assert_eq!(log.max_concurrency(), 2);
        log.push(RequestRecord {
            seq: 0,
            ts: String::new(),
            conn_id: 1,
            method: "GET".into(),
            host: String::new(),
            path_and_query: "/a?b=1".into(),
            headers: vec![("X-A".into(), "1".into()), ("x-a".into(), "2".into())],
            tls: false,
        });
        let r = &log.requests()[0];
        assert_eq!(r.seq, 1);
        assert_eq!(r.path(), "/a");
        assert_eq!(r.header_values("X-A"), vec!["1", "2"]);
    }
}
