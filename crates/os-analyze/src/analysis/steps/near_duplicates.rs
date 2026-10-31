//! Bounded, parallel LSH over persisted signatures; exact Jaccard and atomic publication.
//! Larger populations fall back to the disk implementation instead of growing the cache.
mod disk;
use crate::{analysis::CancelToken, error::AnalyzeError, minhash};
use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

// At most 544 MiB of page records, 64 MiB of hash text, and 8 × 16 MiB of sort keys.
// Texts, shingle sets, candidate queues and SQLite results never grow with the population.
const MAX_PAGES: usize = 1_048_576;
const MAX_HASH_BYTES: usize = 64 * 1024 * 1024;
const MAX_WORKERS: usize = 8;
const PAIR_BATCH: usize = 256;
const SELECT_PAGES: &str = "SELECT u.id,m.sig,u.hash_md5 FROM urls u JOIN minhash m ON m.url_id=u.id WHERE u.is_internal=1 AND u.in_tables=1 AND u.status_code BETWEEN 200 AND 299 AND u.content_class='HTML' AND (?1=0 OR COALESCE(u.indexability,'') <> 'Non-Indexable') ORDER BY u.id";

struct Page {
    id: i64,
    sig: [u8; 512],
    hash: String,
}
enum Loaded {
    Pages(Vec<Page>),
    Disk,
    Cancelled,
}
enum Event {
    Pairs(Vec<(u32, u32)>),
    BandDone(Duration),
}

#[derive(Clone, Copy)]
enum Transaction {
    Collect,
    Publish,
}

/// TEMP collection may read MAIN, but must release that WAL snapshot before
/// acquiring the publication write transaction. Other analysis steps publish
/// concurrently on separate connections; upgrading an old snapshot yields 517.
struct Scratch<'a> {
    conn: &'a rusqlite::Connection,
    transaction: Option<Transaction>,
}
impl<'a> Scratch<'a> {
    fn new(conn: &'a rusqlite::Connection) -> Self {
        Self {
            conn,
            transaction: None,
        }
    }

    fn begin(&mut self, transaction: Transaction) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(match transaction {
            Transaction::Collect => "SAVEPOINT d02_work;",
            Transaction::Publish => "SAVEPOINT d02_publish;",
        })?;
        self.transaction = Some(transaction);
        Ok(())
    }

    fn release(&mut self) -> Result<(), rusqlite::Error> {
        if let Some(transaction) = self.transaction {
            self.conn.execute_batch(match transaction {
                Transaction::Collect => "RELEASE d02_work;",
                Transaction::Publish => "RELEASE d02_publish;",
            })?;
            self.transaction = None;
        }
        Ok(())
    }
}
impl Drop for Scratch<'_> {
    fn drop(&mut self) {
        if let Some(transaction) = self.transaction {
            let _ = self.conn.execute_batch(match transaction {
                Transaction::Collect => "ROLLBACK TO d02_work; RELEASE d02_work;",
                Transaction::Publish => "ROLLBACK TO d02_publish; RELEASE d02_publish;",
            });
        }
        let _ = self.conn.execute_batch(
            "DROP TABLE IF EXISTS temp.d02_bands; DROP TABLE IF EXISTS temp.d02_pairs;",
        );
    }
}

fn load(
    conn: &rusqlite::Connection,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    limit: usize,
    hash_limit: usize,
) -> Result<Loaded, AnalyzeError> {
    let total: usize = conn.query_row("SELECT COUNT(*) FROM minhash", [], |r| r.get(0))?;
    if total > limit {
        return Ok(Loaded::Disk);
    }
    let mut pages = Vec::with_capacity(total);
    let mut stmt = conn.prepare(SELECT_PAGES)?;
    let mut rows = stmt.query([cfg.duplicates.only_indexable])?;
    let mut hash_bytes = 0;
    while let Some(row) = rows.next()? {
        if cancel.checkpoint().is_err() {
            return Ok(Loaded::Cancelled);
        }
        let value = row.get_ref(1)?;
        let sig = value.as_blob().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e))
        })?;
        let Ok(sig) = <[u8; 512]>::try_from(sig) else {
            continue;
        };
        let hash: Option<String> = row.get(2)?;
        let hash = hash.unwrap_or_default();
        hash_bytes += hash.len();
        if hash_bytes > hash_limit {
            return Ok(Loaded::Disk);
        }
        pages.push(Page {
            id: row.get(0)?,
            sig,
            hash,
        });
    }
    Ok(Loaded::Pages(pages))
}

