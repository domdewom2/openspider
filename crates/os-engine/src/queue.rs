//! Warteschlange der Engine mit begrenztem RAM-Fenster (K-78 b).
//!
//! * **Reihenfolge:** `(Priorität, Tiefe, seq)` aufsteigend, `NO_DEPTH` sortiert hinter jeder echten Tiefe. Es wird
//!   immer der kleinste Schlüssel gestartet (CRAWL-005).
//! * **Zwei Zustände.** Solange die Warteschlange klein ist (und in allen Tests, `Frontier::default()`), liegt sie
//!   ganz im RAM. Mit [`Frontier::enable_spill`] gilt: Wächst sie über das Doppelte des Fensters ([`QUEUE_WINDOW`]),
//!   wird sie *ausgelagert*: Im RAM bleiben die `W` Einträge mit den kleinsten Schlüsseln, alle weiteren stehen nur
//!   noch in der Tabelle `queue` der Crawl-Datei (der Writer schreibt Push und Pop ohnehin immer). Neue Einträge mit
//!   einem Schlüssel hinter der Grenze `floor` gehen gar nicht erst ins RAM. Ist das Fenster zur Hälfte leer, lädt
//!   ein Hintergrundauftrag den nächsten Block (`queue JOIN urls` hinter `floor`, Index `queue_order`) nach, nachdem
//!   der Writer geleert wurde (`flush`).
//! * **Warum das ohne Rückfrage an die Datei sicher ist.** Jeder Eintrag, der ins RAM kommt oder es verlässt, wird
//!   unter der Kern-Sperre bewegt, und der Batch mit seinem `QueueOp::Push` liegt zu diesem Zeitpunkt schon im Kanal
//!   zum Writer (Entdeckung und Senden geschehen unter derselben Sperre, `worker.rs`). Ein Auftrag merkt sich beim
//!   Start die nächste `seq` (`watermark`): Alle Einträge mit kleinerer `seq` stehen nach dem `flush` in der Datei.
//!   Nur solche Einträge dürfen das RAM verlassen. Alles, was während eines Auftrags neu hinzukommt oder
//!   neu einsortiert wird, bleibt im RAM (der Block könnte es nicht enthalten); die nächste Auslagerung nimmt es
//!   später mit.
//! * **Grenze `floor`:** Alle nur in der Datei stehenden Einträge haben einen Schlüssel **größer** als `floor`; nur
//!   RAM-Einträge mit Schlüssel `<= floor` werden gestartet. Ist der kleinste RAM-Eintrag größer, kommt
//!   [`Popped::NeedRefill`] und die Steuerschleife wartet auf den Auftrag.
//! * **Was nicht ausgelagert wird:** Einträge, deren Zusatzfelder sich nicht aus der Datei ergeben:
//!   Weiterleitungsziele (`hops > 0`), Einträge einer eigenen Cookie-Sitzung, „Neu crawlen“ (`replace`), Einträge
//!   mit ungewöhnlichem Schlüssel (Priorität 0 ohne Tiefe, Priorität 1 mit Tiefe) und Kennungen jenseits von
//!   [`MAX_SPILL_ID`]. Der Zweck (Seite, Ressource, extern) ergibt sich aus `urls.is_internal` und einer Bitmenge der
//!   Ressourcen-Kennungen.
//! * **Kürzerer Weg (`improve`):** Ein ausgelagerter Eintrag, dessen URL über einen kürzeren Weg erneut gefunden
//!   wird, bekommt einen neuen Schlüssel; der Aufrufer schreibt wie bisher einen neuen `QueueOp::Push` (die Zeile in
//!   `queue` wird ersetzt). Fällt der neue Schlüssel in das Fenster (oder läuft gerade ein Auftrag), kommt der Eintrag
//!   mit der Adresse des Aufrufers ins RAM.
//!
//! Alle Funktionen hier rechnen nur im RAM; die Datenbankzugriffe laufen im Hintergrund ohne Sperre
//! ([`crate::queue_disk`], [`crate::queue_maint`]).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use os_model::UrlId;

use crate::diag;
use crate::frontier::{Purpose, QueueItem, NO_DEPTH};

/// Größe des RAM-Fensters W der Warteschlange (K-78 b): Zahl der Einträge mit den kleinsten Schlüsseln, die im RAM
/// stehen. Begründung (Messung `perf-003`, `docs/plan/M1-acceptance/loadtest/probe-p33/README.md`): Ein Eintrag im
/// RAM kostet rund 200 Byte (Eintrag, Adresse, Tabellenplatz, Sortierschlüssel). Die Auslagerung beginnt bei `2 W`
/// Einträgen, die Spitze im RAM liegt also bei rund `2 W` (mit W = 20 000 gemessen: 42 000 Einträge, 8,9 MB samt
/// Sortierstruktur 10,5 MB), danach pendelt sie zwischen `W / 2` und `3 W / 2`. Bei 3 000 Abrufen je Sekunde wird mit
/// W = 10 000 alle zwei bis drei Sekunden ein Block von 10 000 Zeilen gelesen (einige zehn Millisekunden im
/// Hintergrund, ein Auftrag je Block: `flush` des Writers und eine Abfrage `queue JOIN urls` über den Index
/// `queue_order`); der Durchsatz ändert sich dadurch nicht messbar. Kleiner erhöht die Zahl der Aufträge, größer
/// verschenkt Speicher (K-78 nennt die Größenordnung 20 000).
pub const QUEUE_WINDOW: usize = 10_000;

/// Kennungen ab diesem Wert werden nicht ausgelagert (die Bitmengen bleiben klein: 16 MiB je Menge im Höchstfall).
pub const MAX_SPILL_ID: i64 = 1 << 27;

/// Pause nach einem fehlgeschlagenen Auftrag, bevor die Steuerschleife den nächsten startet.
const RETRY_AFTER: Duration = Duration::from_millis(250);

static WINDOW_OVERRIDE: AtomicUsize = AtomicUsize::new(0);

