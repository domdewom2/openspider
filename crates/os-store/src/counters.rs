//! Pflege der Zähler in `url_counters` (Inlinks, Outlinks, …) durch den Writer (Schema v2, K-75).
//!
//! Zähler zählen nur Links mit **interner Quelle** (ONPAGE-015, Annahme A-9). Die Tabelle `url_counters`
//! (eine Zeile je URL, `WITHOUT ROWID`) ist die einzige Quelle; die gleichnamigen Spalten in `urls` bleiben aus
//! Gründen der Rückwärtsverträglichkeit im Schema, werden aber ab v2 weder gepflegt noch gelesen.
//!
//! | Spalte (Zeile des Ziels, falls nicht anders genannt) | Bedeutung |
//! |---|---|
//! | `inlinks` | Hyperlink-Instanzen (auch mehrfach von derselben Seite, inkl. Selbstlinks) |
//! | `unique_inlinks` | verschiedene Quellseiten mit mindestens einem Hyperlink |
//! | `self_inlinks` | Hyperlink-Instanzen von der Seite auf sich selbst |
//! | `inlinks_all` | Instanzen aller Linktypen (inkl. Ressourcen) |
//! | `img_inlinks` | verschiedene Seiten mit mindestens einer Bild-Instanz |
//! | `outlinks`, `unique_outlinks` (Zeile der Quelle) | Hyperlink-Instanzen / verschiedene Ziele auf **interne** Ziele |
//! | `self_outlinks` (Quelle) | Hyperlink-Instanzen auf sich selbst |
//! | `external_outlinks`, `unique_external_outlinks` (Quelle) | wie `outlinks`, aber auf **externe** Ziele |
//! | `html_2xx_sources` | verschiedene interne HTML-Quellseiten mit Status 2xx und mindestens einem Hyperlink (`% of Total`, K-56) |
//! | `image_link_inlinks` | Link-Instanzen vom Typ `Image` (Images-Grundmenge, ONPAGE-170) |
//!
//! Der Writer arbeitet mit Differenzen: Der Beitrag einer Seite (je Ziel: Hyperlink-, Bild- und Gesamtzahl; die
//! Menge der Bild-Ziele; ob die Seite eine interne HTML-Seite mit Status 2xx ist) wird vor (aus den bisherigen
//! Kindzeilen und der bisherigen Zeile der Seite, nur bei `replace`) und nach dem Schreiben bestimmt, die Differenz
//! auf die betroffenen Zeilen von `url_counters` addiert (aufsteigend nach Kennung, alles in der Transaktion des
//! Batches). Jede betroffene Zeile erhält dabei die Sequenz des Änderungsfeeds (`url_counters.change_seq`).

use std::collections::{BTreeMap, BTreeSet};

use os_model::{ImageRow, LinkRow, LinkType};
use rusqlite::{params, Connection};

use crate::error::StoreError;

/// Zählung der Links einer Seite auf ein Ziel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TargetCount {
    /// Hyperlink-Instanzen.
    pub hyper: i64,
    /// Instanzen aller Linktypen.
    pub all: i64,
    /// Instanzen vom Typ `Image`.
    pub image: i64,
}

/// Beitrag einer Seite zu den Zählern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Contribution {
    /// Je Ziel die Zählung.
    pub targets: BTreeMap<i64, TargetCount>,
    /// Verschiedene Bild-Ziele (`image_instances.image_id`).
    pub images: BTreeSet<i64>,
    /// Die Quellseite ist eine HTML-Seite mit Status 2xx (zählt für `html_2xx_sources` ihrer Ziele).
    pub html_2xx: bool,
}

impl Contribution {
    /// Beitrag aus den Link- und Bildzeilen eines `PageWrite`. Links ohne Ziel-ID zählen nicht. `html_2xx`: die
    /// Seite ist eine HTML-Seite mit Status 2xx.
    pub fn from_rows(links: &[LinkRow], images: &[ImageRow], html_2xx: bool) -> Self {
        let mut c = Contribution {
            html_2xx,
            ..Contribution::default()
        };
        for link in links {
            if !link.crawlable {
                continue;
            }
            if let Some(target) = link.target_id {
                let entry = c.targets.entry(target.0).or_default();
                entry.all += 1;
                match link.link_type {
                    LinkType::Hyperlink => entry.hyper += 1,
                    LinkType::Image => entry.image += 1,
                    _ => {}
                }
            }
        }
        for image in images {
            if let Some(id) = image.image_id {
                c.images.insert(id.0);
            }
        }
        c
    }

