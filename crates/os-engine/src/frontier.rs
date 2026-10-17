//! Frontier und Kernzustand der Engine: Seen-Set, Warteschlange (BFS), Kennungen, Limit-Reservierung.
//!
//! * **Reihenfolge:** `(Priorität, Tiefe, seq)` aufsteigend. Priorität 0 = über Links erreichbar, 1 = nur über
//!   eine Sitemap (ohne Tiefe, hinter allen Link-URLs, CRAWL-005). Es wird immer der kleinste Schlüssel
//!   gestartet, also beginnt keine URL der Tiefe n+1, solange eine bekannte URL der Tiefe n noch wartet.
//! * **Warteschlange:** `queue::Frontier`. Sie liegt im RAM, solange sie klein ist; wächst sie über das Doppelte des
//!   Fensters (`queue::QUEUE_WINDOW`), bleiben im RAM nur die Einträge mit den kleinsten Schlüsseln, der Rest steht
//!   nur in der Tabelle `queue` der Crawl-Datei und wird nachgeladen (K-78 b, Einzelheiten dort).
//! * **Seen-Set:** kompakte Menge im RAM (`seen::SeenSet`, K-78 a: 64-Bit-Schlüssel plus unabhängiger 32-Bit-
//!   Fingerabdruck, 16 Byte je Eintrag, echte Kollisionen in einer Überlaufmenge mit voller Adresse), SQLite-
//!   `UNIQUE` ist die Wahrheit in der Datei. Die Entdeckung ist atomar: Nachschlagen, Kennung vergeben und
//!   Einplanen geschehen unter derselben Sperre, damit eine URL auch bei 200 Threads nie zweimal eingeplant wird.
//! * **Mindesttiefe:** Wird eine bekannte URL über einen kürzeren Weg erneut gefunden, sinkt ihre Tiefe
//!   (`UrlUpdate::Depth`); wartet sie noch, wird sie neu einsortiert. Bereits abgerufene Seiten geben die
//!   neue Tiefe nicht an ihre Kinder weiter.
//! * **Limit:** `max_urls` wird beim Start eines Abrufs reserviert (`try_pop`), die Zahl der gestarteten
//!   Abrufe überschreitet das Limit nie. Abgerufene robots.txt-Dateien zählen nicht (sie reservieren nichts).

use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::Arc;

use os_fetch::{CookieJar, FetchPurpose};
use os_model::{
    DiscoveryKind, NewUrl, NotCrawledReason, QueueOp, UrlId, UrlState, UrlUpdate, WriteBatch,
};
use os_url::{NormUrl, RuleDecision, RuleSet, Scope, ScopeClass};

pub use crate::queue::{Frontier, ImproveHint, Popped};
use crate::redirect::RedirectGraph;
pub use crate::seen::url_key;
use crate::seen::SeenSet;

/// Tiefe „unbekannt“ im Seen-Set und im Schlüssel (sortiert hinter jeder echten Tiefe).
pub const NO_DEPTH: u32 = u32::MAX;

/// Ordnet URLs dem Crawl-Bereich zu (intern/extern). `Scope` und [`CrawlScope`] implementieren es.
pub trait Classify {
    /// Intern oder extern?
    fn classify(&self, url: &NormUrl) -> ScopeClass;
}

impl Classify for Scope {
    fn classify(&self, url: &NormUrl) -> ScopeClass {
        Scope::classify(self, url)
    }
}

/// Crawl-Bereich der Engine: der `Scope` aus Start-URL und Konfiguration, im Listenmodus zusätzlich alle
/// Hosts der Liste (CRAWL-028: jeder Host der Liste ist im Bereich).
#[derive(Debug, Clone)]
pub struct CrawlScope {
    base: Scope,
    extra_hosts: std::collections::HashSet<String>,
}

impl CrawlScope {
    /// Bereich ohne weitere Hosts (Spider-Modus).
    pub fn new(base: Scope) -> Self {
        CrawlScope {
            base,
            extra_hosts: std::collections::HashSet::new(),
        }
    }

    /// Bereich mit zusätzlichen internen Hosts (Kleinbuchstaben, ohne Port).
    pub fn with_hosts(base: Scope, hosts: impl IntoIterator<Item = String>) -> Self {
        CrawlScope {
            base,
            extra_hosts: hosts.into_iter().map(|h| h.to_ascii_lowercase()).collect(),
        }
    }

    /// Fügt einen internen Host hinzu.
    pub fn add_host(&mut self, host: &str) {
        self.extra_hosts.insert(host.to_ascii_lowercase());
    }

    /// Ordnet eine URL ein.
    pub fn classify(&self, url: &NormUrl) -> ScopeClass {
        if let Some(host) = url.url.host_str() {
            if !self.extra_hosts.is_empty()
                && self
                    .extra_hosts
                    .contains(host.trim_end_matches('.').to_ascii_lowercase().as_str())
            {
                return ScopeClass::Internal;
            }
        }
        self.base.classify(url)
    }
}

impl Classify for CrawlScope {
    fn classify(&self, url: &NormUrl) -> ScopeClass {
        CrawlScope::classify(self, url)
    }
}

/// Wofür eine URL abgerufen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Seite oder Datei im Bereich.
    Page,
    /// Ressource einer Seite.
    Resource,
    /// Externe URL (nur Status, nie geparst).
    External,
}

impl Purpose {
    /// Zweck für den Abruf.
    pub fn fetch_purpose(self) -> FetchPurpose {
        match self {
            Purpose::Page => FetchPurpose::Page,
            Purpose::Resource => FetchPurpose::Resource,
            Purpose::External => FetchPurpose::External,
        }
    }
}

