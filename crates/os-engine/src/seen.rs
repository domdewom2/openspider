//! Kompakte Menge der bekannten URLs („Seen-Menge“, K-78 a).
//!
//! Jeder Eintrag der Haupttabelle ist **16 Byte** groß (Schlüssel 8 + Fingerabdruck 4 + Kennung 4), dazu kommt ein
//! Kontrollbyte der Hash-Tabelle. Tiefe (28 Bit) und Flags (4 Bit) stehen kompakt in `meta`, einem Feld mit 4 Byte
//! je Kennung (Kennungen werden fortlaufend vergeben). Die alte Fassung brauchte 24 Byte je Eintrag.
//!
//! * **Schlüssel und Fingerabdruck:** Der 64-Bit-Schlüssel ist der Hash der URL Encoded Address
//!   ([`url_key`]); der **unabhängige** 32-Bit-Fingerabdruck stammt aus einem anderen Hash-Verfahren (FNV-1a). Zwei
//!   verschiedene Adressen mit gleichem Schlüssel *und* gleichem Fingerabdruck sind bei rund 96 Bit praktisch
//!   ausgeschlossen (bei 1 Mio. URLs Größenordnung 10^-17).
//! * **Echte Kollision:** gleicher Schlüssel, anderer Fingerabdruck. Die zweite URL kommt mit ihrer vollen Adresse
//!   in die kleine **Überlaufmenge**; beide URLs bleiben unterscheidbar (keine geht verloren, keine zählt doppelt).
//!   Ein Abgleich mit der Datenbank je Treffer entfällt, SQLite-`UNIQUE` bleibt die Wahrheit für Dubletten.
//! * **Überlaufmenge** nimmt außerdem Einträge auf, die nicht in 28 Bit Tiefe bzw. 32 Bit Kennung passen oder
//!   deren Kennung weit hinter der letzten liegt (dünn besetzte Kennungen). Im Normalbetrieb ist sie leer; dann kostet
//!   ein Fehlschlag in der Haupttabelle keinen weiteren Nachschlag.
//! * **Teiltabellen:** 256 Teiltabellen nach Schlüsselbits 24 bis 31. Das Vergrößern einer Tabelle verdoppelt nur
//!   1/256 des Speichers (die Spitze beim Wachsen bleibt klein), und die Tabellen wachsen zeitlich versetzt.
//! * **Injizierbare Hashfunktion:** [`SeenSet::with_key_fn`] ersetzt Schlüssel und Fingerabdruck (Test mit erzwungener
//!   Kollision).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::Mutex;

use os_model::UrlId;

use crate::diag;
use crate::frontier::{IdentityHasher, Seen, NO_DEPTH};

/// Schlüssel und Fingerabdruck einer Adresse.
pub type KeyFp = (u64, u32);

/// Hashfunktion der Seen-Menge: Adresse → `(Schlüssel, Fingerabdruck)`.
pub type KeyFn = fn(&str) -> KeyFp;

/// 64-Bit-Schlüssel einer URL Encoded Address (stabil innerhalb einer Rust-Version, nur im RAM benutzt).
pub fn url_key(encoded: &str) -> u64 {
    let mut h = DefaultHasher::new();
    encoded.hash(&mut h);
    h.finish()
}

/// Unabhängiger 32-Bit-Fingerabdruck: FNV-1a (64 Bit), die Hälften gefaltet.
pub fn url_fingerprint(encoded: &str) -> u32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in encoded.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ((h >> 32) ^ h) as u32
}

/// Standard: [`url_key`] und [`url_fingerprint`].
pub fn default_key_fp(encoded: &str) -> KeyFp {
    (url_key(encoded), url_fingerprint(encoded))
}

static KEY_FN_OVERRIDE: Mutex<Option<KeyFn>> = Mutex::new(None);

/// Ersetzt die Hashfunktion aller danach angelegten Seen-Mengen (nur für Tests mit erzwungener Kollision; ein Test,
/// der das benutzt, setzt sie nach dem Lauf mit `None` zurück und läuft in einer eigenen Testdatei).
#[doc(hidden)]
pub fn set_key_fn_override(f: Option<KeyFn>) {
    *KEY_FN_OVERRIDE.lock().unwrap_or_else(|p| p.into_inner()) = f;
}

