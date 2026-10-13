//! Writer-Thread mit begrenztem Kanal (Backpressure, PERF-003).
//!
//! Ein eigener Betriebssystem-Thread besitzt die Senke (`Box<dyn CrawlSink>`, Standard `SqliteSink`) und
//! schreibt, was die Worker über einen **begrenzten** Kanal schicken. Ist der Kanal voll, warten die Worker
//! in `reserve().await`/`send().await`: Fetch und Auswertung werden gedrosselt, der Speicher wächst nicht.
//!
//! Der Thread fasst die Nachrichten zu Batches zusammen: höchstens [`BATCH_PAGES`] Seiten oder
//! [`BATCH_MAX_WAIT`] Wartezeit, bei Rückstau (mehr als die Hälfte des Kanals belegt) bis zu
//! [`BATCH_PAGES_BACKLOG`] Seiten. Die Reihenfolge der Nachrichten bleibt erhalten; der Store verarbeitet
//! innerhalb eines Batches `new_urls` vor `pages`, deshalb genügt es, dass die Nachricht mit den neuen URLs
//! vor der mit der Seite im Kanal liegt (die Worker sorgen dafür, siehe `worker.rs`).
//!
//! Ein Fehler der Senke wird gemerkt; danach verwirft der Thread weitere Batches (die Worker blockieren
//! nicht) und meldet den Fehler bei `flush`/`finish`. Die Engine bricht den Crawl dann ab.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use os_model::WriteBatch;
use os_store::{CrawlSink, StoreError};
use tokio::sync::{mpsc, oneshot};

/// Seiten je Batch im Normalbetrieb (M1-Plan 3.7).
pub const BATCH_PAGES: usize = 200;
/// Seiten je Batch bei Rückstau.
pub const BATCH_PAGES_BACKLOG: usize = 1_000;
/// Längste Wartezeit, bis ein nicht voller Batch geschrieben wird.
pub const BATCH_MAX_WAIT: Duration = Duration::from_millis(100);
/// Nachrichten, die im Kanal warten dürfen, bevor die Worker blockieren.
pub const CHANNEL_CAPACITY: usize = 256;

enum Msg {
    Batch(Box<WriteBatch>),
    /// Antwort: `true` = alles geschrieben, `false` = die Senke hat einen Fehler gemeldet.
    Flush(oneshot::Sender<bool>),
    Checkpoint(oneshot::Sender<bool>),
}

/// Zähler des Writers (für Tests und die Lastmessung).
#[derive(Debug, Default)]
pub struct WriterStats {
    /// Geschriebene Batches.
    pub batches: AtomicU64,
    /// Geschriebene Seiten (`PageWrite`).
    pub pages: AtomicU64,
    /// Größter Batch in Seiten.
    pub max_batch_pages: AtomicUsize,
    /// Größte beobachtete Zahl wartender Nachrichten.
    pub max_queued: AtomicUsize,
}

struct Shared {
    failed: AtomicBool,
    /// Erster Fehler der Senke (als Original, für `finish`) und sein Text.
    error: Mutex<Option<StoreError>>,
    stats: WriterStats,
}

/// Der Writer-Thread ist beendet oder hat einen Fehler gemeldet; den Fehler selbst liefert
/// [`WriterThread::finish`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterGone;

impl std::fmt::Display for WriterGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writer-Thread ist beendet oder meldet einen Fehler")
    }
}

impl std::error::Error for WriterGone {}

/// Schreibseite: von den Workern geklont.
#[derive(Clone)]
pub struct WriterSender {
    tx: mpsc::Sender<Msg>,
    shared: Arc<Shared>,
}

/// Reservierter Platz im Kanal. Senden geht ohne Warten und ist damit unter einer Sperre möglich.
pub struct Slot {
    permit: mpsc::OwnedPermit<Msg>,
}

impl Slot {
    /// Legt den Batch in den Kanal (sofort, ohne Warten).
    pub fn send(self, batch: WriteBatch) {
        if !is_empty(&batch) {
            self.permit.send(Msg::Batch(Box::new(batch)));
        }
    }
}