    /// Bisheriger Beitrag aus den gespeicherten Kindzeilen der Seite; `html_2xx` ist der bisherige Zustand der Seite
    /// (Status und Klasse stehen vor dem Überschreiben durch das neue `PageWrite` fest).
    pub fn load(conn: &Connection, page_id: i64, html_2xx: bool) -> Result<Self, StoreError> {
        let mut c = Contribution {
            html_2xx,
            ..Contribution::default()
        };
        let mut stmt = conn.prepare_cached(
            "SELECT target_id, COUNT(*), SUM(link_type = 'Hyperlink' AND crawlable = 1), SUM(link_type = 'Image') FROM links \
             WHERE source_id = ?1 AND crawlable = 1 AND target_id IS NOT NULL GROUP BY target_id",
        )?;
        let mut rows = stmt.query([page_id])?;
        while let Some(row) = rows.next()? {
            c.targets.insert(
                row.get(0)?,
                TargetCount {
                    all: row.get(1)?,
                    hyper: row.get(2)?,
                    image: row.get(3)?,
                },
            );
        }
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT image_id FROM image_instances \
             WHERE page_id = ?1 AND image_id IS NOT NULL",
        )?;
        let mut rows = stmt.query([page_id])?;
        while let Some(row) = rows.next()? {
            c.images.insert(row.get(0)?);
        }
        Ok(c)
    }

    fn is_empty(&self) -> bool {
        self.targets.is_empty() && self.images.is_empty()
    }
}

/// Differenz der Zähler einer `url_counters`-Zeile.
#[derive(Debug, Clone, Copy, Default)]
struct Delta {
    inlinks: i64,
    unique_inlinks: i64,
    self_inlinks: i64,
    inlinks_all: i64,
    img_inlinks: i64,
    outlinks: i64,
    unique_outlinks: i64,
    self_outlinks: i64,
    external_outlinks: i64,
    unique_external_outlinks: i64,
    html_2xx_sources: i64,
    image_link_inlinks: i64,
}

/// Merkt je URL-Kennung, ob die URL intern ist. Die Zähler brauchen es für jedes Linkziel (interner oder externer
/// Outlink der Quelle); die Auskunft aus der breiten `urls`-Zeile kostet bei gleichverteilten Zielen je Batch rund
/// 60 000 zufällige Lesezugriffe auf eine Tabelle, die bei 1 Mio. URLs nicht mehr im Cache liegt. Der Writer trägt die
/// Auskunft beim Anlegen der URLs ein (`NewUrl.is_internal`, ändert sich nie; Kennungen werden nicht wiederverwendet),
/// URLs aus früheren Läufen werden beim ersten Bedarf einmal gelesen. Ein Byte je Kennung (1 MB je 1 Mio. URLs);
/// Kennungen ab [`InternalFlags::MAX_ID`] werden nicht gemerkt, sondern jedes Mal gelesen.
#[derive(Debug, Default)]
pub(crate) struct InternalFlags {
    /// 0 unbekannt, 1 extern, 2 intern.
    flags: Vec<u8>,
}

impl InternalFlags {
    /// Größte Kennung, die gemerkt wird (16 Mio. Kennungen = 16 MB).
    pub const MAX_ID: i64 = 16 << 20;

    pub fn get(&self, id: i64) -> Option<bool> {
        let slot = usize::try_from(id).ok()?;
        match self.flags.get(slot).copied() {
            Some(1) => Some(false),
            Some(2) => Some(true),
            _ => None,
        }
    }

    pub fn set(&mut self, id: i64, internal: bool) {
        if !(0..Self::MAX_ID).contains(&id) {
            return;
        }
        let slot = id as usize;
        if self.flags.len() <= slot {
            // geometrisch wachsen: höchstens ein Umkopieren je Verdopplung
            let new_len = (slot + 1).max(self.flags.len() * 2).max(1024);
            self.flags.resize(new_len, 0);
        }
        self.flags[slot] = if internal { 2 } else { 1 };
    }
}