fn workers() -> usize {
    // No new dependency or global pool: a bounded scoped pool, also usable by CLI/tests.
    std::env::var("OPENSPIDER_NEAR_DUP_THREADS")
        .ok()
        .or_else(|| std::env::var("RAYON_NUM_THREADS").ok())
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from))
        .min(MAX_WORKERS)
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

/// Fingerprints only accelerate sorting. Full band bytes decide membership, so even
/// hash collisions cannot change the candidate set. Equal nonempty HTML hashes are
/// skipped as whole groups (linear work for arbitrarily many exact copies).
fn band_pairs(
    pages: &[Page],
    keys: &mut [(u64, u32)],
    band: usize,
    height: usize,
    cancel: &CancelToken,
    mut emit: impl FnMut(u32, u32) -> bool,
) -> bool {
    let start = band * height;
    let bytes = |index: u32| &pages[index as usize].sig[start..start + height];
    let mut group = 0;
    while group < keys.len() {
        if cancel.checkpoint().is_err() {
            return false;
        }
        let mut end = group + 1;
        while end < keys.len() && keys[end].0 == keys[group].0 {
            end += 1;
        }
        if end - group > 1 {
            let bucket = &mut keys[group..end];
            bucket.sort_unstable_by(|a, b| {
                bytes(a.1)
                    .cmp(bytes(b.1))
                    .then_with(|| pages[a.1 as usize].hash.cmp(&pages[b.1 as usize].hash))
                    .then_with(|| a.1.cmp(&b.1))
            });
            let mut first = 0;
            while first < bucket.len() {
                let mut last = first + 1;
                while last < bucket.len() && bytes(bucket[last].1) == bytes(bucket[first].1) {
                    last += 1;
                }
                let mut hash_first = first;
                while hash_first < last {
                    if cancel.checkpoint().is_err() {
                        return false;
                    }
                    let hash = &pages[bucket[hash_first].1 as usize].hash;
                    let mut hash_last = hash_first + 1;
                    while hash_last < last && pages[bucket[hash_last].1 as usize].hash == *hash {
                        hash_last += 1;
                    }
                    for a in hash_first..hash_last {
                        let from = if hash.is_empty() { a + 1 } else { hash_last };
                        for b in from..last {
                            if cancel.checkpoint().is_err() {
                                return false;
                            }
                            let (a, b) =
                                (bucket[a].1.min(bucket[b].1), bucket[a].1.max(bucket[b].1));
                            // Emit at the FIRST shared band only: exact deduplication without
                            // an unbounded set or repeated SQL/shingling for the same pair.
                            if !(0..band).any(|previous| {
                                let offset = previous * height;
                                pages[a as usize].sig[offset..offset + height]
                                    == pages[b as usize].sig[offset..offset + height]
                            }) && !emit(a, b)
                            {
                                return false;
                            }
                        }
                    }
                    hash_first = hash_last;
                }
                first = last;
            }
        }
        group = end;
    }
    true
}

pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    run_with_limits(file, cfg, cancel, MAX_PAGES, MAX_HASH_BYTES)
}