fn is_empty(b: &WriteBatch) -> bool {
    b.new_urls.is_empty()
        && b.url_updates.is_empty()
        && b.pages.is_empty()
        && b.queue.is_empty()
        && b.sitemaps.is_empty()
        && b.robots.is_empty()
        && b.meta.is_empty()
        && b.events.is_empty()
        && b.counters.is_empty()
        && b.url_flags.is_empty()
}

/// Der Writer-Thread samt Kanal.
pub struct WriterThread {
    sender: WriterSender,
    thread: Option<JoinHandle<()>>,
}

impl WriterThread {
    /// Startet den Thread mit der Senke.
    pub fn spawn(sink: Box<dyn CrawlSink>) -> Self {
        Self::spawn_with(sink, CHANNEL_CAPACITY)
    }

    /// Wie [`spawn`](Self::spawn) mit eigener Kanalgröße (Tests).
    pub fn spawn_with(sink: Box<dyn CrawlSink>, capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        let shared = Arc::new(Shared {
            failed: AtomicBool::new(false),
            error: Mutex::new(None),
            stats: WriterStats::default(),
        });
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("os-engine-writer".to_string())
            .spawn(move || writer_loop(sink, rx, capacity.max(1), thread_shared))
            .expect("Writer-Thread starten");
        WriterThread {
            sender: WriterSender { tx, shared },
            thread: Some(thread),
        }
    }

    /// Schreibseite für die Worker.
    pub fn sender(&self) -> WriterSender {
        self.sender.clone()
    }

    /// Wartet, bis alles bisher Gesendete geschrieben ist.
    pub async fn flush(&self) -> Result<(), WriterGone> {
        self.sender.flush().await
    }

    /// Schreibt ausstehende Daten dauerhaft (WAL-Checkpoint).
    pub async fn checkpoint(&self) -> Result<(), WriterGone> {
        let (ack, wait) = oneshot::channel();
        if self.sender.tx.send(Msg::Checkpoint(ack)).await.is_err() {
            return Err(WriterGone);
        }
        match wait.await {
            Ok(true) => Ok(()),
            _ => Err(WriterGone),
        }
    }

    /// Beendet den Thread: alle Absender müssen vorher freigegeben sein, sonst wartet `finish` auf sie.
    /// Gibt den ersten Fehler der Senke zurück.
    pub fn finish(mut self) -> Result<WriterStatsSnapshot, StoreError> {
        let shared = Arc::clone(&self.sender.shared);
        let thread = self.thread.take();
        // Der letzte Sender verschwindet hier; der Thread beendet sich, sobald der Kanal leer ist.
        drop(self.sender);
        if let Some(t) = thread {
            let _ = t.join();
        }
        let snapshot = WriterStatsSnapshot::of(&shared.stats);
        let error = shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        match error {
            Some(e) => Err(e),
            None => Ok(snapshot),
        }
    }

    /// Zähler.
    pub fn stats(&self) -> WriterStatsSnapshot {
        WriterStatsSnapshot::of(&self.sender.shared.stats)
    }
}

/// Kopie der Zähler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriterStatsSnapshot {
    /// Geschriebene Batches.
    pub batches: u64,
    /// Geschriebene Seiten.
    pub pages: u64,
    /// Größter Batch in Seiten.
    pub max_batch_pages: usize,
    /// Größte Zahl wartender Nachrichten.
    pub max_queued: usize,
}

impl WriterStatsSnapshot {
    fn of(s: &WriterStats) -> Self {
        WriterStatsSnapshot {
            batches: s.batches.load(Ordering::Relaxed),
            pages: s.pages.load(Ordering::Relaxed),
            max_batch_pages: s.max_batch_pages.load(Ordering::Relaxed),
            max_queued: s.max_queued.load(Ordering::Relaxed),
        }
    }
}

impl WriterSender {
    /// Reserviert einen Platz (wartet, solange der Kanal voll ist – hier greift der Gegendruck).
    pub async fn reserve(&self) -> Result<Slot, WriterGone> {
        match self.tx.clone().reserve_owned().await {
            Ok(permit) => Ok(Slot { permit }),
            Err(_) => Err(WriterGone),
        }
    }