/// Eine wartende URL.
#[derive(Debug, Clone)]
pub struct QueueItem {
    pub attempt: u32,
    /// Kennung.
    pub id: UrlId,
    /// URL Encoded Address.
    pub url: Box<str>,
    /// Crawl-Tiefe (`None` = nur über Sitemap erreicht).
    pub depth: Option<u32>,
    /// 0 = Link, 1 = nur Sitemap.
    pub priority: u8,
    /// Reihenfolge innerhalb der Tiefe.
    pub seq: u64,
    /// Zweck.
    pub purpose: Purpose,
    /// Zahl der Weiterleitungen, die bis hierher gefolgt wurden (CRAWL-072).
    pub hops: u32,
    /// Cookie-Sitzung (Index in `Core::sessions`).
    pub session: u32,
    /// Die Daten der URL ersetzen (`PageWrite.replace`): „Neu crawlen“ (CRAWL-131).
    pub replace: bool,
}

/// Hasher für Schlüssel, die schon Hashwerte sind.
#[derive(Default)]
pub struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 << 8) | u64::from(*b);
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}

/// Die URL steht in der Warteschlange oder ist abgerufen.
pub const F_QUEUED: u8 = 1;
/// Die URL erscheint in den Tabellen.
pub const F_IN_TABLES: u8 = 2;
/// Include/Exclude haben die URL ausgeschlossen.
pub const F_RULE_BLOCKED: u8 = 4;
/// Include/Exclude lassen die URL zu (bereits geprüft).
pub const F_RULE_OK: u8 = 8;

/// Eintrag im Seen-Set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    /// Kennung.
    pub id: UrlId,
    /// Kleinste bekannte Tiefe.
    pub depth: u32,
    /// Bitmaske `F_*`.
    pub flags: u8,
}

/// Was die Entdeckung einer URL wissen muss (vom Aufrufer ohne Sperre vorbereitet).
pub struct Cand<'a> {
    pub source: Option<(UrlId, os_model::LinkType, Option<String>)>,
    /// Normalisierte Ziel-URL.
    pub norm: &'a NormUrl,
    /// Im Crawl-Bereich.
    pub internal: bool,
    /// Wie entdeckt.
    pub kind: DiscoveryKind,
    /// Ziel speichern (erscheint in den Tabellen).
    pub store: bool,
    /// Ziel abrufen.
    pub crawl: bool,
    /// Grund, falls nicht abgerufen.
    pub reason: Option<NotCrawledReason>,
    /// Ressource (darf die Tiefengrenze um eins überschreiten).
    pub resource_like: bool,
    /// Include/Exclude anwenden (nicht für die Start-URL).
    pub apply_rules: bool,
    /// Zweck des Abrufs.
    pub purpose: Purpose,
}

/// Ergebnis der Zulassung einer URL.
struct Admission {
    enqueue: bool,
    in_tables: bool,
    reason: Option<NotCrawledReason>,
    rule_flag: u8,
}

/// Zähler, die der Aufrufer in `WriteBatch.counters` übernimmt.
pub const COUNTER_EXCLUDED: &str = "excluded_skipped";
/// Zähler der durch Include nicht erfassten URLs.
pub const COUNTER_NOT_INCLUDED: &str = "not_included_skipped";
/// Zähler der durch das Tiefenlimit übersprungenen URLs.
pub const COUNTER_DEPTH_LIMIT: &str = "skipped_depth_limit";
/// Zähler der durch das Weiterleitungslimit nicht verfolgten Ziele.
pub const COUNTER_REDIRECT_LIMIT: &str = "skipped_redirect_limit";

/// Ergebnis von `try_pop`.
pub enum Pop {
    /// Nächste URL; Abruf reserviert, `active` erhöht.
    Item(QueueItem),
    /// Nichts wartet.
    Empty,
    /// Es wartet etwas, aber das URL-Limit ist erreicht.
    LimitBlocked,
    /// Der nächste Eintrag steht nur in der Crawl-Datei: Die Steuerschleife wartet auf das Nachladen (K-78 b).
    NeedRefill,
}

/// Gemeinsamer Zustand aller Worker (hinter einer Sperre).
pub struct Core {
    pub deferred: Vec<(std::time::Duration, QueueItem)>,
    /// Seen-Set.
    pub seen: SeenSet,
    /// Warteschlange.
    pub frontier: Frontier,
    /// Nächste freie URL-Kennung.
    pub next_url_id: i64,
    /// Nächste freie Reihenfolgenummer.
    pub next_seq: u64,
    /// Nächste freie Sitemap-Kennung.
    pub next_sitemap_id: i64,
    /// Gestartete (reservierte) Abrufe.
    pub started: u64,
    /// Limit (`u64::MAX` = unbegrenzt).
    pub max_urls: u64,
    /// Laufende Worker.
    pub active: usize,
    /// Laufende Klasse-0-Worker, die vor Sitemap-URLs Vorrang haben (CRAWL-059).
    pub active_pages: usize,
    /// Cookie-Sitzungen (Index 0 = Sitzung des Spiders).
    pub sessions: Vec<Option<Arc<CookieJar>>>,
    /// Weiterleitungsgraph und Flags.
    pub redirects: RedirectGraph,
    /// Tiefengrenze (`None` = unbegrenzt).
    pub max_depth: Option<u32>,
    /// Schlüssel (URL Encoded Address) der Sitemap-Dateien, die in diesem Crawl schon abgerufen werden oder
    /// wurden – angefragte und End-URL (K-41a: genau ein Abruf je Sitemap-Datei, nur über `sitemaps::ingest`).
    pub sitemap_keys: std::collections::HashSet<String>,
    /// Nächste Reihenfolgenummer je URL für hreflang-Angaben aus Sitemaps (fortlaufend über alle Sitemaps).
    pub sitemap_hreflang_next: HashMap<UrlId, u32>,
    /// Listenmodus (CRAWL-028): Das Tiefenlimit gilt streng, auch für Ressourcen und externe Prüfungen der
    /// letzten Ebene, und die Einträge einer Sitemap sind Listen-URLs der Tiefe 0.
    pub list_mode: bool,
}