/// Ersetzt die Fenstergröße für alle danach gestarteten Crawls (nur für Tests mit kleinem Fenster; ein Test, der das
/// benutzt, setzt sie danach mit `None` zurück und läuft in einer eigenen Testdatei).
#[doc(hidden)]
pub fn set_window_override(w: Option<usize>) {
    WINDOW_OVERRIDE.store(w.unwrap_or(0), Ordering::SeqCst);
}

/// Wirksame Fenstergröße: [`QUEUE_WINDOW`] oder der Wert aus [`set_window_override`].
pub fn queue_window() -> usize {
    match WINDOW_OVERRIDE.load(Ordering::SeqCst) {
        0 => QUEUE_WINDOW,
        w => w,
    }
}

/// Sortierschlüssel eines Eintrags.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Key {
    pub priority: u8,
    pub depth: u32,
    pub seq: u64,
    pub id: i64,
}

impl Key {
    pub fn of(item: &QueueItem) -> Key {
        Key {
            priority: item.priority,
            depth: item.depth.unwrap_or(NO_DEPTH),
            seq: item.seq,
            id: item.id.0,
        }
    }

    pub fn of_row(row: &DiskRow) -> Key {
        Key {
            priority: row.priority,
            depth: row.depth.unwrap_or(NO_DEPTH),
            seq: row.seq,
            id: row.id,
        }
    }
}

/// Eine Zeile der Tabelle `queue` samt Adresse (`queue JOIN urls`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiskRow {
    pub id: i64,
    pub seq: u64,
    pub depth: Option<u32>,
    pub priority: u8,
    pub url: String,
    pub internal: bool,
}

/// Menge von Kennungen: Bitmenge für `0 <= id < MAX_SPILL_ID`, darüber eine Hashmenge.
#[derive(Default)]
pub(crate) struct IdSet {
    words: Vec<u64>,
    ones: usize,
    big: HashSet<i64>,
}

impl IdSet {
    fn small(id: i64) -> Option<u64> {
        (0..MAX_SPILL_ID).contains(&id).then_some(id as u64)
    }

    pub fn get(&self, id: i64) -> bool {
        match Self::small(id) {
            Some(i) => self
                .words
                .get((i >> 6) as usize)
                .is_some_and(|w| (w >> (i & 63)) & 1 == 1),
            None => self.big.contains(&id),
        }
    }

    /// Setzt die Kennung; `true`, wenn sie vorher nicht enthalten war.
    pub fn set(&mut self, id: i64) -> bool {
        match Self::small(id) {
            Some(i) => {
                let w = (i >> 6) as usize;
                if self.words.len() <= w {
                    self.words.resize(w + 1, 0);
                }
                let bit = 1u64 << (i & 63);
                let was = self.words[w] & bit != 0;
                self.words[w] |= bit;
                if !was {
                    self.ones += 1;
                }
                !was
            }
            None => self.big.insert(id),
        }
    }

    /// Entfernt die Kennung; `true`, wenn sie enthalten war.
    pub fn clear(&mut self, id: i64) -> bool {
        match Self::small(id) {
            Some(i) => {
                let Some(word) = self.words.get_mut((i >> 6) as usize) else {
                    return false;
                };
                let bit = 1u64 << (i & 63);
                let was = *word & bit != 0;
                *word &= !bit;
                if was {
                    self.ones -= 1;
                }
                was
            }
            None => self.big.remove(&id),
        }
    }

    pub fn len(&self) -> usize {
        self.ones + self.big.len()
    }

    fn clear_all(&mut self) {
        self.words.iter_mut().for_each(|w| *w = 0);
        self.ones = 0;
        self.big.clear();
    }

    fn bytes(&self) -> usize {
        self.words.capacity() * 8 + self.big.capacity() * 9
    }
}

/// Ergebnis von [`Frontier::pop_checked`].
pub enum Popped {
    /// Nächster Eintrag.
    Item(QueueItem),
    /// Nichts wartet.
    Empty,
    /// Es wartet etwas, aber nur in der Datei: erst nachladen.
    NeedRefill,
}

/// Art der Hintergrundarbeit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobKind {
    /// Die Warteschlange ist über das Doppelte des Fensters gewachsen: alles hinter dem Fenster auslagern.
    Activate,
    /// Das Fenster ist zur Hälfte leer: nächsten Block aus der Datei laden.
    Refill,
}

/// Was die Steuerschleife für einen Auftrag wissen muss (ohne Sperre benutzbar).
#[derive(Debug, Clone)]
pub(crate) struct JobPlan {
    pub kind: JobKind,
    pub path: PathBuf,
    /// Nachladen: der Block liegt hinter diesem Schlüssel.
    pub after: Key,
    /// Nachladen: Blockgröße.
    pub limit: usize,
}

/// Ergebnis der Dateiarbeit eines Auftrags.
pub(crate) enum JobData {
    /// Zahl der Zeilen in `queue` nach dem `flush`.
    Counted(u64),
    /// Der gelesene Block.
    Block(Vec<DiskRow>),
}

struct Job {
    kind: JobKind,
    /// Alle Einträge mit kleinerer `seq` stehen nach dem `flush` des Auftrags in der Datei.
    watermark: u64,
    /// Auslagern: Grenze des künftigen Fensters.
    floor_candidate: Key,
}

struct Spill {
    window: usize,
    path: PathBuf,
    /// Kennungen, die nur in der Tabelle `queue` stehen.
    disk: IdSet,
    /// Kennungen, deren Zweck `Resource` ist (für ausgelagerte Einträge).
    resource: IdSet,
    /// Grenze der startbaren RAM-Einträge; gilt, solange `disk` nicht leer ist.
    floor: Key,
    /// Ungefähre Zahl der RAM-Einträge mit Schlüssel `<= floor` (Auslöser für das vorausschauende Nachladen).
    poppable: usize,
    /// RAM-Einträge, die nicht ausgelagert werden dürfen.
    pinned: usize,
    /// Ein Pop fand nur Einträge hinter der Grenze: sofort nachladen.
    starved: bool,
    /// Beginn der Zeit, in der Pops wegen fehlendem Block leer ausgingen (Messzähler `q_starved_*`).
    starved_at: Option<Instant>,
    job: Option<Job>,
    /// Ab dieser Größe des RAM-Anteils lohnt sich das Auslagern (erst).
    activate_at: usize,
    /// Die Senke schreibt `queue` nicht (oder die Datei ist unlesbar): nie auslagern.
    disabled: bool,
    retry_at: Option<Instant>,
}