    /// Sendet einen Batch (wartet bei vollem Kanal).
    pub async fn send(&self, batch: WriteBatch) -> Result<(), WriterGone> {
        if is_empty(&batch) {
            return Ok(());
        }
        self.tx
            .send(Msg::Batch(Box::new(batch)))
            .await
            .map_err(|_| WriterGone)
    }

    /// Wartet, bis alles bisher Gesendete geschrieben ist.
    pub async fn flush(&self) -> Result<(), WriterGone> {
        let (ack, wait) = oneshot::channel();
        if self.tx.send(Msg::Flush(ack)).await.is_err() {
            return Err(WriterGone);
        }
        match wait.await {
            Ok(true) => Ok(()),
            _ => Err(WriterGone),
        }
    }

    /// Ob die Senke einen Fehler gemeldet hat.
    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::SeqCst)
    }

    /// Text des ersten Fehlers der Senke.
    pub fn error(&self) -> Option<String> {
        self.shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(ToString::to_string)
    }
}

fn merge(into: &mut WriteBatch, from: WriteBatch) {
    into.new_urls.extend(from.new_urls);
    into.url_updates.extend(from.url_updates);
    into.pages.extend(from.pages);
    into.queue.extend(from.queue);
    into.sitemaps.extend(from.sitemaps);
    into.robots.extend(from.robots);
    into.meta.extend(from.meta);
    into.events.extend(from.events);
    into.counters.extend(from.counters);
    into.url_flags.extend(from.url_flags);
}

fn record_error(shared: &Shared, e: StoreError) {
    let mut slot = shared.error.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        *slot = Some(e);
    }
    shared.failed.store(true, Ordering::SeqCst);
}

fn write_out(
    sink: &mut Box<dyn CrawlSink>,
    batch: &mut WriteBatch,
    pages: &mut usize,
    shared: &Shared,
) {
    if is_empty(batch) {
        return;
    }
    let taken = std::mem::take(batch);
    let n = *pages;
    *pages = 0;
    if shared.failed.load(Ordering::SeqCst) {
        return; // nach einem Fehler nur noch verwerfen
    }
    match sink.write(taken) {
        Ok(()) => {
            let s = &shared.stats;
            s.batches.fetch_add(1, Ordering::Relaxed);
            s.pages.fetch_add(n as u64, Ordering::Relaxed);
            s.max_batch_pages.fetch_max(n, Ordering::Relaxed);
        }
        Err(e) => record_error(shared, e),
    }
}