impl Core {
    /// Rückstellung bis zu einem absoluten monotonen Zeitpunkt; keine neue URL.
    pub fn defer(&mut self, item: QueueItem, due: std::time::Duration) {
        self.refund();
        self.deferred.push((due, item));
    }
    pub fn requeue(&mut self, item: QueueItem) {
        self.refund();
        self.frontier.push(item);
    }
    pub fn wake_deferred(&mut self, now: std::time::Duration) {
        let waiting = std::mem::take(&mut self.deferred);
        for (due, item) in waiting {
            if due <= now {
                self.frontier.push(item);
            } else {
                self.deferred.push((due, item));
            }
        }
    }
    pub fn next_due(&self) -> Option<std::time::Duration> {
        self.deferred.iter().map(|(due, _)| *due).min()
    }
    pub fn restore_deferred(&mut self) {
        for (_, mut item) in self.deferred.drain(..) {
            item.attempt = 0;
            self.frontier.push(item);
        }
    }
    fn record_discovery(
        &mut self,
        c: &Cand<'_>,
        target: UrlId,
        depth: Option<u32>,
        batch: &mut WriteBatch,
    ) {
        batch.discoveries.push(os_model::DiscoveryWrite {
            seq: self.alloc_seq(),
            target,
            source: c.source.as_ref().map(|s| s.0),
            kind: c.kind,
            link_type: c.source.as_ref().map(|s| s.1),
            anchor: c.source.as_ref().and_then(|s| s.2.clone()),
            depth,
        });
    }

    /// Leerer Zustand.
    pub fn new(max_urls: u64, max_depth: Option<u32>) -> Self {
        Core {
            deferred: Vec::new(),
            seen: SeenSet::default(),
            frontier: Frontier::default(),
            next_url_id: 1,
            next_seq: 1,
            next_sitemap_id: 1,
            started: 0,
            max_urls: if max_urls == 0 { u64::MAX } else { max_urls },
            active: 0,
            active_pages: 0,
            sessions: vec![None],
            redirects: RedirectGraph::default(),
            max_depth,
            sitemap_keys: std::collections::HashSet::new(),
            sitemap_hreflang_next: HashMap::new(),
            list_mode: false,
        }
    }

    /// Vergibt eine Kennung.
    pub fn alloc_id(&mut self) -> UrlId {
        let id = UrlId(self.next_url_id);
        self.next_url_id += 1;
        id
    }

    /// Vergibt eine Reihenfolgenummer.
    pub fn alloc_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    /// Schlägt eine URL nach.
    pub fn lookup(&self, encoded: &str) -> Option<Seen> {
        self.seen.get(encoded)
    }

    /// Nimmt die nächste URL und reserviert den Abruf.
    pub fn try_pop(&mut self) -> Pop {
        if self.frontier.is_empty() {
            return Pop::Empty;
        }
        if self.started >= self.max_urls {
            return Pop::LimitBlocked;
        }
        match self.frontier.pop_checked_while(self.active_pages != 0) {
            Popped::Item(item) => {
                self.started += 1;
                self.active += 1;
                self.active_pages += usize::from(item.priority == 0);
                Pop::Item(item)
            }
            Popped::Empty => Pop::Empty,
            Popped::NeedRefill => Pop::NeedRefill,
        }
    }

    /// Gibt eine Reservierung zurück (der Abruf fand nicht statt, z. B. durch robots.txt gesperrt).
    pub fn refund(&mut self) {
        self.started = self.started.saturating_sub(1);
    }

    /// Ein Worker ist fertig.
    pub fn finish_worker(&mut self, page: bool) {
        self.active = self.active.saturating_sub(1);
        self.active_pages = self.active_pages.saturating_sub(usize::from(page));
    }

    /// Legt eine neue Cookie-Sitzung an.
    pub fn new_session(&mut self, jar: Option<Arc<CookieJar>>) -> u32 {
        self.sessions.push(jar);
        (self.sessions.len() - 1) as u32
    }

    /// Cookie-Speicher einer Sitzung.
    pub fn session_jar(&self, session: u32) -> Option<Arc<CookieJar>> {
        self.sessions.get(session as usize).cloned().flatten()
    }

    /// Nächster fälliger Hintergrundauftrag für das RAM-Fenster der Warteschlange (K-78 b).
    pub(crate) fn next_queue_job(&mut self) -> Option<crate::queue::JobPlan> {
        self.frontier
            .begin_job(self.next_seq, std::time::Instant::now())
    }

    /// Anzahl bekannter URLs.
    pub fn discovered(&self) -> u64 {
        self.seen.len() as u64
    }

    /// Geschätzte Größe der Kernstrukturen für die Speicherzähler (`diag`, M1-P33).
    pub fn memory(&self) -> crate::diag::CoreMemory {
        let (queue_items, queue_bytes, heap_entries, heap_bytes, queue_disk, queue_pinned) =
            self.frontier.memory();
        let (redirect_entries, redirect_bytes) = self.redirects.memory();
        let (seen_entries, seen_bytes, seen_overflow) = self.seen.memory();
        let sitemap_key_bytes: usize = self
            .sitemap_keys
            .iter()
            .map(|k| k.len().div_ceil(16) * 16 + 24)
            .sum();
        let sitemap_bytes = sitemap_key_bytes
            + self.sitemap_keys.capacity() * 8
            + self.sitemap_hreflang_next.capacity() * (std::mem::size_of::<(UrlId, u32)>() + 1);
        crate::diag::CoreMemory {
            seen_entries,
            seen_bytes,
            seen_overflow,
            queue_items,
            queue_bytes,
            queue_disk,
            queue_pinned,
            heap_entries,
            heap_bytes,
            redirect_entries,
            redirect_bytes,
            sitemap_entries: (self.sitemap_keys.len() + self.sitemap_hreflang_next.len()) as u64,
            sitemap_bytes: sitemap_bytes as u64,
        }
    }