/// Zahl der Teiltabellen.
const SHARDS: usize = 256;
/// Tiefe „unbekannt“ im gepackten Feld.
const PACKED_NO_DEPTH: u32 = 0x0FFF_FFFF;
/// Ab dieser Tiefe passt der Wert nicht mehr in 28 Bit (Überlaufmenge).
const MAX_PACKED_DEPTH: u32 = PACKED_NO_DEPTH - 1;
/// Alle vier Flags `F_*` liegen in den unteren Bits.
const FLAG_MASK: u8 = 0x0F;
/// Eine Kennung höchstens so weit hinter dem Ende von `meta` gilt noch als dicht (sonst Überlaufmenge).
const MAX_ID_GAP: usize = 1 << 20;

#[derive(Debug, Clone, Copy)]
struct Slot {
    fp: u32,
    id: u32,
}

type Shard = HashMap<u64, Slot, BuildHasherDefault<IdentityHasher>>;

/// Die Menge der bekannten URLs (siehe Moduldoku).
pub struct SeenSet {
    shards: Vec<Shard>,
    /// Je Kennung: Tiefe (obere 28 Bit) und Flags (untere 4 Bit).
    meta: Vec<u32>,
    overflow: HashMap<Box<str>, Seen>,
    overflow_bytes: usize,
    /// Einträge in den Teiltabellen.
    in_tables: usize,
    key_fn: KeyFn,
}

impl Default for SeenSet {
    fn default() -> Self {
        let f = *KEY_FN_OVERRIDE.lock().unwrap_or_else(|p| p.into_inner());
        Self::with_key_fn(f.unwrap_or(default_key_fp))
    }
}

fn pack(depth: u32, flags: u8) -> Option<u32> {
    if flags & !FLAG_MASK != 0 {
        return None;
    }
    let d = if depth == NO_DEPTH {
        PACKED_NO_DEPTH
    } else if depth <= MAX_PACKED_DEPTH {
        depth
    } else {
        return None;
    };
    Some((d << 4) | u32::from(flags))
}

fn unpack(meta: u32) -> (u32, u8) {
    let d = meta >> 4;
    (
        if d == PACKED_NO_DEPTH { NO_DEPTH } else { d },
        (meta & 0x0F) as u8,
    )
}

fn shard_of(key: u64) -> usize {
    ((key >> 24) & 0xFF) as usize
}

impl SeenSet {
    /// Leere Menge mit eigener Hashfunktion.
    pub fn with_key_fn(key_fn: KeyFn) -> Self {
        SeenSet {
            shards: (0..SHARDS).map(|_| Shard::default()).collect(),
            meta: Vec::new(),
            overflow: HashMap::new(),
            overflow_bytes: 0,
            in_tables: 0,
            key_fn,
        }
    }

    /// Schlüssel und Fingerabdruck einer Adresse.
    pub fn key_fp(&self, encoded: &str) -> KeyFp {
        (self.key_fn)(encoded)
    }

    /// Anzahl bekannter URLs.
    pub fn len(&self) -> usize {
        self.in_tables + self.overflow.len()
    }

    /// Ob keine URL bekannt ist.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Einträge der Überlaufmenge.
    pub fn overflow_len(&self) -> usize {
        self.overflow.len()
    }

    /// Schlägt eine Adresse nach.
    pub fn get(&self, encoded: &str) -> Option<Seen> {
        self.get_hashed(self.key_fp(encoded), encoded)
    }

    /// Schlägt eine Adresse mit bereits berechnetem Schlüssel nach.
    pub fn get_hashed(&self, kf: KeyFp, encoded: &str) -> Option<Seen> {
        let (key, fp) = kf;
        match self.shards[shard_of(key)].get(&key) {
            Some(slot) if slot.fp == fp => Some(self.assemble(*slot)),
            // gleicher Schlüssel, anderer Fingerabdruck: echte Kollision, die Adresse steht (falls bekannt) in der
            // Überlaufmenge
            Some(_) => self.overflow.get(encoded).copied(),
            None if self.overflow.is_empty() => None,
            None => self.overflow.get(encoded).copied(),
        }
    }

    fn assemble(&self, slot: Slot) -> Seen {
        let (depth, flags) = unpack(self.meta[slot.id as usize]);
        Seen {
            id: UrlId(i64::from(slot.id)),
            depth,
            flags,
        }
    }