/// Warteschlange mit Neueinsortierung (Einträge im Heap werden bei Änderung ungültig, `seq` entscheidet).
#[derive(Default)]
pub struct Frontier {
    heap: BinaryHeap<Reverse<Key>>,
    items: HashMap<i64, QueueItem>,
    /// Heap-Bytes der Adressen im RAM (auf 16 Byte gerundet, Schätzung für die Speicherzähler).
    url_bytes: usize,
    spill: Option<Box<Spill>>,
}

/// Was ein Aufrufer von `improve` über die URL weiß, falls der Eintrag nur in der Datei steht.
pub struct ImproveHint<'a> {
    /// Kleinste bisher bekannte Tiefe der URL (`NO_DEPTH`, wenn nur über eine Sitemap bekannt) *vor* dem Fund.
    pub old_depth: u32,
    /// URL Encoded Address.
    pub url: &'a str,
    /// Im Crawl-Bereich.
    pub internal: bool,
}

/// Heap-Anteil einer Adresse: Länge, auf die Allokator-Granularität (16 Byte) aufgerundet.
fn heap_bytes_of(url: &str) -> usize {
    url.len().div_ceil(16) * 16
}

/// Schlüssel der Form, die die Tabelle `queue` in derselben Reihenfolge sortiert wie der Speicher
/// (Priorität 0 mit Tiefe, Priorität 1 ohne Tiefe).
fn normal_shape(item: &QueueItem) -> bool {
    (item.priority == 0 && item.depth.is_some()) || (item.priority == 1 && item.depth.is_none())
}

/// Darf der Eintrag das RAM verlassen (seine Zusatzfelder ergeben sich aus der Datei)?
fn spillable_item(item: &QueueItem) -> bool {
    (0..MAX_SPILL_ID).contains(&item.id.0)
        && item.hops == 0
        && item.session == 0
        && !item.replace
        && normal_shape(item)
}

fn purpose_of(internal: bool, resource: bool) -> Purpose {
    if !internal {
        Purpose::External
    } else if resource {
        Purpose::Resource
    } else {
        Purpose::Page
    }
}

impl Frontier {
    /// Schaltet die Auslagerung ein: `window` Einträge im RAM, der Rest in der Tabelle `queue` der Datei `path`.
    pub fn enable_spill(&mut self, window: usize, path: PathBuf) {
        let window = window.max(2);
        self.spill = Some(Box::new(Spill {
            window,
            path,
            disk: IdSet::default(),
            resource: IdSet::default(),
            floor: Key {
                priority: 0,
                depth: 0,
                seq: 0,
                id: 0,
            },
            poppable: 0,
            pinned: self.items.values().filter(|i| !spillable_item(i)).count(),
            starved: false,
            starved_at: None,
            job: None,
            activate_at: 2 * window,
            disabled: false,
            retry_at: None,
        }));
    }

    /// Ob die Auslagerung eingeschaltet ist.
    pub fn spill_enabled(&self) -> bool {
        self.spill.is_some()
    }

    /// Grenze der startbaren RAM-Einträge, solange Einträge nur in der Datei stehen.
    fn gate(&self) -> Option<Key> {
        let s = self.spill.as_ref()?;
        (s.disk.len() > 0).then_some(s.floor)
    }

    /// Plant eine URL ein.
    pub fn push(&mut self, item: QueueItem) {
        if let Some(s) = self.spill.as_mut() {
            if (0..MAX_SPILL_ID).contains(&item.id.0) {
                if item.purpose == Purpose::Resource {
                    s.resource.set(item.id.0);
                } else {
                    s.resource.clear(item.id.0);
                }
                // ein Eintrag im RAM ersetzt einen ausgelagerten derselben URL (der Push überschreibt die Zeile)
                s.disk.clear(item.id.0);
            }
            // hinter der Grenze und ohne laufenden Auftrag: nur in die Datei (der Batch mit dem Push liegt schon
            // im Kanal, siehe Kopf der Datei)
            if s.job.is_none()
                && s.disk.len() > 0
                && spillable_item(&item)
                && Key::of(&item) > s.floor
            {
                s.disk.set(item.id.0);
                diag::inc(&diag::STATS.q_spilled);
                return;
            }
        }
        self.insert_ram(item);
    }

    fn insert_ram(&mut self, item: QueueItem) {
        let key = Key::of(&item);
        let spillable = spillable_item(&item);
        self.heap.push(Reverse(key));
        self.url_bytes += heap_bytes_of(&item.url);
        if let Some(old) = self.items.insert(item.id.0, item) {
            self.url_bytes = self.url_bytes.saturating_sub(heap_bytes_of(&old.url));
            if let Some(s) = self.spill.as_mut() {
                if !spillable_item(&old) {
                    s.pinned = s.pinned.saturating_sub(1);
                }
            }
        }
        if let Some(s) = self.spill.as_mut() {
            if !spillable {
                s.pinned += 1;
            }
            if s.disk.len() > 0 && key <= s.floor {
                s.poppable += 1;
            }
        }
    }

    fn remove_ram(&mut self, id: i64) -> Option<QueueItem> {
        let item = self.items.remove(&id)?;
        self.url_bytes = self.url_bytes.saturating_sub(heap_bytes_of(&item.url));
        if let Some(s) = self.spill.as_mut() {
            if !spillable_item(&item) {
                s.pinned = s.pinned.saturating_sub(1);
            }
        }
        Some(item)
    }

    /// Nimmt die nächste URL; `None`, wenn nichts startbar ist (auch wenn noch Einträge in der Datei stehen, dazu
    /// [`pop_checked`](Self::pop_checked)).
    pub fn pop(&mut self) -> Option<QueueItem> {
        match self.pop_checked() {
            Popped::Item(i) => Some(i),
            Popped::Empty | Popped::NeedRefill => None,
        }
    }