fn run_with_limits(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    page_limit: usize,
    hash_limit: usize,
) -> Result<StepState, AnalyzeError> {
    if !cfg.duplicates.near_enabled {
        return Ok(StepState::NotApplicable);
    }
    cancel.set_progress(0.0);
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    let profile = std::env::var_os("OPENSPIDER_NEAR_DUP_PROFILE").is_some();
    let started = Instant::now();
    let conn = file.conn();
    let pages = match load(conn, cfg, cancel, page_limit, hash_limit)? {
        Loaded::Pages(pages) => pages,
        Loaded::Disk => return disk::run(file, cfg, cancel),
        Loaded::Cancelled => return Ok(StepState::Pending),
    };
    let load_time = started.elapsed();
    cancel.set_progress(0.2);
    conn.execute_batch("DROP TABLE IF EXISTS temp.d02_pairs; CREATE TEMP TABLE d02_pairs(url_id INTEGER,other_id INTEGER,similarity REAL,PRIMARY KEY(url_id,other_id)) WITHOUT ROWID;")?;
    let mut scratch = Scratch::new(conn);
    scratch.begin(Transaction::Collect)?;
    let height = minhash::band_rows(cfg.duplicates.near_threshold_pct) * 4;
    let bands = 512 / height;
    let threads = workers().min(bands);
    let next = AtomicUsize::new(0);
    let mut band_time = Duration::ZERO;
    let mut read_time = Duration::ZERO;
    let mut shingle_time = Duration::ZERO;
    let mut compare_time = Duration::ZERO;
    let mut write_time = Duration::ZERO;
    let mut candidates = 0_u64;
    let mut completed = 0;
    let lsh_start = Instant::now();
    let result = std::thread::scope(|scope| -> Result<StepState, AnalyzeError> {
        let (tx, rx) = mpsc::sync_channel(threads * 2);
        for _ in 0..threads {
            let (tx, pages, next) = (tx.clone(), &pages, &next);
            scope.spawn(move || {
                let mut keys = Vec::with_capacity(pages.len());
                loop {
                    let band = next.fetch_add(1, Ordering::Relaxed);
                    if band >= bands || cancel.checkpoint().is_err() {
                        break;
                    }
                    let timer = Instant::now();
                    keys.clear();
                    for (chunk, part) in pages.chunks(1024).enumerate() {
                        if cancel.checkpoint().is_err() {
                            return;
                        }
                        keys.extend(part.iter().enumerate().map(|(i, page)| {
                            (
                                fingerprint(&page.sig[band * height..(band + 1) * height]),
                                (chunk * 1024 + i) as u32,
                            )
                        }));
                    }
                    keys.sort_unstable();
                    let elapsed = timer.elapsed();
                    let mut batch = Vec::with_capacity(PAIR_BATCH);
                    if !band_pairs(pages, &mut keys, band, height, cancel, |a, b| {
                        batch.push((a, b));
                        if batch.len() == PAIR_BATCH {
                            tx.send(Event::Pairs(std::mem::replace(
                                &mut batch,
                                Vec::with_capacity(PAIR_BATCH),
                            )))
                            .is_ok()
                        } else {
                            true
                        }
                    }) {
                        return;
                    }
                    if !batch.is_empty() && tx.send(Event::Pairs(batch)).is_err() {
                        return;
                    }
                    if tx.send(Event::BandDone(elapsed)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(tx);
        let mut text = conn
            .prepare_cached("SELECT COALESCE((SELECT v_text FROM url_ext WHERE url_id=?1 AND key='content.text'),'')")?;
        let mut insert = conn.prepare("INSERT INTO temp.d02_pairs VALUES (?1,?2,?3),(?2,?1,?3)")?;
        for event in rx {
            if cancel.checkpoint().is_err() {
                return Ok(StepState::Pending);
            }
            match event {
                Event::BandDone(elapsed) => {
                    band_time += elapsed;
                    completed += 1;
                    cancel.set_progress(0.2 + 0.7 * completed as f32 / bands as f32);
                }
                Event::Pairs(batch) => {
                    for (a, b) in batch {
                        if cancel.checkpoint().is_err() {
                            return Ok(StepState::Pending);
                        }
                        candidates += 1;
                        let (a, b) = (&pages[a as usize], &pages[b as usize]);
                        let timer = Instant::now();
                        let left: String = text.query_row([a.id], |r| r.get(0))?;
                        let right: String =
                            text.query_row([b.id], |r| r.get(0)).unwrap_or_default();
                        if profile {
                            read_time += timer.elapsed();
                        }
                        let timer = Instant::now();
                        let (left, right) = (minhash::shingles(&left), minhash::shingles(&right));
                        if profile {
                            shingle_time += timer.elapsed();
                        }
                        let timer = Instant::now();
                        let similarity = minhash::jaccard(&left, &right);
                        if profile {
                            compare_time += timer.elapsed();
                        }
                        if similarity * 100.0 + 1e-10
                            >= f64::from(cfg.duplicates.near_threshold_pct)
                        {
                            let timer = Instant::now();
                            insert.execute(rusqlite::params![a.id, b.id, similarity])?;
                            if profile {
                                write_time += timer.elapsed();
                            }
                        }
                    }
                }
            }
        }
        Ok(if completed == bands && cancel.checkpoint().is_ok() {
            StepState::Done
        } else {
            StepState::Pending
        })
    })?;
    if result != StepState::Done {
        return Ok(result);
    }
    let lsh_time = lsh_start.elapsed();
    cancel.set_progress(0.95);
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    let timer = Instant::now();
    scratch.release()?;
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    scratch.begin(Transaction::Publish)?;
    conn.execute_batch(
        "DELETE FROM near_duplicates; INSERT INTO near_duplicates SELECT * FROM temp.d02_pairs;",
    )?;
    scratch.release()?;
    write_time += timer.elapsed();
    if profile {
        eprintln!("n16 pages={} threads={threads} total={:?} load={load_time:?} lsh_wall={lsh_time:?} band_hash_sort_worker_sum={band_time:?} candidate_read={read_time:?} shingling={shingle_time:?} compare={compare_time:?} write_publish={write_time:?} candidates={candidates}; persisted MinHash/SimHash unchanged", pages.len(), started.elapsed());
    }
    cancel.set_progress(1.0);
    Ok(StepState::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: ONPAGE-144
    #[test]
    fn n16_fingerprint_collisions_and_repeated_bands_preserve_exact_candidate_set() {
        let mut pages: Vec<_> = ["a", "a", "b", "", "", "c"]
            .into_iter()
            .enumerate()
            .map(|(id, hash)| Page {
                id: id as i64,
                sig: [0; 512],
                hash: hash.into(),
            })
            .collect();
        pages[4].sig[0] = 1;
        pages[5].sig.fill(2);
        for height in [4, 8, 16, 32, 64, 128, 256, 512] {
            let mut actual = Vec::new();
            for band in 0..512 / height {
                // Deliberately force every fingerprint to collide, even unrelated signatures.
                let mut keys = (0..pages.len() as u32).map(|i| (0, i)).collect::<Vec<_>>();
                assert!(band_pairs(
                    &pages,
                    &mut keys,
                    band,
                    height,
                    &CancelToken::new(),
                    |a, b| {
                        actual.push((a, b));
                        true
                    }
                ));
            }
            actual.sort_unstable();
            let mut expected = Vec::new();
            for a in 0..pages.len() {
                for b in a + 1..pages.len() {
                    if !pages[a].hash.is_empty() && pages[a].hash == pages[b].hash {
                        continue;
                    }
                    if pages[a]
                        .sig
                        .chunks(height)
                        .zip(pages[b].sig.chunks(height))
                        .any(|(a, b)| a == b)
                    {
                        expected.push((a as u32, b as u32));
                    }
                }
            }
            assert_eq!(actual, expected, "height={height}");
        }
    }

    // spec: PERF-007
    #[test]
    fn n16_large_exact_bucket_does_not_enumerate_pairs_and_honors_cancellation() {
        let pages = (0..10_000)
            .map(|id| Page {
                id,
                sig: [0; 512],
                hash: "same".into(),
            })
            .collect::<Vec<_>>();
        let mut keys = (0..pages.len() as u32).map(|i| (0, i)).collect::<Vec<_>>();
        let token = CancelToken::new();
        assert!(band_pairs(&pages, &mut keys, 0, 32, &token, |_, _| panic!(
            "exact copies are not candidates"
        )));
        token.cancel();
        assert!(!band_pairs(
            &pages,
            &mut keys,
            0,
            32,
            &token,
            |_, _| unreachable!()
        ));
    }

    // spec: PERF-007
    #[test]
    fn n16_population_and_hash_byte_limits_choose_disk_fallback() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE urls(id INTEGER PRIMARY KEY,hash_md5 TEXT,is_internal INTEGER,in_tables INTEGER,status_code INTEGER,content_class TEXT,indexability TEXT); CREATE TABLE minhash(url_id INTEGER PRIMARY KEY,sig BLOB); INSERT INTO urls VALUES(1,'hash',1,1,200,'HTML','Indexable'); INSERT INTO minhash VALUES(1,zeroblob(512));").unwrap();
        let cfg = CrawlConfig::default();
        let token = CancelToken::new();
        assert!(matches!(
            load(&conn, &cfg, &token, 0, 100).unwrap(),
            Loaded::Disk
        ));
        assert!(matches!(
            load(&conn, &cfg, &token, 1, 3).unwrap(),
            Loaded::Disk
        ));
        assert!(
            matches!(load(&conn, &cfg, &token, 1, 4).unwrap(), Loaded::Pages(p) if p.len() == 1)
        );
        token.cancel();
        assert!(matches!(
            load(&conn, &cfg, &token, 1, 4).unwrap(),
            Loaded::Cancelled
        ));
    }
}
