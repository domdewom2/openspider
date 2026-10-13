//! Pflege der Zähler in `urls` (Inlinks, Outlinks, …) durch den Writer.
//!
//! Zähler zählen nur Links mit **interner Quelle** (ONPAGE-015, Annahme A-9):
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
//!
//! Der Writer arbeitet mit Differenzen: Der Beitrag einer Seite (je Ziel: Hyperlink- und Gesamtzahl; die
//! Menge der Bild-Ziele) wird vor (aus den bisherigen Kindzeilen, nur bei `replace`) und nach dem Schreiben
//! bestimmt, die Differenz auf die betroffenen `urls`-Zeilen addiert. Jede betroffene Zeile erhält dabei die
//! Sequenz des Änderungsfeeds.

use std::collections::{BTreeMap, BTreeSet, HashMap};

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
}

/// Beitrag einer Seite zu den Zählern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Contribution {
    /// Je Ziel die Zählung.
    pub targets: BTreeMap<i64, TargetCount>,
    /// Verschiedene Bild-Ziele (`image_instances.image_id`).
    pub images: BTreeSet<i64>,
}

impl Contribution {
    /// Beitrag aus den Link- und Bildzeilen eines `PageWrite`. Links ohne Ziel-ID zählen nicht.
    pub fn from_rows(links: &[LinkRow], images: &[ImageRow]) -> Self {
        let mut c = Contribution::default();
        for link in links {
            if let Some(target) = link.target_id {
                let entry = c.targets.entry(target.0).or_default();
                entry.all += 1;
                if link.link_type == LinkType::Hyperlink {
                    entry.hyper += 1;
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

    /// Bisheriger Beitrag aus den gespeicherten Kindzeilen der Seite.
    pub fn load(conn: &Connection, page_id: i64) -> Result<Self, StoreError> {
        let mut c = Contribution::default();
        let mut stmt = conn.prepare_cached(
            "SELECT target_id, COUNT(*), SUM(link_type = 'Hyperlink') FROM links \
             WHERE source_id = ?1 AND target_id IS NOT NULL GROUP BY target_id",
        )?;
        let mut rows = stmt.query([page_id])?;
        while let Some(row) = rows.next()? {
            c.targets.insert(
                row.get(0)?,
                TargetCount {
                    all: row.get(1)?,
                    hyper: row.get(2)?,
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

/// Differenz der Zähler einer `urls`-Zeile.
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
}

/// Sammelt die Zählerdifferenzen aller Seiten eines Batches und schreibt sie am Ende mit einem `UPDATE` je
/// betroffener Zeile. Das spart bei Seiten, die viele Seiten gemeinsam verlinken (Navigation, Fußzeile), den
/// größten Teil der Schreibzugriffe auf `urls`.
///
/// Ziele müssen zum Zeitpunkt des Schreibens in `urls` stehen (in diesem oder einem früheren Batch angelegt,
/// `new_urls` wird zuerst verarbeitet); ein unbekanntes Ziel erhält keinen Zähler, und der Link zählt
/// nicht als interner oder externer Outlink der Quelle.
#[derive(Debug, Default)]
pub(crate) struct BatchCounters {
    deltas: BTreeMap<i64, Delta>,
    internal: HashMap<i64, Option<bool>>,
}

impl BatchCounters {
    /// Ob eine Zeile intern (`Some(true)`), extern (`Some(false)`) oder unbekannt (`None`) ist.
    fn is_internal(&mut self, conn: &Connection, id: i64) -> Result<Option<bool>, StoreError> {
        if let Some(v) = self.internal.get(&id) {
            return Ok(*v);
        }
        let mut stmt = conn.prepare_cached("SELECT is_internal FROM urls WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        let value = match rows.next()? {
            Some(row) => Some(row.get::<_, i64>(0)? != 0),
            None => None,
        };
        // Unbekannte Ziele nicht merken: sie können im selben Batch später angelegt werden.
        if value.is_some() {
            self.internal.insert(id, value);
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
            "UPDATE urls SET inlinks = inlinks + ?1, unique_inlinks = unique_inlinks + ?2, \
             self_inlinks = self_inlinks + ?3, inlinks_all = inlinks_all + ?4, img_inlinks = img_inlinks + ?5, \
             outlinks = outlinks + ?6, unique_outlinks = unique_outlinks + ?7, self_outlinks = self_outlinks + ?8, \
             external_outlinks = external_outlinks + ?9, unique_external_outlinks = unique_external_outlinks + ?10, \
             change_seq = ?11 WHERE id = ?12",
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
                seq,
                id
            ])?;
        }
        Ok(())
    }
}

/// Zähler einer Zeile, wie sie sich aus den Tabellen `links`/`image_instances` ergeben (Gegenprobe für Tests
/// und Prüfwerkzeuge; kostet einen Durchlauf über die Linktabelle).
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
}

/// Berechnet die Zähler der URL `id` vollständig neu aus `links` und `image_instances` (nur interne Quellen).
pub fn recompute(conn: &Connection, id: i64) -> Result<RecomputedCounters, StoreError> {
    let inl = conn.query_row(
        "SELECT COALESCE(SUM(l.link_type = 'Hyperlink'), 0), \
                COUNT(DISTINCT CASE WHEN l.link_type = 'Hyperlink' THEN l.source_id END), \
                COALESCE(SUM(l.link_type = 'Hyperlink' AND l.source_id = l.target_id), 0), \
                COUNT(*) \
         FROM links l JOIN urls s ON s.id = l.source_id \
         WHERE l.target_id = ?1 AND s.is_internal = 1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let out = conn.query_row(
        "SELECT COALESCE(SUM(t.is_internal = 1), 0), \
                COUNT(DISTINCT CASE WHEN t.is_internal = 1 THEN l.target_id END), \
                COALESCE(SUM(l.target_id = l.source_id), 0), \
                COALESCE(SUM(t.is_internal = 0), 0), \
                COUNT(DISTINCT CASE WHEN t.is_internal = 0 THEN l.target_id END) \
         FROM links l JOIN urls t ON t.id = l.target_id \
         JOIN urls s ON s.id = l.source_id \
         WHERE l.source_id = ?1 AND l.link_type = 'Hyperlink' AND s.is_internal = 1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    let img: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT i.page_id) FROM image_instances i JOIN urls s ON s.id = i.page_id \
         WHERE i.image_id = ?1 AND s.is_internal = 1",
        [id],
        |r| r.get(0),
    )?;
    Ok(RecomputedCounters {
        inlinks: inl.0,
        unique_inlinks: inl.1,
        self_inlinks: inl.2,
        inlinks_all: inl.3,
        outlinks: out.0,
        unique_outlinks: out.1,
        self_outlinks: out.2,
        external_outlinks: out.3,
        unique_external_outlinks: out.4,
        img_inlinks: img,
    })
}

/// Gespeicherte Zähler der URL `id`.
pub fn stored(conn: &Connection, id: i64) -> Result<RecomputedCounters, StoreError> {
    Ok(conn.query_row(
        "SELECT inlinks, unique_inlinks, self_inlinks, inlinks_all, outlinks, unique_outlinks, \
                self_outlinks, external_outlinks, unique_external_outlinks, img_inlinks \
         FROM urls WHERE id = ?1",
        [id],
        |r| {
            Ok(RecomputedCounters {
                inlinks: r.get(0)?,
                unique_inlinks: r.get(1)?,
                self_inlinks: r.get(2)?,
                inlinks_all: r.get(3)?,
                outlinks: r.get(4)?,
                unique_outlinks: r.get(5)?,
                self_outlinks: r.get(6)?,
                external_outlinks: r.get(7)?,
                unique_external_outlinks: r.get(8)?,
                img_inlinks: r.get(9)?,
            })
        },
    )?)
}

/// Prüft alle URLs: Liefert die Kennungen, deren gespeicherte Zähler von der Neuberechnung abweichen.
pub fn verify_all(conn: &Connection) -> Result<Vec<i64>, StoreError> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM urls ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut bad = Vec::new();
    for id in ids {
        if stored(conn, id)? != recompute(conn, id)? {
            bad.push(id);
        }
    }
    Ok(bad)
}