    /// Nimmt die nächste URL und meldet, ob erst aus der Datei nachgeladen werden muss.
    pub fn pop_checked(&mut self) -> Popped {
        let gate = self.gate();
        while let Some(&Reverse(key)) = self.heap.peek() {
            if !self.items.get(&key.id).is_some_and(|i| i.seq == key.seq) {
                self.heap.pop(); // ungültig (neu einsortiert oder schon genommen)
                continue;
            }
            if gate.is_some_and(|f| key > f) {
                return self.starve();
            }
            self.heap.pop();
            if let Some(s) = self.spill.as_mut() {
                s.poppable = s.poppable.saturating_sub(1);
            }
            return match self.remove_ram(key.id) {
                Some(i) => Popped::Item(i),
                None => Popped::Empty,
            };
        }
        if gate.is_some() {
            self.starve()
        } else {
            Popped::Empty
        }
    }

    fn starve(&mut self) -> Popped {
        if let Some(s) = self.spill.as_mut() {
            s.starved = true;
            s.starved_at.get_or_insert_with(Instant::now);
        }
        Popped::NeedRefill
    }

    /// Sortiert eine wartende URL im RAM neu ein, wenn der neue Schlüssel besser ist. `new_seq` ist eine frische
    /// Reihenfolgenummer. Liefert den neuen Eintrag (für die Warteschlange der Datei). Für Einträge, die nur in der
    /// Datei stehen, [`improve_with`](Self::improve_with).
    pub fn improve(
        &mut self,
        id: UrlId,
        priority: u8,
        depth: Option<u32>,
        new_seq: u64,
    ) -> Option<QueueItem> {
        let floor = self.gate();
        let item = self.items.get_mut(&id.0)?;
        let better = priority < item.priority
            || (priority == item.priority
                && depth.unwrap_or(NO_DEPTH) < item.depth.unwrap_or(NO_DEPTH));
        if !better {
            return None;
        }
        let was_ready = floor.is_none_or(|f| Key::of(item) <= f);
        item.priority = priority;
        item.depth = depth;
        item.seq = new_seq;
        let snapshot = item.clone();
        let key = Key::of(&snapshot);
        self.heap.push(Reverse(key));
        if let (Some(f), Some(s)) = (floor, self.spill.as_mut()) {
            if !was_ready && key <= f {
                s.poppable += 1;
            }
        }
        Some(snapshot)
    }

    /// Wie [`improve`](Self::improve), kennt aber auch Einträge, die nur in der Datei stehen: `hint` beschreibt die URL
    /// so, wie der Aufrufer sie gerade gefunden hat.
    pub fn improve_with(
        &mut self,
        id: UrlId,
        priority: u8,
        depth: Option<u32>,
        new_seq: u64,
        hint: &ImproveHint<'_>,
    ) -> Option<QueueItem> {
        if self.items.contains_key(&id.0) {
            return self.improve(id, priority, depth, new_seq);
        }
        let s = self.spill.as_mut()?;
        if !s.disk.get(id.0) {
            return None;
        }
        // Der Eintrag in der Datei hat (bei normalem Schlüssel) die bisher bekannte Tiefe der URL.
        let old_priority = u8::from(hint.old_depth == NO_DEPTH);
        let better = priority < old_priority
            || (priority == old_priority && depth.unwrap_or(NO_DEPTH) < hint.old_depth);
        if !better {
            return None;
        }
        let item = QueueItem {
            id,
            url: hint.url.into(),
            depth,
            priority,
            seq: new_seq,
            purpose: purpose_of(hint.internal, s.resource.get(id.0)),
            hops: 0,
            session: 0,
            replace: false,
        };
        // Im Fenster (oder während eines Auftrags, dessen Block die neue Zeile vielleicht nicht enthält) kommt der
        // Eintrag ins RAM; sonst bleibt er in der Datei, der neue Push ersetzt die Zeile.
        if s.job.is_some() || Key::of(&item) <= s.floor {
            s.disk.clear(id.0);
            diag::inc(&diag::STATS.q_improved_disk);
            self.insert_ram(item.clone());
        }
        Some(item)
    }

    /// Anzahl wartender URLs (RAM und Datei).
    pub fn len(&self) -> usize {
        self.items.len() + self.disk_len()
    }

