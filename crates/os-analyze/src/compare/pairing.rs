//! Zuordnung der URLs beider Crawls (OUT-142, OUT-146).
//!
//! **Existenz (OS-Festlegung, OUT-142):** Eine URL „existiert in einem Crawl“, wenn sie dort gecrawlt wurde
//! (`urls.in_tables = 1 AND urls.state = 'done'`), nicht nur entdeckt oder blockiert. Nur existierende URLs bilden
//! Paare; eine nur entdeckte URL ist für die Klassifikation ohne Partner.
//!
//! **Paarbildung:** Jede existierende Previous-URL wird mit dem URL-Mapping abgebildet; trifft das Ergebnis eine
//! existierende Current-URL, ist das ein Paar-Kandidat. Kollisionen (mehrere Previous-URLs auf dieselbe Current-URL)
//! entscheidet die Regel aus OUT-146: unverändert identische Previous-URL vor jeder gemappten, sonst die mit der am
//! weitesten vorn stehenden ersten passenden Regel, bei Gleichstand die lexikographisch kleinste Previous-URL. Die
//! unterlegenen Previous-URLs bleiben ohne Partner; jede Kollision wird geloggt und als [`Collision`] gemeldet.

use std::collections::HashMap;

use os_model::CancelToken;
use os_store::CrawlFile;
use roaring::RoaringBitmap;

use super::mapping::UrlMapping;
use super::CompareError;

/// Bedingung über `urls` (ohne Alias) für „existiert im Crawl“.
pub const EXISTS_SQL: &str = "in_tables = 1 AND state = 'done'";

/// Eine Kollision des URL-Mappings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// Current-URL, auf die mehrere Previous-URLs abbilden.
    pub current_url: String,
    /// Previous-URL, die das Paar bildet.
    pub winner: String,
    /// Previous-URLs ohne Partner (sortiert).
    pub losers: Vec<String>,
}

/// Paare zwischen Current (`c`) und Previous (`p`) als URL-Kennungen beider Dateien.
#[derive(Debug, Clone, Default)]
pub struct Pairing {
    cur_to_prev: HashMap<u32, u32>,
    prev_to_cur: HashMap<u32, u32>,
    /// Existierende URLs des Current-Crawls.
    pub current: RoaringBitmap,
    /// Existierende URLs des Previous-Crawls.
    pub previous: RoaringBitmap,
    /// Current-URLs mit Partner.
    pub paired_current: RoaringBitmap,
    /// Previous-URLs mit Partner.
    pub paired_previous: RoaringBitmap,
    /// Kollisionen des Mappings.
    pub collisions: Vec<Collision>,
}

pub(crate) fn url_index(id: i64) -> Result<u32, CompareError> {
    u32::try_from(id).map_err(|_| CompareError::Invalid(format!("URL-Kennung {id} außerhalb des Bitmap-Bereichs")))
}

/// Liest die existierenden URLs einer Datei als `(Kennung, URL)`.
fn existing(
    file: &CrawlFile,
    cancel: &CancelToken,
    mut f: impl FnMut(u32, String) -> Result<(), CompareError>,
) -> Result<(), CompareError> {
    let mut stmt = file
        .conn()
        .prepare(&format!("SELECT id, url FROM urls WHERE {EXISTS_SQL} ORDER BY id"))?;
    let mut rows = stmt.raw_query();
    let mut n = 0u32;
    while let Some(row) = rows.next()? {
        n = n.wrapping_add(1);
        if n % 4096 == 0 {
            super::checkpoint(cancel)?;
        }
        f(url_index(row.get(0)?)?, row.get(1)?)?;
    }
    Ok(())
}

