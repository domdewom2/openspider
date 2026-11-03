//! Defekte `.xls`-Dateien im Listen- und SERP-Import (M2-N33, RUSTSEC-2026-0436): Der Import liefert einen
//! sauberen Fehler, bricht nicht mit einem Panic ab, hängt nicht und belegt nur begrenzt Speicher.
//! Fixtures und ihr Aufbau: `tests/fixtures/crawl-list-corrupt/README.md`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use os_engine::{list_input, serp_input, EngineError};

/// Zählt je Thread die netto belegten Bytes und deren Höchststand (andere Tests laufen parallel).
struct ThreadPeak;

thread_local! {
    static NET: Cell<isize> = const { Cell::new(0) };
    static PEAK: Cell<isize> = const { Cell::new(0) };
}

fn track(delta: isize) {
    let _ = NET.try_with(|net| {
        let now = net.get() + delta;
        net.set(now);
        let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
}

unsafe impl GlobalAlloc for ThreadPeak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            track(layout.size() as isize);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            track(layout.size() as isize);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        track(-(layout.size() as isize));
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            track(new_size as isize - layout.size() as isize);
        }
        p
    }
}

#[global_allocator]
static ALLOC: ThreadPeak = ThreadPeak;

/// Obergrenze für den Speicher eines Imports einer Datei von höchstens 1,2 KB.
const MAX_PEAK: isize = 16 * 1024 * 1024;
/// Obergrenze für die Dauer (calamine 0.36.1 lief bei `difat-loop.xls` endlos).
const MAX_TIME: Duration = Duration::from_secs(30);

const CORRUPT: [&str; 4] = [
    "truncated-header.xls",
    "truncated-directory.xls",
    "difat-loop.xls",
    "no-fat.xls",
];

fn fixture(name: &str) -> PathBuf {
    os_testsite::fixtures_root()
        .join("crawl-list-corrupt")
        .join(name)
}

/// Führt `read` in einem eigenen Thread aus; liefert Ergebnis und Speicher-Höchststand dieses Threads.
/// Ein Panic im Thread oder eine Zeitüberschreitung lässt den Test scheitern.
fn bounded<T: Send + 'static>(
    what: String,
    read: impl FnOnce() -> T + Send + 'static,
) -> (T, isize) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        NET.with(|n| n.set(0));
        PEAK.with(|p| p.set(0));
        let out = read();
        let peak = PEAK.with(Cell::get);
        let _ = tx.send((out, peak));
    });
    match rx.recv_timeout(MAX_TIME) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("{what}: kein Ergebnis nach {MAX_TIME:?}"),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{what}: Import ist abgestürzt (Panic)")
        }
    }
}

fn assert_file_error(what: &str, result: Result<impl std::fmt::Debug, EngineError>, peak: isize) {
    match result {
        Err(EngineError::Invalid(msg)) => assert!(!msg.is_empty(), "{what}: leere Fehlermeldung"),
        other => panic!("{what}: erwartet Fehler der Datei, erhalten {other:?}"),
    }
    assert!(
        peak < MAX_PEAK,
        "{what}: {peak} Byte belegt (Grenze {MAX_PEAK})"
    );
}

// spec: CRAWL-027
#[test]
fn list_import_rejects_corrupt_xls_without_panic_hang_or_unbounded_memory() {
    for name in CORRUPT {
        let path = fixture(name);
        assert!(path.is_file(), "Fixture fehlt: {}", path.display());
        let (items, peak) = bounded(format!("read_items {name}"), {
            let path = path.clone();
            move || list_input::read_items(&path)
        });
        assert_file_error(&format!("read_items {name}"), items, peak);
        if let Err(EngineError::Invalid(msg)) = list_input::read_items(&path) {
            assert!(
                msg.contains(name),
                "{name}: Meldung nennt die Datei nicht: {msg}"
            );
        }
        let (list, peak) = bounded(format!("read_list {name}"), move || {
            list_input::read_list(&path)
        });
        assert_file_error(&format!("read_list {name}"), list, peak);
    }
}

// spec: CRAWL-031
#[test]
fn serp_import_rejects_corrupt_xls_without_panic_hang_or_unbounded_memory() {
    for name in CORRUPT {
        let path = fixture(name);
        let (rows, peak) = bounded(format!("serp_input::read {name}"), move || {
            serp_input::read(&path)
        });
        assert_file_error(&format!("serp_input::read {name}"), rows, peak);
    }
}

// spec: CRAWL-027
#[test]
fn intact_workbooks_still_read_through_the_guard() {
    // Gegenprobe: die bestehenden Fixtures laufen unverändert durch denselben Pfad.
    let xlsx = os_testsite::fixtures_root()
        .join("crawl-list-input")
        .join("urls.xlsx");
    let parsed = list_input::read_list(&xlsx).unwrap();
    assert!(!parsed.urls.is_empty());
    for name in ["serp.xls", "serp.xlsx"] {
        let path = os_testsite::fixtures_root()
            .join("crawl-m2-list")
            .join(name);
        assert_eq!(serp_input::read(&path).unwrap().len(), 5, "{name}");
    }
}