    /// Ob nichts wartet.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.disk_len() == 0
    }

    /// Ob die URL wartet.
    pub fn contains(&self, id: UrlId) -> bool {
        self.items.contains_key(&id.0) || self.spill.as_ref().is_some_and(|s| s.disk.get(id.0))
    }

    /// Einträge, die nur in der Tabelle `queue` stehen.
    pub fn disk_len(&self) -> usize {
        self.spill.as_ref().map_or(0, |s| s.disk.len())
    }

    /// Einträge im RAM.
    pub fn ram_len(&self) -> usize {
        self.items.len()
    }

    /// Geschätzte Größe für die Speicherzähler: `(Einträge im RAM, Bytes der Einträge samt Adressen und Bitmengen,
    /// Einträge der Sortierstruktur, Bytes der Sortierstruktur, Einträge nur in der Datei, nicht auslagerbare
    /// Einträge im RAM)`.
    pub fn memory(&self) -> (u64, u64, u64, u64, u64, u64) {
        let slot = std::mem::size_of::<(i64, QueueItem)>() + 1;
        let bits = self
            .spill
            .as_ref()
            .map_or(0, |s| s.disk.bytes() + s.resource.bytes());
        let items_bytes = self.items.capacity() * slot + self.url_bytes + bits;
        let heap_bytes = self.heap.capacity() * std::mem::size_of::<Reverse<Key>>();
        (
            self.items.len() as u64,
            items_bytes as u64,
            self.heap.len() as u64,
            heap_bytes as u64,
            self.disk_len() as u64,
            self.spill.as_ref().map_or(0, |s| s.pinned) as u64,
        )
    }

    // ---------------------------------------------------------------------------------------------------
    // Aufträge

    /// Fragt, ob Auslagern oder Nachladen fällig ist, und markiert den Auftrag als laufend. `next_seq` ist die
    /// nächste freie `seq` des Kerns. Es läuft höchstens ein Auftrag zugleich.
    pub(crate) fn begin_job(&mut self, next_seq: u64, now: Instant) -> Option<JobPlan> {
        let ram = self.items.len();
        let s = self.spill.as_mut()?;
        if s.job.is_some() || s.retry_at.is_some_and(|t| now < t) {
            return None;
        }
        let disk = s.disk.len();
        if disk > 0 {
            if !(s.starved || s.poppable <= s.window / 2) {
                return None;
            }
            s.job = Some(Job {
                kind: JobKind::Refill,
                watermark: next_seq,
                floor_candidate: s.floor,
            });
            return Some(JobPlan {
                kind: JobKind::Refill,
                path: s.path.clone(),
                after: s.floor,
                limit: s.window,
            });
        }
        if s.disabled || ram <= s.activate_at {
            return None;
        }
        // Grenze des künftigen Fensters: der W-te kleinste Schlüssel
        let window = s.window;
        let mut keys: Vec<Key> = self.items.values().map(Key::of).collect();
        let (_, &mut cutoff, _) = keys.select_nth_unstable(window - 1);
        let s = self.spill.as_mut()?;
        s.job = Some(Job {
            kind: JobKind::Activate,
            watermark: next_seq,
            floor_candidate: cutoff,
        });
        Some(JobPlan {
            kind: JobKind::Activate,
            path: s.path.clone(),
            after: cutoff,
            limit: 0,
        })
    }

    /// Nie wieder auslagern (die Datei ist unlesbar): die Warteschlange bleibt im RAM.
    pub(crate) fn stop_spilling(&mut self) {
        if let Some(s) = self.spill.as_mut() {
            s.disabled = true;
            diag::inc(&diag::STATS.q_spill_off);
        }
    }

    /// Der Auftrag ist fehlgeschlagen (Writer, Datei): nichts ändert sich, ein neuer Versuch folgt nach einer Pause.
    pub(crate) fn abort_job(&mut self, now: Instant) {
        if let Some(s) = self.spill.as_mut() {
            s.job = None;
            s.retry_at = Some(now + RETRY_AFTER);
        }
    }

    /// Der Auftrag hat seine Dateiarbeit erledigt: Ergebnis übernehmen.
    pub(crate) fn finish_job(&mut self, data: JobData) {
        let Some(job) = self.spill.as_mut().and_then(|s| s.job.take()) else {
            return;
        };
        match (job.kind, data) {
            (JobKind::Activate, JobData::Counted(rows)) => self.finish_activate(&job, rows),
            (JobKind::Refill, JobData::Block(block)) => self.finish_refill(&job, block),
            _ => {}
        }
        self.rebuild();
        if let Some(s) = self.spill.as_mut() {
            s.starved = false;
            s.retry_at = None;
            if let Some(since) = s.starved_at.take() {
                diag::record_starved(
                    diag::STATS
                        .q_refill_jobs
                        .load(Ordering::Relaxed)
                        .saturating_sub(1),
                    diag::nanos(since),
                );
            }
        }
    }

    fn finish_activate(&mut self, job: &Job, rows: u64) {
        let victims = self.victims(job.floor_candidate, job.watermark);
        let Some(s) = self.spill.as_mut() else {
            return;
        };
        if (victims.len() as u64) > rows {
            // Die Senke hat die Einträge nicht in `queue` geschrieben: alles bleibt im RAM, nie wieder auslagern.
            s.disabled = true;
            diag::inc(&diag::STATS.q_spill_off);
            return;
        }
        let evicted = victims.len();
        s.floor = job.floor_candidate;
        for id in &victims {
            s.disk.set(*id);
        }
        // Schafft das Auslagern kaum etwas (fast alles ist nicht auslagerbar), lohnt es erst bei mehr Einträgen.
        s.activate_at = if evicted < s.window / 4 {
            self.items.len() + s.window
        } else {
            2 * s.window
        };
        for id in victims {
            self.remove_ram(id);
        }
        diag::inc(&diag::STATS.q_trims);
        diag::STATS
            .q_spilled
            .fetch_add(evicted as u64, Ordering::Relaxed);
    }

    /// Spill-fähige RAM-Einträge hinter `floor`, die nachweislich in der Datei stehen (`seq < watermark`).
    fn victims(&self, floor: Key, watermark: u64) -> Vec<i64> {
        self.items
            .values()
            .filter(|i| i.seq < watermark && spillable_item(i) && Key::of(i) > floor)
            .map(|i| i.id.0)
            .collect()
    }

    fn finish_refill(&mut self, job: &Job, block: Vec<DiskRow>) {
        let exhausted;
        let mut last: Option<Key> = None;
        {
            let Some(s) = self.spill.as_ref() else {
                return;
            };
            exhausted = block.len() < s.window;
        }
        let mut loaded = 0u64;
        for row in block {
            last = Some(Key::of_row(&row));
            let Some(s) = self.spill.as_mut() else {
                return;
            };
            // Zeilen, die nicht (mehr) ausgelagert sind, sind veraltet oder stehen schon im RAM
            if !s.disk.clear(row.id) {
                continue;
            }
            let item = QueueItem {
                id: UrlId(row.id),
                url: row.url.into_boxed_str(),
                depth: row.depth,
                priority: row.priority,
                seq: row.seq,
                purpose: purpose_of(row.internal, s.resource.get(row.id)),
                hops: 0,
                session: 0,
                replace: false,
            };
            self.insert_ram(item);
            loaded += 1;
        }
        diag::inc(&diag::STATS.q_refills);
        diag::STATS.q_loaded.fetch_add(loaded, Ordering::Relaxed);
        let Some(s) = self.spill.as_mut() else {
            return;
        };
        if exhausted {
            // Alles hinter der Grenze wurde gelesen: Was die Datei nicht kennt, ist verloren (darf nicht vorkommen).
            let lost = s.disk.len();
            if lost > 0 {
                tracing::error!(
                    "Warteschlange: {lost} ausgelagerte Einträge fehlen in der Crawl-Datei"
                );
                diag::STATS.q_lost.fetch_add(lost as u64, Ordering::Relaxed);
                s.disk.clear_all();
            }
            return;
        }
        let Some(last) = last else {
            return;
        };
        s.floor = last;
        // Was während des Auftrags neu hinzukam und nachweislich in der Datei steht, geht hinter dem neuen Fenster
        // wieder aus dem RAM.
        let victims = self.victims(last, job.watermark);
        let Some(s) = self.spill.as_mut() else {
            return;
        };
        for id in &victims {
            s.disk.set(*id);
        }
        let n = victims.len() as u64;
        for id in victims {
            self.remove_ram(id);
        }
        diag::STATS.q_spilled.fetch_add(n, Ordering::Relaxed);
    }

    /// Baut die Sortierstruktur aus den gültigen Einträgen neu auf und zählt die startbaren neu.
    fn rebuild(&mut self) {
        let keys: Vec<Reverse<Key>> = self.items.values().map(|i| Reverse(Key::of(i))).collect();
        self.heap = BinaryHeap::from(keys);
        if let Some(s) = self.spill.as_mut() {
            s.poppable = if s.disk.len() > 0 {
                self.items
                    .values()
                    .filter(|i| Key::of(i) <= s.floor)
                    .count()
            } else {
                self.items.len()
            };
            s.pinned = self.items.values().filter(|i| !spillable_item(i)).count();
        }
    }

    // ---------------------------------------------------------------------------------------------------
    // Aufbau beim Fortsetzen (`resume.rs`)

    /// Merkt, dass die Kennung ein Ressourcen-Ziel ist (nur mit eingeschalteter Auslagerung).
    pub(crate) fn mark_resource(&mut self, id: i64) {
        if let Some(s) = self.spill.as_mut() {
            s.resource.set(id);
        }
    }

    /// Fortsetzen: `ids` stehen nur in der Datei, ihre Schlüssel liegen hinter `floor`.
    pub(crate) fn adopt_disk(&mut self, ids: IdSet, floor: Key) {
        if let Some(s) = self.spill.as_mut() {
            s.disk = ids;
            s.floor = floor;
        }
        self.rebuild();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn item(id: i64, depth: Option<u32>, seq: u64) -> QueueItem {
        QueueItem {
            id: UrlId(id),
            url: format!("https://site.test/{id}").into_boxed_str(),
            depth,
            priority: u8::from(depth.is_none()),
            seq,
            purpose: Purpose::Page,
            hops: 0,
            session: 0,
            replace: false,
        }
    }

    /// Die Tabelle `queue` der Datei, nachgebildet: Die Tests bestimmen, wann ein Push „geschrieben“ wird.
    #[derive(Default)]
    struct Table {
        rows: BTreeMap<Key, DiskRow>,
        by_id: HashMap<i64, Key>,
        pending: Vec<Op>,
    }

    enum Op {
        Push(QueueItem),
        Pop(i64),
    }

    impl Table {
        fn push(&mut self, item: &QueueItem) {
            self.pending.push(Op::Push(item.clone()));
        }
        fn pop(&mut self, id: i64) {
            self.pending.push(Op::Pop(id));
        }
        /// Der Writer arbeitet alles Gesendete ab (`flush`).
        fn flush(&mut self) {
            for op in std::mem::take(&mut self.pending) {
                match op {
                    Op::Push(i) => {
                        if let Some(k) = self.by_id.remove(&i.id.0) {
                            self.rows.remove(&k);
                        }
                        let row = DiskRow {
                            id: i.id.0,
                            seq: i.seq,
                            depth: i.depth,
                            priority: i.priority,
                            url: i.url.to_string(),
                            internal: i.purpose != Purpose::External,
                        };
                        let k = Key::of_row(&row);
                        self.by_id.insert(i.id.0, k);
                        self.rows.insert(k, row);
                    }
                    Op::Pop(id) => {
                        if let Some(k) = self.by_id.remove(&id) {
                            self.rows.remove(&k);
                        }
                    }
                }
            }
        }
        fn block(&self, after: Key, limit: usize) -> Vec<DiskRow> {
            use std::ops::Bound::{Excluded, Unbounded};
            self.rows
                .range((Excluded(after), Unbounded))
                .take(limit)
                .map(|(_, r)| r.clone())
                .collect()
        }
    }

    /// Ein Frontier samt nachgebildeter Datei und einem Zähler für `seq`.
    struct Rig {
        f: Frontier,
        table: Table,
        seq: u64,
    }

    impl Rig {
        fn new(window: usize) -> Self {
            let mut f = Frontier::default();
            f.enable_spill(window, PathBuf::from("nicht-benutzt"));
            Rig {
                f,
                table: Table::default(),
                seq: 1,
            }
        }

        fn push(&mut self, id: i64, depth: Option<u32>) {
            let it = item(id, depth, self.seq);
            self.seq += 1;
            self.table.push(&it);
            self.f.push(it);
        }

        fn improve(&mut self, id: i64, old_depth: Option<u32>, depth: u32) -> bool {
            let seq = self.seq;
            let hint = ImproveHint {
                old_depth: old_depth.unwrap_or(NO_DEPTH),
                url: Box::leak(format!("https://site.test/{id}").into_boxed_str()),
                internal: true,
            };
            match self.f.improve_with(UrlId(id), 0, Some(depth), seq, &hint) {
                Some(it) => {
                    self.seq += 1;
                    self.table.push(&it);
                    true
                }
                None => false,
            }
        }

        /// Führt fällige Aufträge aus; `lag` = Pushes, die der Writer beim `flush` noch nicht gesehen hat.
        fn run_jobs(&mut self) {
            while let Some(plan) = self.f.begin_job(self.seq, Instant::now()) {
                self.table.flush();
                let data = match plan.kind {
                    JobKind::Activate => JobData::Counted(self.table.rows.len() as u64),
                    JobKind::Refill => JobData::Block(self.table.block(plan.after, plan.limit)),
                };
                self.f.finish_job(data);
            }
        }

        fn pop(&mut self) -> Option<i64> {
            loop {
                self.run_jobs();
                match self.f.pop_checked() {
                    Popped::Item(i) => {
                        self.table.pop(i.id.0);
                        return Some(i.id.0);
                    }
                    Popped::Empty => return None,
                    Popped::NeedRefill => {
                        assert!(
                            self.f.begin_job(self.seq, Instant::now()).is_some()
                                || self.f.spill.as_ref().unwrap().job.is_some(),
                            "NeedRefill ohne fälligen Auftrag"
                        );
                        // begin_job oben hat den Auftrag schon markiert: ausführen
                        self.table.flush();
                        let s = self.f.spill.as_ref().unwrap();
                        let (after, limit) = (s.floor, s.window);
                        let block = self.table.block(after, limit);
                        self.f.finish_job(JobData::Block(block));
                    }
                }
            }
        }
    }

    // spec: CRAWL-005
    #[test]
    fn a_small_window_pops_in_the_same_order_as_an_unbounded_queue() {
        let mut rig = Rig::new(8);
        let mut plain = Frontier::default();
        let mut seq = 1u64;
        // BFS-artig: Tiefe 1 mit 40 URLs, dann je Pop zwei neue Kinder der nächsten Tiefe
        let mut next_id = 1i64;
        for _ in 0..40 {
            let it = item(next_id, Some(1), seq);
            plain.push(it.clone());
            rig.push(next_id, Some(1));
            next_id += 1;
            seq += 1;
        }
        let mut popped_rig = Vec::new();
        let mut popped_plain = Vec::new();
        let mut spilled_seen = false;
        while let Some(p) = plain.pop() {
            let r = rig.pop().expect("der Rig hat auch noch etwas");
            popped_plain.push(p.id.0);
            popped_rig.push(r);
            spilled_seen |= rig.f.disk_len() > 0;
            let depth = p.depth.unwrap_or(0) + 1;
            if depth <= 4 {
                for _ in 0..2 {
                    let it = item(next_id, Some(depth), seq);
                    plain.push(it.clone());
                    rig.push(next_id, Some(depth));
                    next_id += 1;
                    seq += 1;
                }
            }
        }
        assert_eq!(rig.pop(), None);
        assert_eq!(popped_rig, popped_plain);
        assert!(spilled_seen, "es wurde ausgelagert");
        assert!(popped_plain.len() > 500);
        assert!(rig.f.is_empty());
    }

    // spec: CRAWL-005, PERF-003
    #[test]
    fn the_ram_part_stays_near_the_window_and_the_rest_waits_in_the_file() {
        let mut rig = Rig::new(100);
        for id in 1..=5_000 {
            rig.push(id, Some(1 + (id as u32 % 3)));
        }
        rig.run_jobs();
        assert!(rig.f.ram_len() <= 100 + 5, "RAM {}", rig.f.ram_len());
        assert_eq!(rig.f.len(), 5_000);
        assert_eq!(rig.f.disk_len(), rig.f.len() - rig.f.ram_len());
        // weitere Einträge hinter der Grenze gehen nur in die Datei
        for id in 5_001..=6_000 {
            rig.push(id, Some(9));
        }
        assert!(rig.f.ram_len() <= 105, "RAM {}", rig.f.ram_len());
        assert_eq!(rig.f.len(), 6_000);
        let mut count = 0;
        let mut last_depth = 0;
        while let Some(id) = rig.pop() {
            count += 1;
            let depth = if id <= 5_000 { 1 + (id as u32 % 3) } else { 9 };
            assert!(depth >= last_depth, "Tiefe {depth} nach {last_depth}");
            last_depth = depth;
        }
        assert_eq!(count, 6_000);
    }

    // spec: PERF-005
    #[test]
    fn entries_pushed_while_a_job_runs_are_neither_lost_nor_duplicated() {
        let mut rig = Rig::new(10);
        for id in 1..=100 {
            rig.push(id, Some(1));
        }
        // Auftrag beginnt, während der Writer noch nicht geschrieben hat; danach kommen neue Einträge hinzu
        let plan = rig.f.begin_job(rig.seq, Instant::now()).expect("Auslagern");
        assert_eq!(plan.kind, JobKind::Activate);
        for id in 101..=130 {
            rig.push(id, Some(1));
        }
        rig.table.flush();
        rig.f
            .finish_job(JobData::Counted(rig.table.rows.len() as u64));
        assert!(rig.f.disk_len() >= 80, "{}", rig.f.disk_len());
        // die 30 neuen Einträge sind im RAM geblieben
        for id in 101..=130 {
            assert!(rig.f.items.contains_key(&id), "{id}");
        }
        // Nachladen mit weiteren Pushes mitten im Auftrag
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        while let Some(id) = rig.pop() {
            assert!(seen.insert(id), "{id} doppelt");
            order.push(id);
            if id == 20 {
                // läuft vermutlich gerade kein Auftrag; ein Push mit großer Tiefe geht in die Datei
                rig.push(1_000, Some(7));
            }
        }
        assert_eq!(seen.len(), 131);
        assert_eq!(order.last(), Some(&1_000));
    }

    // spec: CRAWL-005
    #[test]
    fn a_shorter_path_moves_a_spilled_entry_forward() {
        let mut rig = Rig::new(10);
        for id in 1..=100 {
            rig.push(id, Some(3));
        }
        rig.run_jobs();
        assert!(rig.f.disk_len() > 50);
        // 90 steht in der Datei; über einen kürzeren Weg hat es Tiefe 1: vor allen anderen
        assert!(!rig.f.items.contains_key(&90));
        assert!(rig.improve(90, Some(3), 1));
        assert_eq!(rig.pop(), Some(90));
        // ein Fund mit gleicher oder größerer Tiefe ändert nichts
        assert!(!rig.improve(95, Some(3), 3));
        assert!(!rig.improve(95, Some(3), 5));
        // 99 bekommt Tiefe 2: bleibt hinter dem Fenster in der Datei, kommt aber vor den Tiefe-3-Einträgen
        assert!(rig.improve(99, Some(3), 2));
        assert_eq!(rig.pop(), Some(99));
        let mut rest = Vec::new();
        while let Some(id) = rig.pop() {
            rest.push(id);
        }
        assert_eq!(rest.len(), 98);
        assert!(!rest.contains(&90) && !rest.contains(&99));
    }

    // spec: CRAWL-005
    #[test]
    fn an_improvement_during_a_job_is_not_overwritten_by_the_stale_block() {
        let mut rig = Rig::new(10);
        for id in 1..=100 {
            rig.push(id, Some(3));
        }
        rig.run_jobs();
        // Nachladen beginnt (Block würde 60 mit Tiefe 3 enthalten) ...
        while rig.f.spill.as_ref().unwrap().poppable > 5 {
            let id = rig.pop().unwrap();
            assert!(id <= 100);
        }
        let plan = rig.f.begin_job(rig.seq, Instant::now()).expect("Nachladen");
        assert_eq!(plan.kind, JobKind::Refill);
        // ... der Block wird gelesen ...
        rig.table.flush();
        let stale = rig.table.block(plan.after, plan.limit);
        // ... und währenddessen findet ein anderer Worker für 100 einen kürzeren Weg
        let in_block = stale.iter().any(|r| r.id == 100);
        assert!(rig.improve(100, Some(3), 1));
        rig.f.finish_job(JobData::Block(stale));
        assert!(in_block || rig.f.disk_len() > 0);
        // 100 wird als Nächstes gestartet und nur einmal
        assert_eq!(rig.pop(), Some(100));
        let mut seen = HashSet::new();
        while let Some(id) = rig.pop() {
            assert!(seen.insert(id));
            assert_ne!(id, 100);
        }
    }

    // spec: PERF-005
    #[test]
    fn a_sink_that_does_not_write_the_queue_disables_spilling_and_loses_nothing() {
        let mut rig = Rig::new(10);
        for id in 1..=100 {
            rig.push(id, Some(1));
        }
        let _plan = rig.f.begin_job(rig.seq, Instant::now()).unwrap();
        // die Datei kennt keine einzige Zeile
        rig.f.finish_job(JobData::Counted(0));
        assert_eq!(rig.f.disk_len(), 0);
        assert_eq!(rig.f.ram_len(), 100);
        assert!(
            rig.f.begin_job(rig.seq, Instant::now()).is_none(),
            "abgeschaltet"
        );
        let mut n = 0;
        while rig.f.pop().is_some() {
            n += 1;
        }
        assert_eq!(n, 100);
    }

    // spec: CRAWL-005
    #[test]
    fn non_spillable_entries_stay_in_ram_and_keep_their_order() {
        let mut rig = Rig::new(10);
        for id in 1..=60 {
            rig.push(id, Some(2));
        }
        // ein Weiterleitungsziel mit Tiefe 5 (hops 1) bleibt immer im RAM
        let mut redirect = item(500, Some(5), rig.seq);
        redirect.hops = 1;
        rig.seq += 1;
        rig.table.push(&redirect);
        rig.f.push(redirect);
        for id in 61..=120 {
            rig.push(id, Some(2));
        }
        rig.run_jobs();
        assert!(rig.f.items.contains_key(&500));
        let (_, _, _, _, disk, pinned) = rig.f.memory();
        assert!(disk > 80 && pinned == 1, "{disk} {pinned}");
        let mut out = Vec::new();
        while let Some(id) = rig.pop() {
            out.push(id);
        }
        assert_eq!(out.len(), 121);
        assert_eq!(out.last(), Some(&500), "Tiefe 5 zuletzt");
    }

    // spec: CRAWL-005
    #[test]
    fn sitemap_only_entries_come_last_and_survive_spilling() {
        let mut rig = Rig::new(10);
        for id in 1..=50 {
            rig.push(id, None);
        }
        for id in 51..=100 {
            rig.push(id, Some(4));
        }
        rig.run_jobs();
        let mut out = Vec::new();
        while let Some(id) = rig.pop() {
            out.push(id);
        }
        assert_eq!(out.len(), 100);
        let first_sitemap = out.iter().position(|id| *id <= 50).unwrap();
        assert_eq!(
            first_sitemap, 50,
            "erst alle Link-URLs, dann die Sitemap-URLs"
        );
        assert!(
            out[50..].windows(2).all(|w| w[0] < w[1]),
            "Sitemap-URLs in Reihenfolge"
        );
    }

    // spec: PERF-001
    #[test]
    fn a_random_workload_matches_the_unbounded_queue_for_any_window() {
        // Zufällige Folgen aus Einplanen, Entnehmen und kürzeren Wegen; die Reihenfolge der Entnahmen muss für jede
        // Fenstergröße dieselbe sein wie ohne Auslagerung.
        for (window, seed) in [(2usize, 1u64), (5, 2), (16, 3), (50, 4)] {
            let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut rnd = move |n: u64| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state % n
            };
            let mut rig = Rig::new(window);
            let mut plain = Frontier::default();
            let mut depth_of: HashMap<i64, u32> = HashMap::new();
            let mut queued: Vec<i64> = Vec::new();
            let mut seq = 1u64;
            let mut next_id = 1i64;
            let mut a = Vec::new();
            let mut b = Vec::new();
            for _ in 0..4_000 {
                match rnd(10) {
                    0..=4 => {
                        let depth = 1 + rnd(6) as u32;
                        let it = item(next_id, Some(depth), seq);
                        plain.push(it.clone());
                        rig.push(next_id, Some(depth));
                        depth_of.insert(next_id, depth);
                        queued.push(next_id);
                        next_id += 1;
                        seq += 1;
                    }
                    5..=6 if !queued.is_empty() => {
                        let id = queued[rnd(queued.len() as u64) as usize];
                        let old = depth_of[&id];
                        let new = 1 + rnd(6) as u32;
                        let got = plain.improve(UrlId(id), 0, Some(new), seq);
                        let rig_got = rig.improve(id, Some(old), new);
                        assert_eq!(got.is_some(), rig_got, "improve {id} {old}->{new}");
                        if got.is_some() {
                            depth_of.insert(id, new);
                            seq += 1;
                        }
                    }
                    _ => {
                        let p = plain.pop().map(|i| i.id.0);
                        let r = rig.pop();
                        assert_eq!(p, r, "Fenster {window}");
                        if let Some(id) = p {
                            queued.retain(|q| *q != id);
                            a.push(id);
                            b.push(id);
                        }
                    }
                }
                assert_eq!(plain.len(), rig.f.len(), "Fenster {window}");
            }
            assert_eq!(a, b);
            loop {
                let p = plain.pop().map(|i| i.id.0);
                let r = rig.pop();
                assert_eq!(p, r);
                if p.is_none() {
                    break;
                }
            }
            assert!(rig.f.is_empty());
        }
    }
}