    fn admit(
        &mut self,
        c: &Cand<'_>,
        depth: Option<u32>,
        known_flags: u8,
        rules: &RuleSet,
    ) -> Admission {
        // 1. Include/Exclude (nur einmal je URL ausgewertet)
        let rule_flag;
        let mut blocked: Option<NotCrawledReason> = None;
        if !c.apply_rules || known_flags & F_RULE_OK != 0 {
            rule_flag = F_RULE_OK;
        } else {
            match rules.decide(&c.norm.encoded) {
                RuleDecision::Allowed => rule_flag = F_RULE_OK,
                RuleDecision::Excluded(_) => {
                    rule_flag = F_RULE_BLOCKED;
                    blocked = Some(NotCrawledReason::Excluded);
                }
                RuleDecision::NotIncluded => {
                    rule_flag = F_RULE_BLOCKED;
                    blocked = Some(NotCrawledReason::NotIncluded);
                }
            }
        }
        if let Some(reason) = blocked {
            return Admission {
                enqueue: false,
                in_tables: false,
                reason: Some(reason),
                rule_flag,
            };
        }
        // 2. Entscheidung der Speichern/Crawlen-Matrix
        if !c.crawl {
            return Admission {
                enqueue: false,
                in_tables: c.store,
                reason: c.reason.or(Some(NotCrawledReason::CrawlDisabled)),
                rule_flag,
            };
        }
        // 3. Tiefengrenze (Ressourcen und externe Prüfungen der letzten Ebene bleiben erlaubt)
        if let (Some(max), Some(d)) = (self.max_depth, depth) {
            // Im Listenmodus gibt es keine Zugabe für die letzte Ebene (CRAWL-028: Ressourcen und externe
            // Ziele der Listenseiten werden bei Tiefe 0 nicht angefragt).
            let allowance =
                u32::from(!self.list_mode && (c.resource_like || c.purpose == Purpose::External));
            if d > max.saturating_add(allowance) {
                return Admission {
                    enqueue: false,
                    in_tables: false,
                    reason: Some(if self.list_mode {
                        NotCrawledReason::ListModeDepth
                    } else {
                        NotCrawledReason::DepthLimit
                    }),
                    rule_flag,
                };
            }
        }
        Admission {
            enqueue: true,
            in_tables: c.store,
            reason: None,
            rule_flag,
        }
    }