fn writer_loop(
    mut sink: Box<dyn CrawlSink>,
    mut rx: mpsc::Receiver<Msg>,
    capacity: usize,
    shared: Arc<Shared>,
) {
    let mut batch = WriteBatch::default();
    let mut pages = 0usize;
    let mut first_at: Option<Instant> = None;

    loop {
        // nächste Nachricht: leer → blockierend warten, sonst bis zum Ablauf der Wartezeit
        let msg = if first_at.is_none() {
            match rx.blocking_recv() {
                Some(m) => m,
                None => break,
            }
        } else {
            match rx.try_recv() {
                Ok(m) => m,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
                Err(mpsc::error::TryRecvError::Empty) => {
                    let waited = first_at.map_or(Duration::ZERO, |t| t.elapsed());
                    if waited >= BATCH_MAX_WAIT {
                        write_out(&mut sink, &mut batch, &mut pages, &shared);
                        first_at = None;
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    continue;
                }
            }
        };
        let queued = rx.len() + 1;
        shared.stats.max_queued.fetch_max(queued, Ordering::Relaxed);
        match msg {
            Msg::Batch(b) => {
                pages += b.pages.len();
                merge(&mut batch, *b);
                first_at.get_or_insert_with(Instant::now);
                let limit = if rx.len() * 2 > capacity {
                    BATCH_PAGES_BACKLOG
                } else {
                    BATCH_PAGES
                };
                if pages >= limit {
                    write_out(&mut sink, &mut batch, &mut pages, &shared);
                    first_at = None;
                }
            }
            Msg::Flush(ack) => {
                write_out(&mut sink, &mut batch, &mut pages, &shared);
                first_at = None;
                let _ = ack.send(!shared.failed.load(Ordering::SeqCst));
            }
            Msg::Checkpoint(ack) => {
                write_out(&mut sink, &mut batch, &mut pages, &shared);
                first_at = None;
                let ok = if shared.failed.load(Ordering::SeqCst) {
                    false
                } else if let Err(e) = sink.checkpoint() {
                    record_error(&shared, e);
                    false
                } else {
                    true
                };
                let _ = ack.send(ok);
            }
        }
    }
    write_out(&mut sink, &mut batch, &mut pages, &shared);
    if !shared.failed.load(Ordering::SeqCst) {
        if let Err(e) = sink.checkpoint() {
            record_error(&shared, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{CrawlEvent, Timestamp};

    #[derive(Default)]
    struct Log {
        batches: Vec<WriteBatch>,
        checkpoints: u32,
    }

    struct TestSink {
        log: Arc<Mutex<Log>>,
        delay: Duration,
        fail_at: Option<usize>,
    }

    impl CrawlSink for TestSink {
        fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
            std::thread::sleep(self.delay);
            let mut log = self.log.lock().unwrap();
            if self.fail_at == Some(log.batches.len()) {
                return Err(StoreError::NotImplemented("Testfehler"));
            }
            log.batches.push(batch);
            Ok(())
        }
        fn checkpoint(&mut self) -> Result<(), StoreError> {
            self.log.lock().unwrap().checkpoints += 1;
            Ok(())
        }
    }

    fn sink(delay: Duration, fail_at: Option<usize>) -> (Box<dyn CrawlSink>, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log::default()));
        (
            Box::new(TestSink {
                log: Arc::clone(&log),
                delay,
                fail_at,
            }),
            log,
        )
    }

    fn page_batch(n: usize) -> WriteBatch {
        let mut b = WriteBatch::default();
        for i in 0..n {
            b.pages.push(os_model::PageWrite {
                url_id: os_model::UrlId(i as i64 + 1),
                replace: false,
                fetch: os_model::FetchFacts::default(),
                page: None,
                indexability: os_model::Indexability::Indexable,
                indexability_status: None,
                links: vec![],
                images: vec![],
                canonicals: vec![],
                directives: vec![],
                refresh: vec![],
                hreflang: vec![],
                pagination: vec![],
                headers: vec![],
            });
        }
        b
    }

    fn event(code: &str) -> WriteBatch {
        WriteBatch {
            events: vec![CrawlEvent {
                at: Timestamp::UNIX_EPOCH,
                kind: "info".into(),
                code: code.into(),
                message: String::new(),
            }],
            ..WriteBatch::default()
        }
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn messages_are_merged_in_order_and_flush_waits_for_the_write() {
        let (s, log) = sink(Duration::ZERO, None);
        let w = WriterThread::spawn(s);
        let tx = w.sender();
        for code in ["a", "b", "c"] {
            tx.send(event(code)).await.unwrap();
        }
        tx.flush().await.unwrap();
        {
            let log = log.lock().unwrap();
            let codes: Vec<_> = log
                .batches
                .iter()
                .flat_map(|b| b.events.iter().map(|e| e.code.clone()))
                .collect();
            assert_eq!(codes, ["a", "b", "c"]);
        }
        drop(tx);
        let stats = w.finish().unwrap();
        assert!(stats.batches >= 1);
        assert!(log.lock().unwrap().checkpoints >= 1, "Abschluss-Checkpoint");
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn a_batch_never_exceeds_the_page_limit_without_backlog() {
        let (s, log) = sink(Duration::ZERO, None);
        let w = WriterThread::spawn(s);
        let tx = w.sender();
        for _ in 0..30 {
            tx.send(page_batch(20)).await.unwrap(); // 600 Seiten
        }
        tx.flush().await.unwrap();
        drop(tx);
        let stats = w.finish().unwrap();
        assert_eq!(stats.pages, 600);
        let log = log.lock().unwrap();
        assert!(
            log.batches.len() >= 3,
            "mindestens 3 Batches bei 600 Seiten"
        );
        // Der Rückstau-Fall darf größer werden, bleibt aber unter dem oberen Limit + einer Nachricht.
        assert!(stats.max_batch_pages <= BATCH_PAGES_BACKLOG + 20);
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn a_lone_message_is_written_after_the_maximum_wait() {
        let (s, log) = sink(Duration::ZERO, None);
        let w = WriterThread::spawn(s);
        let tx = w.sender();
        tx.send(page_batch(1)).await.unwrap();
        let start = Instant::now();
        loop {
            if log.lock().unwrap().batches.len() == 1 {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Batch wurde nicht geschrieben"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        drop(tx);
        w.finish().unwrap();
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn full_channel_blocks_the_sender_until_the_writer_catches_up() {
        // Eine langsame Senke und ein kleiner Kanal: `send` darf nur so weit vorauslaufen wie der Kanal groß ist.
        let (s, log) = sink(Duration::from_millis(30), None);
        let w = WriterThread::spawn_with(s, 4);
        let tx = w.sender();
        let sent = Arc::new(AtomicUsize::new(0));
        let producer = {
            let tx = tx.clone();
            let sent = Arc::clone(&sent);
            tokio::spawn(async move {
                for _ in 0..120 {
                    tx.send(page_batch(BATCH_PAGES)).await.unwrap();
                    sent.fetch_add(1, Ordering::SeqCst);
                }
            })
        };
        // Während der Produzent läuft: gesendet - geschrieben bleibt durch Kanal + Batch begrenzt.
        let mut max_ahead = 0usize;
        while !producer.is_finished() {
            let written: usize = log
                .lock()
                .unwrap()
                .batches
                .iter()
                .map(|b| b.pages.len() / BATCH_PAGES)
                .sum();
            let ahead = sent.load(Ordering::SeqCst).saturating_sub(written);
            max_ahead = max_ahead.max(ahead);
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        producer.await.unwrap();
        tx.flush().await.unwrap();
        // Kanal (4) + sich ansammelnder Batch (bis 5 Nachrichten) + gerade geschriebener Batch (bis 5)
        assert!(
            max_ahead <= 4 + 5 + 5 + 2,
            "Produzent lief {max_ahead} von 120 Nachrichten voraus (Kanal 4)"
        );
        drop(tx);
        let stats = w.finish().unwrap();
        assert_eq!(stats.pages, 120 * BATCH_PAGES as u64);
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn a_failing_sink_is_reported_and_does_not_block_senders() {
        let (s, _log) = sink(Duration::ZERO, Some(0));
        let w = WriterThread::spawn_with(s, 2);
        let tx = w.sender();
        for _ in 0..20 {
            // Jeder Batch ist groß genug für einen eigenen Schreibvorgang.
            let _ = tx.send(page_batch(BATCH_PAGES)).await;
        }
        let flushed = tx.flush().await;
        assert!(flushed.is_err());
        assert!(tx.failed());
        assert_eq!(
            tx.error().as_deref().map(|e| e.contains("Testfehler")),
            Some(true)
        );
        drop(tx);
        assert!(w.finish().is_err());
    }

    // Infrastruktur (PERF-003), kein Spec-Tag.
    #[tokio::test]
    async fn reserved_slot_sends_without_waiting_and_keeps_order() {
        let (s, log) = sink(Duration::ZERO, None);
        let w = WriterThread::spawn(s);
        let tx = w.sender();
        let slot = tx.reserve().await.unwrap();
        // Zwischen Reservierung und Senden darf ein anderer Absender dazwischenkommen – der Slot behält
        // seinen Platz im Kanal nur, wenn er vor dem anderen reserviert wurde.
        slot.send(event("erst"));
        tx.send(event("dann")).await.unwrap();
        tx.flush().await.unwrap();
        let codes: Vec<_> = log
            .lock()
            .unwrap()
            .batches
            .iter()
            .flat_map(|b| b.events.iter().map(|e| e.code.clone()))
            .collect();
        assert_eq!(codes, ["erst", "dann"]);
        drop(tx);
        w.finish().unwrap();
    }
}