    /// Trägt eine URL ein oder aktualisiert sie.
    pub fn insert(&mut self, encoded: &str, seen: Seen) {
        let kf = self.key_fp(encoded);
        self.insert_hashed(kf, encoded, seen);
    }

    /// Wie [`insert`](Self::insert) mit bereits berechnetem Schlüssel.
    pub fn insert_hashed(&mut self, kf: KeyFp, encoded: &str, seen: Seen) {
        let (key, fp) = kf;
        let packed = pack(seen.depth, seen.flags);
        let id32 = u32::try_from(seen.id.0).ok().filter(|id| {
            // dicht: nicht weit hinter dem bisherigen Ende von `meta`
            (*id as usize) < self.meta.len().saturating_add(MAX_ID_GAP)
        });
        if !self.overflow.is_empty() {
            if let Some(e) = self.overflow.get_mut(encoded) {
                *e = seen;
                return;
            }
        }
        let shard = shard_of(key);
        match self.shards[shard].get(&key).copied() {
            Some(slot) if slot.fp == fp => match (packed, id32) {
                (Some(m), Some(id)) => {
                    if slot.id != id {
                        self.shards[shard].insert(key, Slot { fp, id });
                    }
                    self.set_meta(id, m);
                }
                _ => {
                    // passt nicht mehr in die Haupttabelle
                    self.shards[shard].remove(&key);
                    self.in_tables -= 1;
                    self.put_overflow(encoded, seen);
                }
            },
            Some(_) => {
                diag::inc(&diag::STATS.seen_collisions);
                self.put_overflow(encoded, seen);
            }
            None => match (packed, id32) {
                (Some(m), Some(id)) => {
                    self.shards[shard].insert(key, Slot { fp, id });
                    self.in_tables += 1;
                    self.set_meta(id, m);
                }
                _ => self.put_overflow(encoded, seen),
            },
        }
    }

    fn set_meta(&mut self, id: u32, m: u32) {
        let i = id as usize;
        if self.meta.len() <= i {
            self.meta.resize(i + 1, pack(NO_DEPTH, 0).unwrap_or(0));
        }
        self.meta[i] = m;
    }

    fn put_overflow(&mut self, encoded: &str, seen: Seen) {
        if self.overflow.insert(encoded.into(), seen).is_none() {
            self.overflow_bytes += encoded.len().div_ceil(16) * 16 + 16;
        }
    }