impl Pairing {
    /// Bildet die Paare; `mapping` bildet Previous-URLs auf Current-URLs ab.
    pub fn build(
        current: &CrawlFile,
        previous: &CrawlFile,
        mapping: &UrlMapping,
        cancel: &CancelToken,
    ) -> Result<Pairing, CompareError> {
        super::checkpoint(cancel)?;
        let mut pairing = Pairing::default();
        let mut by_url: HashMap<String, u32> = HashMap::new();
        existing(current, cancel, |id, url| {
            pairing.current.insert(id);
            by_url.insert(url, id);
            Ok(())
        })?;
        // Ziel-Kennung → (Rang, Previous-URL, Previous-Kennung, unterlegene URLs).
        struct Candidate {
            rank: usize,
            url: String,
            id: u32,
            losers: Vec<String>,
        }
        let mut best: HashMap<u32, Candidate> = HashMap::new();
        existing(previous, cancel, |id, url| {
            pairing.previous.insert(id);
            let (target, rank) = if mapping.is_empty() {
                (by_url.get(&url).copied(), 0)
            } else {
                let mapped = mapping.apply(&url);
                (
                    by_url.get(&mapped.url).copied(),
                    mapped.first_rule.map_or(0, |r| r + 1),
                )
            };
            let Some(target) = target else {
                return Ok(());
            };
            match best.get_mut(&target) {
                None => {
                    best.insert(
                        target,
                        Candidate {
                            rank,
                            url,
                            id,
                            losers: Vec::new(),
                        },
                    );
                }
                Some(c) => {
                    if (rank, url.as_str()) < (c.rank, c.url.as_str()) {
                        let old = std::mem::replace(&mut c.url, url);
                        c.losers.push(old);
                        c.rank = rank;
                        c.id = id;
                    } else {
                        c.losers.push(url);
                    }
                }
            }
            Ok(())
        })?;
        let current_urls: HashMap<u32, &str> = if best.values().any(|c| !c.losers.is_empty()) {
            by_url.iter().map(|(u, id)| (*id, u.as_str())).collect()
        } else {
            HashMap::new()
        };
        let mut targets: Vec<_> = best.into_iter().collect();
        targets.sort_by_key(|(t, _)| *t);
        for (target, mut c) in targets {
            pairing.cur_to_prev.insert(target, c.id);
            pairing.prev_to_cur.insert(c.id, target);
            pairing.paired_current.insert(target);
            pairing.paired_previous.insert(c.id);
            if !c.losers.is_empty() {
                c.losers.sort();
                let current_url = current_urls.get(&target).copied().unwrap_or_default();
                tracing::warn!(
                    current = current_url,
                    winner = %c.url,
                    losers = ?c.losers,
                    "URL-Mapping: mehrere Previous-URLs bilden auf dieselbe Current-URL ab"
                );
                pairing.collisions.push(Collision {
                    current_url: current_url.to_string(),
                    winner: c.url,
                    losers: c.losers,
                });
            }
        }
        pairing
            .collisions
            .sort_by(|a, b| a.current_url.cmp(&b.current_url));
        Ok(pairing)
    }

    /// Dieselben Paare mit vertauschten Rollen (OUT-141).
    pub fn inverted(&self) -> Pairing {
        Pairing {
            cur_to_prev: self.prev_to_cur.clone(),
            prev_to_cur: self.cur_to_prev.clone(),
            current: self.previous.clone(),
            previous: self.current.clone(),
            paired_current: self.paired_previous.clone(),
            paired_previous: self.paired_current.clone(),
            collisions: self.collisions.clone(),
        }
    }

    /// Partner einer Current-URL im Previous-Crawl.
    pub fn previous_of(&self, current: u32) -> Option<u32> {
        self.cur_to_prev.get(&current).copied()
    }

    /// Partner einer Previous-URL im Current-Crawl.
    pub fn current_of(&self, previous: u32) -> Option<u32> {
        self.prev_to_cur.get(&previous).copied()
    }

    /// Bildet Previous-Kennungen mit Partner auf ihre Current-Kennungen ab (ohne Partner entfallen sie).
    pub fn to_current(&self, previous: &RoaringBitmap) -> RoaringBitmap {
        let mut out = RoaringBitmap::new();
        for id in previous & &self.paired_previous {
            if let Some(c) = self.current_of(id) {
                out.insert(c);
            }
        }
        out
    }

    /// Anzahl der Paare.
    pub fn pairs(&self) -> u64 {
        self.paired_current.len()
    }
}