/// Sammelt die Zählerdifferenzen aller Seiten eines Batches und schreibt sie am Ende mit einem `UPDATE` je
/// betroffener Zeile von `url_counters`. Das spart bei Seiten, die viele Seiten gemeinsam verlinken (Navigation,
/// Fußzeile), den größten Teil der Schreibzugriffe.
///
/// Ziele müssen zum Zeitpunkt des Schreibens in `urls` stehen (in diesem oder einem früheren Batch angelegt,
/// `new_urls` wird zuerst verarbeitet); ein unbekanntes Ziel erhält keinen Zähler, und der Link zählt
/// nicht als interner oder externer Outlink der Quelle.
#[derive(Debug)]
pub(crate) struct BatchCounters<'a> {
    deltas: BTreeMap<i64, Delta>,
    internal: &'a mut InternalFlags,
}

impl<'a> BatchCounters<'a> {
    /// Leere Sammlung; `internal` ist das Gedächtnis des Writers über Batches hinweg.
    pub fn new(internal: &'a mut InternalFlags) -> Self {
        BatchCounters {
            deltas: BTreeMap::new(),
            internal,
        }
    }

    /// Ob eine Zeile intern (`Some(true)`), extern (`Some(false)`) oder unbekannt (`None`) ist.
    fn is_internal(&mut self, conn: &Connection, id: i64) -> Result<Option<bool>, StoreError> {
        if let Some(v) = self.internal.get(id) {
            return Ok(Some(v));
        }
        let mut stmt = conn.prepare_cached("SELECT is_internal FROM urls WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        let value = match rows.next()? {
            Some(row) => Some(row.get::<_, i64>(0)? != 0),
            None => None,
        };
        // Unbekannte Ziele nicht merken: sie können im selben Batch später angelegt werden.
        if let Some(v) = value {
            self.internal.set(id, v);
        }
        Ok(value)
    }

    /// Zähler der Quellseite aus ihrem Beitrag: `(outlinks, unique_outlinks, self_outlinks,
    /// external_outlinks, unique_external_outlinks)`.
    fn out_counts(
        &mut self,
        conn: &Connection,
        page_id: i64,
        c: &Contribution,
    ) -> Result<[i64; 5], StoreError> {
        let mut out = [0i64; 5];
        for (&target, count) in &c.targets {
            if count.hyper == 0 {
                continue;
            }
            if target == page_id {
                out[2] += count.hyper;
            }
            let internal = if target == page_id {
                Some(true)
            } else {
                self.is_internal(conn, target)?
            };
            match internal {
                Some(true) => {
                    out[0] += count.hyper;
                    out[1] += 1;
                }
                Some(false) => {
                    out[3] += count.hyper;
                    out[4] += 1;
                }
                None => {}
            }
        }
        Ok(out)
    }

    /// Nimmt den Übergang des Beitrags einer Seite von `old` auf `new` auf. Die Quellseite muss intern sein
    /// (Prüfung durch den Aufrufer).
    pub fn add_page(
        &mut self,
        conn: &Connection,
        page_id: i64,
        old: &Contribution,
        new: &Contribution,
    ) -> Result<(), StoreError> {
        if old.is_empty() && new.is_empty() {
            return Ok(());
        }
        let targets: BTreeSet<i64> = old
            .targets
            .keys()
            .chain(new.targets.keys())
            .copied()
            .collect();
        for target in targets {
            let o = old.targets.get(&target).copied().unwrap_or_default();
            let n = new.targets.get(&target).copied().unwrap_or_default();
            let d = self.deltas.entry(target).or_default();
            d.inlinks += n.hyper - o.hyper;
            d.unique_inlinks += i64::from(n.hyper > 0) - i64::from(o.hyper > 0);
            d.inlinks_all += n.all - o.all;
            d.image_link_inlinks += n.image - o.image;
            d.html_2xx_sources +=
                i64::from(new.html_2xx && n.hyper > 0) - i64::from(old.html_2xx && o.hyper > 0);
            if target == page_id {
                d.self_inlinks += n.hyper - o.hyper;
            }
        }
        for image in old.images.union(&new.images) {
            let d = self.deltas.entry(*image).or_default();
            d.img_inlinks +=
                i64::from(new.images.contains(image)) - i64::from(old.images.contains(image));
        }

        let before = self.out_counts(conn, page_id, old)?;
        let after = self.out_counts(conn, page_id, new)?;
        let d = self.deltas.entry(page_id).or_default();
        d.outlinks += after[0] - before[0];
        d.unique_outlinks += after[1] - before[1];
        d.self_outlinks += after[2] - before[2];
        d.external_outlinks += after[3] - before[3];
        d.unique_external_outlinks += after[4] - before[4];
        Ok(())
    }