    /// Geschätzte Größe `(Einträge, Bytes, Einträge der Überlaufmenge)` für die Speicherzähler.
    pub fn memory(&self) -> (u64, u64, u64) {
        let slot = std::mem::size_of::<(u64, Slot)>() + 1;
        // `capacity()` ist die nutzbare Kapazität (7/8 der Buckets)
        let table: usize = self
            .shards
            .iter()
            .map(|s| s.capacity().div_ceil(7) * 8 * slot)
            .sum();
        let overflow = self.overflow.capacity() * (std::mem::size_of::<(Box<str>, Seen)>() + 1)
            + self.overflow_bytes;
        (
            self.len() as u64,
            (table + self.meta.capacity() * 4 + overflow) as u64,
            self.overflow.len() as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::{F_IN_TABLES, F_QUEUED, F_RULE_OK};

    fn seen(id: i64, depth: u32, flags: u8) -> Seen {
        Seen {
            id: UrlId(id),
            depth,
            flags,
        }
    }

    /// Gleicher Schlüssel für alle Adressen mit `collide`, der Fingerabdruck bleibt echt.
    fn colliding(encoded: &str) -> KeyFp {
        let (key, fp) = default_key_fp(encoded);
        if encoded.contains("collide") {
            (0x1234_5678_9abc_def0, fp)
        } else {
            (key, fp)
        }
    }

    // spec: PERF-001
    #[test]
    fn entries_are_found_updated_and_counted() {
        let mut s = SeenSet::default();
        assert!(s.is_empty() && s.get("https://a.test/").is_none());
        s.insert("https://a.test/", seen(1, 0, F_QUEUED | F_RULE_OK));
        s.insert("https://a.test/x", seen(2, NO_DEPTH, F_IN_TABLES));
        assert_eq!(s.len(), 2);
        let a = s.get("https://a.test/").unwrap();
        assert_eq!((a.id.0, a.depth, a.flags), (1, 0, F_QUEUED | F_RULE_OK));
        let x = s.get("https://a.test/x").unwrap();
        assert_eq!((x.id.0, x.depth, x.flags), (2, NO_DEPTH, F_IN_TABLES));
        // Aktualisieren ändert die Zahl nicht
        s.insert("https://a.test/x", seen(2, 3, F_QUEUED | F_IN_TABLES));
        assert_eq!(s.len(), 2);
        let x = s.get("https://a.test/x").unwrap();
        assert_eq!((x.depth, x.flags), (3, F_QUEUED | F_IN_TABLES));
        assert!(s.get("https://a.test/y").is_none());
        assert_eq!(s.overflow_len(), 0);
    }

    // spec: PERF-001
    #[test]
    fn a_key_collision_with_another_fingerprint_keeps_both_urls() {
        let mut s = SeenSet::with_key_fn(colliding);
        s.insert("https://a.test/collide-1", seen(1, 1, F_QUEUED));
        s.insert("https://a.test/collide-2", seen(2, 2, F_QUEUED));
        s.insert("https://a.test/other", seen(3, 3, F_QUEUED));
        assert_eq!(s.len(), 3);
        assert_eq!(
            s.overflow_len(),
            1,
            "die zweite Adresse steht in der Überlaufmenge"
        );
        assert_eq!(s.get("https://a.test/collide-1").unwrap().id.0, 1);
        assert_eq!(s.get("https://a.test/collide-2").unwrap().id.0, 2);
        assert_eq!(s.get("https://a.test/other").unwrap().id.0, 3);
        // Aktualisieren trifft die richtige Adresse
        s.insert(
            "https://a.test/collide-2",
            seen(2, 0, F_QUEUED | F_IN_TABLES),
        );
        let a = s.get("https://a.test/collide-1").unwrap();
        let b = s.get("https://a.test/collide-2").unwrap();
        assert_eq!((a.depth, a.flags), (1, F_QUEUED));
        assert_eq!((b.depth, b.flags), (0, F_QUEUED | F_IN_TABLES));
        assert!(s.get("https://a.test/collide-3").is_none());
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn values_that_do_not_fit_the_compact_form_go_to_the_overflow() {
        let mut s = SeenSet::default();
        // Tiefe jenseits von 28 Bit, Kennung jenseits von 32 Bit, Kennung weit hinter dem Ende
        s.insert("https://a.test/deep", seen(1, 0x1000_0000, F_QUEUED));
        s.insert("https://a.test/huge-id", seen(1 << 40, 5, F_QUEUED));
        s.insert("https://a.test/sparse", seen(50_000_000, 5, F_QUEUED));
        s.insert("https://a.test/dense", seen(2, 5, F_QUEUED));
        assert_eq!(s.len(), 4);
        assert_eq!(s.overflow_len(), 3);
        assert_eq!(s.get("https://a.test/deep").unwrap().depth, 0x1000_0000);
        assert_eq!(s.get("https://a.test/huge-id").unwrap().id.0, 1 << 40);
        assert_eq!(s.get("https://a.test/sparse").unwrap().id.0, 50_000_000);
        assert_eq!(s.get("https://a.test/dense").unwrap().id.0, 2);
        // ein Eintrag der Haupttabelle, der später nicht mehr passt, wandert in die Überlaufmenge
        s.insert("https://a.test/dense", seen(2, 0x2000_0000, F_QUEUED));
        assert_eq!(s.overflow_len(), 4);
        assert_eq!(s.len(), 4);
        assert_eq!(s.get("https://a.test/dense").unwrap().depth, 0x2000_0000);
    }

    #[test]
    fn the_main_table_needs_16_bytes_per_entry_plus_the_control_byte() {
        let mut s = SeenSet::default();
        for i in 0..200_000i64 {
            s.insert(
                &format!("https://site.test/p/{i}"),
                seen(i + 1, 3, F_QUEUED),
            );
        }
        let (entries, bytes, overflow) = s.memory();
        assert_eq!((entries, overflow), (200_000, 0));
        // Tabelle: 17 Byte je Bucket bei höchstens 7/8 Füllung der Teiltabellen, dazu 4 Byte je Kennung
        let per_entry = bytes as f64 / entries as f64;
        assert!(
            per_entry < 17.0 / 0.4 + 4.0,
            "{per_entry} Byte je Eintrag (Tabellenreserve eingeschlossen)"
        );
        assert!(per_entry >= 16.0 + 4.0, "{per_entry}");
    }
}