    /// Entdeckt eine URL: vergibt eine Kennung und plant sie ein, oder aktualisiert eine bekannte URL
    /// (kleinere Tiefe, jetzt abrufbar, jetzt in den Tabellen). Schreibt alles in `batch`.
    ///
    /// `depth` ist die Tiefe, die die URL über diesen Weg hat; `hops` und `session` gelten für einen neuen
    /// Eintrag in der Warteschlange.
    #[allow(clippy::too_many_arguments)]
    pub fn discover(
        &mut self,
        c: &Cand<'_>,
        depth: Option<u32>,
        hops: u32,
        session: u32,
        rules: &RuleSet,
        batch: &mut WriteBatch,
    ) -> UrlId {
        let kf = self.seen.key_fp(&c.norm.encoded);
        // Ohne Tiefe (nur über eine Sitemap erreichbar) gilt Priorität 1: hinter allen Link-URLs.
        let priority = u8::from(depth.is_none());
        if let Some(known) = self.seen.get_hashed(kf, &c.norm.encoded) {
            let mut e = known;
            let id = e.id;
            // kürzerer Weg → Mindesttiefe
            if let Some(d) = depth {
                if d < e.depth {
                    self.record_discovery(c, id, Some(d), batch);
                    e.depth = d;
                    batch.url_updates.push(UrlUpdate::Depth { id, depth: d });
                }
            }
            if e.flags & F_QUEUED != 0 {
                // Priorität/Tiefe der wartenden URL verbessern (z. B. Sitemap-URL, jetzt auch verlinkt)
                if depth.is_some() {
                    let seq = self.next_seq;
                    let hint = ImproveHint {
                        old_depth: known.depth,
                        url: &c.norm.encoded,
                        internal: c.internal,
                    };
                    if let Some(item) = self.frontier.improve_with(id, 0, depth, seq, &hint) {
                        self.next_seq += 1;
                        batch.queue.push(QueueOp::Push {
                            id,
                            seq: item.seq,
                            depth: item.depth,
                            priority: item.priority,
                        });
                    }
                }
                if c.store && e.flags & F_IN_TABLES == 0 && e.flags & F_RULE_BLOCKED == 0 {
                    e.flags |= F_IN_TABLES;
                    batch
                        .url_updates
                        .push(UrlUpdate::InTables { id, value: true });
                }
            } else if e.flags & F_RULE_BLOCKED == 0 {
                let adm = self.admit(
                    c,
                    depth.or(Some(e.depth).filter(|d| *d != NO_DEPTH)),
                    e.flags,
                    rules,
                );
                e.flags |= adm.rule_flag;
                if adm.enqueue {
                    let seq = self.alloc_seq();
                    let known_depth = Some(e.depth).filter(|d| *d != NO_DEPTH);
                    let prio = u8::from(known_depth.is_none());
                    self.frontier.push(QueueItem {
                        id,
                        url: c.norm.encoded.clone().into_boxed_str(),
                        depth: known_depth,
                        priority: prio,
                        seq,
                        purpose: c.purpose,
                        hops,
                        session,
                        replace: false,
                        attempt: 0,
                    });
                    batch.url_updates.push(UrlUpdate::State {
                        id,
                        state: UrlState::Queued,
                        reason: None,
                    });
                    batch.queue.push(QueueOp::Push {
                        id,
                        seq,
                        depth: known_depth,
                        priority: prio,
                    });
                    e.flags |= F_QUEUED;
                }
                if adm.in_tables && e.flags & F_IN_TABLES == 0 {
                    e.flags |= F_IN_TABLES;
                    batch
                        .url_updates
                        .push(UrlUpdate::InTables { id, value: true });
                }
            }
            if e != known {
                self.seen.insert_hashed(kf, &c.norm.encoded, e);
            }
            return id;
        }

        // neue URL
        let adm = self.admit(c, depth, 0, rules);
        let id = self.alloc_id();
        self.record_discovery(c, id, depth, batch);
        let mut flags = adm.rule_flag;
        if adm.in_tables {
            flags |= F_IN_TABLES;
        }
        match adm.reason {
            Some(NotCrawledReason::Excluded) => {
                batch.counters.push((COUNTER_EXCLUDED.to_string(), 1));
            }
            Some(NotCrawledReason::NotIncluded) => {
                batch.counters.push((COUNTER_NOT_INCLUDED.to_string(), 1));
            }
            Some(NotCrawledReason::DepthLimit | NotCrawledReason::ListModeDepth) => {
                batch.counters.push((COUNTER_DEPTH_LIMIT.to_string(), 1));
            }
            _ => {}
        }
        batch.new_urls.push(NewUrl {
            id,
            url: c.norm.encoded.clone(),
            address: c.norm.display.clone(),
            origin: c.norm.origin.clone(),
            is_internal: c.internal,
            in_tables: adm.in_tables,
            discovered_via: c.kind,
            crawl_depth: depth,
            folder_depth: os_url::folder_depth(&c.norm.url),
            url_length: os_url::url_length(&c.norm.display),
            state: if adm.enqueue {
                UrlState::Queued
            } else {
                UrlState::Discovered
            },
            not_crawled_reason: adm.reason,
        });
        if adm.enqueue {
            flags |= F_QUEUED;
            let seq = self.alloc_seq();
            self.frontier.push(QueueItem {
                id,
                url: c.norm.encoded.clone().into_boxed_str(),
                depth,
                priority,
                seq,
                purpose: c.purpose,
                hops,
                session,
                replace: false,
                attempt: 0,
            });
            batch.queue.push(QueueOp::Push {
                id,
                seq,
                depth,
                priority,
            });
        }
        self.seen.insert_hashed(
            kf,
            &c.norm.encoded,
            Seen {
                id,
                depth: depth.unwrap_or(NO_DEPTH),
                flags,
            },
        );
        id
    }

    /// Trägt eine URL ohne Prüfung ein (robots.txt-Zeile, Start-URL beim Fortsetzen).
    pub fn register(&mut self, encoded: &str, id: UrlId, depth: Option<u32>, flags: u8) {
        self.seen.insert(
            encoded,
            Seen {
                id,
                depth: depth.unwrap_or(NO_DEPTH),
                flags,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_url::parse_start_url;

    fn norm(s: &str) -> NormUrl {
        parse_start_url(s).unwrap()
    }

    fn cand<'a>(n: &'a NormUrl) -> Cand<'a> {
        Cand {
            norm: n,
            internal: true,
            kind: DiscoveryKind::Link,
            store: true,
            crawl: true,
            reason: None,
            resource_like: false,
            apply_rules: true,
            purpose: Purpose::Page,
            source: None,
        }
    }

    fn rules(include: &[&str], exclude: &[&str]) -> RuleSet {
        let to = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        RuleSet::compile(&to(include), &to(exclude), false).unwrap()
    }

    fn pop_ids(core: &mut Core) -> Vec<i64> {
        let mut out = Vec::new();
        while let Pop::Item(i) = core.try_pop() {
            out.push(i.id.0);
            core.finish_worker(i.priority == 0);
        }
        out
    }

    // spec: CRAWL-005
    #[test]
    fn queue_is_ordered_by_priority_then_depth_then_discovery() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let a = norm("https://site.test/a");
        let bb = norm("https://site.test/b");
        let c = norm("https://site.test/c");
        let d = norm("https://site.test/d");
        // Tiefe 2, Tiefe 1, Tiefe 1, nur Sitemap
        let id_a = core.discover(&cand(&a), Some(2), 0, 0, &r, &mut b);
        let id_b = core.discover(&cand(&bb), Some(1), 0, 0, &r, &mut b);
        let id_c = core.discover(&cand(&c), Some(1), 0, 0, &r, &mut b);
        let mut sm = cand(&d);
        sm.kind = DiscoveryKind::Sitemap;
        let id_d = core.discover(&sm, None, 0, 0, &r, &mut b);
        assert_eq!(
            pop_ids(&mut core),
            vec![id_b.0, id_c.0, id_a.0, id_d.0],
            "erst Tiefe 1 in Entdeckungsreihenfolge, dann Tiefe 2, zuletzt nur-Sitemap"
        );
    }