    /// Schreibt die gesammelten Differenzen (aufsteigend nach Kennung) und stempelt jede betroffene Zeile
    /// mit `seq`.
    pub fn flush(self, conn: &Connection, seq: i64) -> Result<(), StoreError> {
        if self.deltas.is_empty() {
            return Ok(());
        }
        let mut stmt = conn.prepare_cached(
            "UPDATE url_counters SET inlinks = inlinks + ?1, unique_inlinks = unique_inlinks + ?2, \
             self_inlinks = self_inlinks + ?3, inlinks_all = inlinks_all + ?4, img_inlinks = img_inlinks + ?5, \
             outlinks = outlinks + ?6, unique_outlinks = unique_outlinks + ?7, self_outlinks = self_outlinks + ?8, \
             external_outlinks = external_outlinks + ?9, unique_external_outlinks = unique_external_outlinks + ?10, \
             html_2xx_sources = html_2xx_sources + ?11, image_link_inlinks = image_link_inlinks + ?12, \
             change_seq = ?13 WHERE url_id = ?14",
        )?;
        for (id, d) in self.deltas {
            stmt.execute(params![
                d.inlinks,
                d.unique_inlinks,
                d.self_inlinks,
                d.inlinks_all,
                d.img_inlinks,
                d.outlinks,
                d.unique_outlinks,
                d.self_outlinks,
                d.external_outlinks,
                d.unique_external_outlinks,
                d.html_2xx_sources,
                d.image_link_inlinks,
                seq,
                id
            ])?;
        }
        Ok(())
    }
}

/// Der Trigger aus Migration 0002, der jeder neuen `urls`-Zeile ihre Zählerzeile gibt.
/// Auch `os-loadtest::gen_ui` (UI-148, M2-K19) lädt `urls_counters_insert` über
/// `sqlite_master`, entfernt ihn beim Bulk-Laden und stellt seine SQL-Definition
/// wieder her; die Sekundärindizes von urls/links/url_counters lädt es ebenso.
/// Bei Schema-/Namensänderungen den Vertragstest `gen_ui_large_fixture` mitprüfen.
pub(crate) const INSERT_TRIGGER_SQL: &str =
    "CREATE TRIGGER IF NOT EXISTS urls_counters_insert AFTER INSERT ON urls \
     BEGIN INSERT OR IGNORE INTO url_counters (url_id) VALUES (new.id); END";

/// Entfernt den Einfüge-Trigger. Der Writer legt die Zählerzeile selbst an: Ein Trigger zwingt SQLite zu einem
/// Statement-Journal je eingefügter `urls`-Zeile (Messung M1-P32: 50 µs statt 7 µs je Zeile, bei 1 Mio. URLs rund
/// 50 s).
pub(crate) fn drop_insert_trigger(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch("DROP TRIGGER IF EXISTS urls_counters_insert")?;
    Ok(())
}

/// Legt den Einfüge-Trigger wieder an (Ende, Pause, Schließen der Senke), damit die Datei im Ruhezustand das volle
/// Schema hat und direkte Einfügungen durch Werkzeuge und Tests weiter eine Zählerzeile bekommen.
pub(crate) fn restore_insert_trigger(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(INSERT_TRIGGER_SQL)?;
    Ok(())
}

/// Zähler einer Zeile, wie sie sich aus den Tabellen `links`/`image_instances` ergeben (Gegenprobe für Tests,
/// Prüfwerkzeuge und das Neu-Crawlen; kostet je Aufruf höchstens einen Durchlauf über die Linktabelle, auch ohne den
/// Index `links_target`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecomputedCounters {
    /// `inlinks`.
    pub inlinks: i64,
    /// `unique_inlinks`.
    pub unique_inlinks: i64,
    /// `self_inlinks`.
    pub self_inlinks: i64,
    /// `inlinks_all`.
    pub inlinks_all: i64,
    /// `outlinks`.
    pub outlinks: i64,
    /// `unique_outlinks`.
    pub unique_outlinks: i64,
    /// `self_outlinks`.
    pub self_outlinks: i64,
    /// `external_outlinks`.
    pub external_outlinks: i64,
    /// `unique_external_outlinks`.
    pub unique_external_outlinks: i64,
    /// `img_inlinks`.
    pub img_inlinks: i64,
    /// `html_2xx_sources`.
    pub html_2xx_sources: i64,
    /// `image_link_inlinks`.
    pub image_link_inlinks: i64,
}

/// Spalten von `url_counters` in der Reihenfolge von [`RecomputedCounters`].
const COUNTER_COLUMNS: [&str; 12] = [
    "inlinks",
    "unique_inlinks",
    "self_inlinks",
    "inlinks_all",
    "outlinks",
    "unique_outlinks",
    "self_outlinks",
    "external_outlinks",
    "unique_external_outlinks",
    "img_inlinks",
    "html_2xx_sources",
    "image_link_inlinks",
];

fn recomputed_from_row(
    row: &rusqlite::Row<'_>,
    first: usize,
) -> rusqlite::Result<RecomputedCounters> {
    Ok(RecomputedCounters {
        inlinks: row.get(first)?,
        unique_inlinks: row.get(first + 1)?,
        self_inlinks: row.get(first + 2)?,
        inlinks_all: row.get(first + 3)?,
        outlinks: row.get(first + 4)?,
        unique_outlinks: row.get(first + 5)?,
        self_outlinks: row.get(first + 6)?,
        external_outlinks: row.get(first + 7)?,
        unique_external_outlinks: row.get(first + 8)?,
        img_inlinks: row.get(first + 9)?,
        html_2xx_sources: row.get(first + 10)?,
        image_link_inlinks: row.get(first + 11)?,
    })
}

/// SQL der Neuberechnung: eine Zeile je betrachteter URL mit den zwölf Zählern in der Reihenfolge von
/// [`COUNTER_COLUMNS`] (erste Spalte `id`). `filtered`: nur die URLs aus dem JSON-Array in `?1`, sonst alle (nach
/// Kennung geordnet, dahinter die zwölf gespeicherten Zähler aus `url_counters`, `NULL` ohne Zeile).
///
/// Jede Gruppe liest `links` höchstens einmal, mit dem Index `links_target` über die Kennungen, ohne ihn in einem
/// sequenziellen Lauf (nie ein Lauf je URL): Die Aufrufer fragen deshalb immer die ganze Menge auf einmal ab.
fn recompute_sql(filtered: bool) -> String {
    let stored_columns = COUNTER_COLUMNS
        .iter()
        .map(|c| format!("c.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let (ids_cte, in_filter, out_filter, img_filter, from, tail_select, tail_join) = if filtered {
        (
            "ids(id) AS (SELECT value FROM json_each(?1)), ",
            "AND l.target_id IN (SELECT id FROM ids)",
            "AND l.source_id IN (SELECT id FROM ids)",
            "AND i.image_id IN (SELECT id FROM ids)",
            "ids",
            String::new(),
            String::new(),
        )
    } else {
        (
            "",
            "",
            "",
            "",
            "urls",
            format!(", {stored_columns}"),
            " LEFT JOIN url_counters c ON c.url_id = urls.id ORDER BY urls.id".to_string(),
        )
    };
    format!(
        "WITH {ids_cte}\
         inl AS (SELECT l.target_id AS id, \
                 SUM(l.link_type = 'Hyperlink' AND l.crawlable = 1) AS inlinks, \
                 COUNT(DISTINCT CASE WHEN l.link_type = 'Hyperlink' AND l.crawlable = 1 THEN l.source_id END) AS unique_inlinks, \
                 SUM(l.link_type = 'Hyperlink' AND l.crawlable = 1 AND l.source_id = l.target_id) AS self_inlinks, \
                 COUNT(*) AS inlinks_all, \
                 COUNT(DISTINCT CASE WHEN l.link_type = 'Hyperlink' AND l.crawlable = 1 AND s.content_class = 'HTML' \
                       AND s.status_code BETWEEN 200 AND 299 THEN l.source_id END) AS html_2xx_sources, \
                 SUM(l.link_type = 'Image') AS image_link_inlinks \
                 FROM links l JOIN urls s ON s.id = l.source_id \
                 WHERE l.target_id IS NOT NULL AND s.is_internal = 1 {in_filter} GROUP BY l.target_id), \
         outl AS (SELECT l.source_id AS id, \
                 SUM(t.is_internal = 1) AS outlinks, \
                 COUNT(DISTINCT CASE WHEN t.is_internal = 1 THEN l.target_id END) AS unique_outlinks, \
                 SUM(l.target_id = l.source_id) AS self_outlinks, \
                 SUM(t.is_internal = 0) AS external_outlinks, \
                 COUNT(DISTINCT CASE WHEN t.is_internal = 0 THEN l.target_id END) AS unique_external_outlinks \
                 FROM links l JOIN urls t ON t.id = l.target_id JOIN urls s ON s.id = l.source_id \
                 WHERE l.link_type = 'Hyperlink' AND l.crawlable = 1 AND s.is_internal = 1 {out_filter} GROUP BY l.source_id), \
         img AS (SELECT i.image_id AS id, COUNT(DISTINCT i.page_id) AS n \
                 FROM image_instances i JOIN urls s ON s.id = i.page_id \
                 WHERE i.image_id IS NOT NULL AND s.is_internal = 1 {img_filter} GROUP BY i.image_id) \
         SELECT {from}.id, \
                COALESCE(inl.inlinks, 0), COALESCE(inl.unique_inlinks, 0), COALESCE(inl.self_inlinks, 0), \
                COALESCE(inl.inlinks_all, 0), \
                COALESCE(outl.outlinks, 0), COALESCE(outl.unique_outlinks, 0), COALESCE(outl.self_outlinks, 0), \
                COALESCE(outl.external_outlinks, 0), COALESCE(outl.unique_external_outlinks, 0), \
                COALESCE(img.n, 0), COALESCE(inl.html_2xx_sources, 0), COALESCE(inl.image_link_inlinks, 0) \
                {tail_select} \
         FROM {from} LEFT JOIN inl ON inl.id = {from}.id LEFT JOIN outl ON outl.id = {from}.id \
              LEFT JOIN img ON img.id = {from}.id{tail_join}",
    )
}

/// Berechnet die Zähler der URLs `ids` vollständig neu aus `links` und `image_instances` (nur interne Quellen):
/// ein Durchlauf je Gruppe für die **ganze** Menge – mit dem Index `links_target` über die Kennungen, ohne ihn
/// in einem sequenziellen Lauf über `links`. Kennungen ohne `urls`-Zeile liefern Nullen.
pub fn recompute_many(
    conn: &Connection,
    ids: &[i64],
) -> Result<BTreeMap<i64, RecomputedCounters>, StoreError> {
    let mut out = BTreeMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let json = format!(
        "[{}]",
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    );
    let mut stmt = conn.prepare_cached(&recompute_sql(true))?;
    let mut rows = stmt.query([json])?;
    while let Some(row) = rows.next()? {
        out.insert(row.get(0)?, recomputed_from_row(row, 1)?);
    }
    Ok(out)
}

/// Berechnet die Zähler der URL `id` vollständig neu aus `links` und `image_instances` (nur interne Quellen).
pub fn recompute(conn: &Connection, id: i64) -> Result<RecomputedCounters, StoreError> {
    Ok(recompute_many(conn, &[id])?.remove(&id).unwrap_or_default())
}

/// Gespeicherte Zähler der URL `id` (Tabelle `url_counters`).
pub fn stored(conn: &Connection, id: i64) -> Result<RecomputedCounters, StoreError> {
    let sql = format!(
        "SELECT {} FROM url_counters WHERE url_id = ?1",
        COUNTER_COLUMNS.join(", ")
    );
    Ok(conn.query_row(&sql, [id], |r| recomputed_from_row(r, 0))?)
}

/// Prüft alle URLs: Liefert die Kennungen (aufsteigend), deren gespeicherte Zähler von der Neuberechnung abweichen
/// oder die keine Zeile in `url_counters` haben. Ein Durchlauf je Gruppe über die ganze Datei, kein Lauf je URL;
/// die Zeilen werden gestreamt.
pub fn verify_all(conn: &Connection) -> Result<Vec<i64>, StoreError> {
    let mut stmt = conn.prepare(&recompute_sql(false))?;
    let mut rows = stmt.query([])?;
    let mut bad = Vec::new();
    while let Some(row) = rows.next()? {
        let expected = recomputed_from_row(row, 1)?;
        let stored = match row.get::<_, Option<i64>>(13)? {
            None => None,
            Some(_) => Some(recomputed_from_row(row, 13)?),
        };
        if stored != Some(expected) {
            bad.push(row.get(0)?);
        }
    }
    Ok(bad)
}