    // spec: CRAWL-005
    #[test]
    fn a_shorter_path_lowers_the_depth_and_reorders_the_queue() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let x = norm("https://site.test/x");
        let y = norm("https://site.test/y");
        let id_x = core.discover(&cand(&x), Some(3), 0, 0, &r, &mut b);
        let id_y = core.discover(&cand(&y), Some(2), 0, 0, &r, &mut b);
        b.url_updates.clear();
        b.queue.clear();
        // x wird zusätzlich direkt von der Startseite verlinkt: Tiefe 1
        let again = core.discover(&cand(&x), Some(1), 0, 0, &r, &mut b);
        assert_eq!(again, id_x, "dieselbe Kennung");
        assert_eq!(b.url_updates, vec![UrlUpdate::Depth { id: id_x, depth: 1 }]);
        assert_eq!(b.queue.len(), 1, "neu einsortiert: {:?}", b.queue);
        assert_eq!(core.seen.len(), 2);
        assert_eq!(pop_ids(&mut core), vec![id_x.0, id_y.0], "x vor y");
        // ein längerer Weg ändert nichts
        let mut b2 = WriteBatch::default();
        core.discover(&cand(&x), Some(5), 0, 0, &r, &mut b2);
        assert!(b2.url_updates.is_empty() && b2.queue.is_empty());
    }

    // spec: CRAWL-005
    #[test]
    fn a_sitemap_only_url_found_by_link_gets_depth_and_priority_zero() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let n = norm("https://site.test/nur-sitemap");
        let other = norm("https://site.test/link");
        let mut sm = cand(&n);
        sm.kind = DiscoveryKind::Sitemap;
        let id = core.discover(&sm, None, 0, 0, &r, &mut b);
        assert_eq!(b.new_urls[0].crawl_depth, None);
        core.discover(&cand(&other), Some(4), 0, 0, &r, &mut b);
        // jetzt verlinkt: Tiefe 5, Priorität 0 → vor anderen Sitemap-URLs, aber nach Tiefe-4-Links
        core.discover(&cand(&n), Some(5), 0, 0, &r, &mut b);
        let order = pop_ids(&mut core);
        assert_eq!(order.last(), Some(&id.0));
        assert!(b.url_updates.contains(&UrlUpdate::Depth { id, depth: 5 }));
    }

    // Messzähler (M1-P33), kein Spec-Tag: die Schätzung zählt Einträge der Seen-Menge und der Warteschlange.
    #[test]
    fn memory_estimate_counts_seen_and_queued_entries() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        for i in 0..10 {
            let n = norm(&format!("https://site.test/{i}"));
            core.discover(&cand(&n), Some(1), 0, 0, &r, &mut b);
        }
        let m = core.memory();
        assert_eq!(m.seen_entries, 10);
        assert_eq!(m.queue_items, 10);
        assert!(m.seen_bytes >= 10 * 24 && m.queue_bytes > m.queue_items * 48);
        assert_eq!(m.queue_disk, 0);
        assert!(matches!(core.try_pop(), Pop::Item(_)));
        assert_eq!(core.memory().queue_items, 9);
        assert_eq!(
            m.total_bytes(),
            m.seen_bytes + m.queue_bytes + m.heap_bytes + m.redirect_bytes + m.sitemap_bytes
        );
    }

    // spec: PERF-001
    #[test]
    fn urls_with_a_forced_hash_collision_are_both_discovered_and_fetched() {
        use crate::seen::{default_key_fp, KeyFp, SeenSet};
        // gleicher 64-Bit-Schlüssel für alle URLs unter /c/, die Fingerabdrücke bleiben verschieden
        fn colliding(encoded: &str) -> KeyFp {
            let (key, fp) = default_key_fp(encoded);
            if encoded.contains("/c/") {
                (7, fp)
            } else {
                (key, fp)
            }
        }
        let mut core = Core::new(0, None);
        core.seen = SeenSet::with_key_fn(colliding);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let a = norm("https://site.test/c/a");
        let bb = norm("https://site.test/c/b");
        let cc = norm("https://site.test/c/c");
        let id_a = core.discover(&cand(&a), Some(1), 0, 0, &r, &mut b);
        let id_b = core.discover(&cand(&bb), Some(1), 0, 0, &r, &mut b);
        let id_c = core.discover(&cand(&cc), Some(2), 0, 0, &r, &mut b);
        assert!(
            id_a != id_b && id_b != id_c && id_a != id_c,
            "drei Kennungen"
        );
        assert_eq!(b.new_urls.len(), 3, "alle drei URLs sind neu");
        assert_eq!(
            core.seen.overflow_len(),
            2,
            "zwei Adressen in der Überlaufmenge"
        );
        // erneut gefunden: dieselbe Kennung, keine zweite Einplanung
        let again_b = core.discover(&cand(&bb), Some(1), 0, 0, &r, &mut b);
        let again_c = core.discover(&cand(&cc), Some(2), 0, 0, &r, &mut b);
        assert_eq!((again_b, again_c), (id_b, id_c));
        assert_eq!(b.new_urls.len(), 3);
        assert_eq!(core.lookup(&bb.encoded).map(|s| s.id), Some(id_b));
        assert_eq!(pop_ids(&mut core), vec![id_a.0, id_b.0, id_c.0]);
    }

    // spec: CRAWL-003
    #[test]
    fn the_same_url_is_never_scheduled_twice() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let a = norm("https://site.test/a");
        let ids: Vec<_> = (0..50)
            .map(|_| core.discover(&cand(&a), Some(1), 0, 0, &r, &mut b))
            .collect();
        assert!(ids.windows(2).all(|w| w[0] == w[1]));
        assert_eq!(b.new_urls.len(), 1);
        assert_eq!(
            b.queue
                .iter()
                .filter(|q| matches!(q, QueueOp::Push { .. }))
                .count(),
            1
        );
        assert_eq!(pop_ids(&mut core).len(), 1);
    }

    // spec: CRAWL-066
    #[test]
    fn reservation_never_exceeds_the_limit_and_refunds_are_reusable() {
        let mut core = Core::new(3, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        for i in 0..10 {
            let n = norm(&format!("https://site.test/{i}"));
            core.discover(&cand(&n), Some(1), 0, 0, &r, &mut b);
        }
        let mut taken = 0;
        while let Pop::Item(_) = core.try_pop() {
            taken += 1;
        }
        assert_eq!(taken, 3);
        assert!(matches!(core.try_pop(), Pop::LimitBlocked));
        assert_eq!(core.frontier.len(), 7, "Rest bleibt in der Warteschlange");
        core.refund();
        assert!(matches!(core.try_pop(), Pop::Item(_)));
        assert!(matches!(core.try_pop(), Pop::LimitBlocked));
        // Limit 0 = unbegrenzt
        let mut core = Core::new(0, None);
        let mut b = WriteBatch::default();
        for i in 0..10 {
            let n = norm(&format!("https://site.test/{i}"));
            core.discover(&cand(&n), Some(1), 0, 0, &r, &mut b);
        }
        assert_eq!(pop_ids(&mut core).len(), 10);
    }

    // spec: CRAWL-067
    #[test]
    fn depth_limit_blocks_pages_but_not_resources_of_the_last_level() {
        let mut core = Core::new(0, Some(2));
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let page = norm("https://site.test/p3");
        let img = norm("https://site.test/i3.png");
        let img_in_css = norm("https://site.test/deep.png");
        let ext = norm("https://other.test/");
        let id_page = core.discover(&cand(&page), Some(3), 0, 0, &r, &mut b);
        let mut ic = cand(&img);
        ic.resource_like = true;
        ic.purpose = Purpose::Resource;
        core.discover(&ic, Some(3), 0, 0, &r, &mut b);
        let mut dc = cand(&img_in_css);
        dc.resource_like = true;
        dc.purpose = Purpose::Resource;
        core.discover(&dc, Some(4), 0, 0, &r, &mut b);
        let mut ec = cand(&ext);
        ec.internal = false;
        ec.purpose = Purpose::External;
        core.discover(&ec, Some(3), 0, 0, &r, &mut b);

        let page_row = b.new_urls.iter().find(|u| u.id == id_page).unwrap();
        assert_eq!(page_row.state, UrlState::Discovered);
        assert_eq!(
            page_row.not_crawled_reason,
            Some(NotCrawledReason::DepthLimit)
        );
        assert!(
            !page_row.in_tables,
            "Limit-URLs stehen nicht in den Tabellen"
        );
        let by_url = |u: &str| b.new_urls.iter().find(|n| n.url == u).unwrap();
        assert_eq!(by_url("https://site.test/i3.png").state, UrlState::Queued);
        assert_eq!(
            by_url("https://site.test/deep.png").not_crawled_reason,
            Some(NotCrawledReason::DepthLimit)
        );
        assert_eq!(by_url("https://other.test/").state, UrlState::Queued);
        assert!(b.counters.contains(&(COUNTER_DEPTH_LIMIT.to_string(), 1)));
    }

    // spec: CRAWL-067
    #[test]
    fn depth_zero_admits_no_hyperlink_target() {
        let mut core = Core::new(0, Some(0));
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let n = norm("https://site.test/1");
        core.discover(&cand(&n), Some(1), 0, 0, &r, &mut b);
        assert_eq!(pop_ids(&mut core).len(), 0);
        // sitemap-only (ohne Tiefe) unterliegt der Tiefengrenze nicht
        let s = norm("https://site.test/s");
        let mut sm = cand(&s);
        sm.kind = DiscoveryKind::Sitemap;
        core.discover(&sm, None, 0, 0, &r, &mut b);
        assert_eq!(pop_ids(&mut core).len(), 1);
    }

    // spec: CRAWL-109, CRAWL-110
    #[test]
    fn exclude_and_include_keep_a_row_outside_the_tables() {
        let mut core = Core::new(0, None);
        let r = rules(&[".*/blog/.*"], &[".*\\.pdf"]);
        let mut b = WriteBatch::default();
        let blog = norm("https://site.test/blog/a");
        let shop = norm("https://site.test/shop/x");
        let pdf = norm("https://site.test/blog/handbuch.pdf");
        core.discover(&cand(&blog), Some(1), 0, 0, &r, &mut b);
        core.discover(&cand(&shop), Some(1), 0, 0, &r, &mut b);
        core.discover(&cand(&pdf), Some(1), 0, 0, &r, &mut b);
        fn reason_of(b: &WriteBatch, u: &str) -> (UrlState, Option<NotCrawledReason>, bool) {
            b.new_urls
                .iter()
                .find(|n| n.url == u)
                .map(|n| (n.state, n.not_crawled_reason, n.in_tables))
                .unwrap()
        }
        assert_eq!(
            reason_of(&b, "https://site.test/blog/a"),
            (UrlState::Queued, None, true)
        );
        assert_eq!(
            reason_of(&b, "https://site.test/shop/x"),
            (
                UrlState::Discovered,
                Some(NotCrawledReason::NotIncluded),
                false
            )
        );
        assert_eq!(
            reason_of(&b, "https://site.test/blog/handbuch.pdf"),
            (
                UrlState::Discovered,
                Some(NotCrawledReason::Excluded),
                false
            )
        );
        assert!(b.counters.contains(&(COUNTER_EXCLUDED.to_string(), 1)));
        // Start-URL: Regeln gelten nicht
        let start = norm("https://site.test/");
        let mut sc = cand(&start);
        sc.apply_rules = false;
        core.discover(&sc, Some(0), 0, 0, &r, &mut b);
        assert_eq!(
            reason_of(&b, "https://site.test/"),
            (UrlState::Queued, None, true)
        );
        // ausgeschlossene URL ein zweites Mal gefunden: keine erneute Auswertung, kein neuer Zähler
        let before = b.counters.len();
        core.discover(&cand(&pdf), Some(1), 0, 0, &r, &mut b);
        assert_eq!(b.counters.len(), before);
    }

    // spec: CRAWL-038
    #[test]
    fn a_stored_only_url_is_queued_when_a_later_link_type_may_crawl_it() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let n = norm("https://site.test/doc");
        let mut first = cand(&n);
        first.crawl = false;
        first.reason = Some(NotCrawledReason::StoreOnly);
        let id = core.discover(&first, Some(1), 0, 0, &r, &mut b);
        assert_eq!(
            b.new_urls[0].not_crawled_reason,
            Some(NotCrawledReason::StoreOnly)
        );
        assert!(core.frontier.is_empty());
        core.discover(&cand(&n), Some(2), 0, 0, &r, &mut b);
        assert!(core.frontier.contains(id));
        assert!(b.url_updates.contains(&UrlUpdate::State {
            id,
            state: UrlState::Queued,
            reason: None
        }));
    }

    // spec: CRAWL-038
    #[test]
    fn hidden_target_becomes_visible_when_a_stored_link_finds_it() {
        let mut core = Core::new(0, None);
        let r = rules(&[], &[]);
        let mut b = WriteBatch::default();
        let n = norm("https://site.test/hidden");
        let mut hidden = cand(&n);
        hidden.store = false;
        let id = core.discover(&hidden, Some(1), 0, 0, &r, &mut b);
        assert!(!b.new_urls[0].in_tables);
        assert_eq!(b.new_urls[0].state, UrlState::Queued);
        core.discover(&cand(&n), Some(1), 0, 0, &r, &mut b);
        assert!(b
            .url_updates
            .contains(&UrlUpdate::InTables { id, value: true }));
    }
    // spec: CRAWL-145
    // Infrastruktur (M2-F05a) für CRAWL-145; Fachprüfung M2-E03.
    #[test]
    fn deferred_attempt_refunds_slot_and_restores_on_pause_without_new_discovery() {
        let mut core = Core::new(1, None);
        let n = norm("https://site.test/retry");
        let mut batch = WriteBatch::default();
        core.discover(&cand(&n), Some(0), 0, 0, &rules(&[], &[]), &mut batch);
        let Pop::Item(mut item) = core.try_pop() else {
            panic!("item")
        };
        item.attempt = 1;
        core.defer(item, std::time::Duration::from_secs(2));
        core.finish_worker(true); // ActiveGuard releases the worker on return from run_item.
        assert_eq!(core.active, 0);
        assert_eq!(core.started, 0);
        assert_eq!(core.discovered(), 1);
        core.wake_deferred(std::time::Duration::from_secs(1));
        assert!(matches!(core.try_pop(), Pop::Empty));
        core.restore_deferred();
        let Pop::Item(item) = core.try_pop() else {
            panic!("restored")
        };
        assert_eq!(item.attempt, 0);
        assert_eq!(batch.discoveries.len(), 1);
    }

    // spec: CRAWL-059
    // Infrastruktur (M2-F05a) für CRAWL-059; Fachprüfung M2-E02.
    #[test]
    fn sitemap_waits_for_active_page_and_discovery_records_shorter_paths() {
        let mut core = Core::new(0, None);
        let first = norm("https://site.test/page");
        let sitemap = norm("https://site.test/sitemap-page");
        let rules = rules(&[], &[]);
        let mut batch = WriteBatch::default();
        let source = core.discover(&cand(&first), Some(0), 0, 0, &rules, &mut batch);
        let mut target = cand(&sitemap);
        target.source = Some((source, os_model::LinkType::Hyperlink, Some("anchor".into())));
        core.discover(&target, None, 0, 0, &rules, &mut batch);
        assert!(matches!(core.try_pop(), Pop::Item(_)));
        assert!(matches!(core.try_pop(), Pop::Empty));
        core.finish_worker(true);
        assert!(matches!(core.try_pop(), Pop::Item(_)));
        core.discover(&target, Some(2), 1, 0, &rules, &mut batch);
        assert_eq!(batch.discoveries.len(), 3);
        assert_eq!(batch.discoveries.last().unwrap().depth, Some(2));
        assert!(batch.discoveries.windows(2).all(|w| w[0].seq < w[1].seq));
    }
    // spec: CRAWL-059
    #[test]
    fn sitemap_workers_run_in_parallel_and_waiting_does_not_grow_the_heap() {
        let mut core = Core::new(0, None);
        let rules = rules(&[], &[]);
        let mut batch = WriteBatch::default();
        core.discover(
            &cand(&norm("https://site.test/page")),
            Some(0),
            0,
            0,
            &rules,
            &mut batch,
        );
        for n in 0..4 {
            core.discover(
                &cand(&norm(&format!("https://site.test/map-{n}"))),
                None,
                0,
                0,
                &rules,
                &mut batch,
            );
        }
        assert!(matches!(core.try_pop(), Pop::Item(_)));
        let before = core.memory().heap_entries;
        for _ in 0..100 {
            assert!(matches!(core.try_pop(), Pop::Empty));
        }
        assert_eq!(core.memory().heap_entries, before);
        core.finish_worker(true);
        for _ in 0..4 {
            assert!(matches!(core.try_pop(), Pop::Item(_)));
        }
        assert_eq!(core.active, 4);
        assert_eq!(core.active_pages, 0);
    }
}
