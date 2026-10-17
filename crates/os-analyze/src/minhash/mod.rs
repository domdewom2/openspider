//! Deterministisches Shingling, MinHash und schwellwertabhängiges LSH.
use std::collections::BTreeSet;

pub fn shingles(text: &str) -> BTreeSet<String> {
    let normalized: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let words: Vec<_> = normalized.split_whitespace().collect();
    if words.is_empty() {
        return BTreeSet::new();
    }
    words
        .windows(words.len().min(3))
        .map(|w| w.join(" "))
        .collect()
}

pub fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    intersection as f64 / (a.len() + b.len() - intersection) as f64
}

pub fn signature(shingles: &BTreeSet<String>) -> Vec<u32> {
    let hashes: Vec<u64> = shingles
        .iter()
        .map(|w| {
            w.bytes().fold(0xcbf29ce484222325, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            })
        })
        .collect();
    (0..128u64)
        .map(|seed| {
            hashes
                .iter()
                .map(|h| {
                    let mut x = h.wrapping_add(seed.wrapping_mul(0x9e3779b97f4a7c15));
                    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
                    (x ^ (x >> 31)) as u32
                })
                .min()
                .unwrap_or(u32::MAX)
        })
        .collect()
}

/// Größtmögliche Bandhöhe mit Kandidatenwahrscheinlichkeit ≥ 0,999.
pub fn band_rows(threshold: u8) -> usize {
    let s = f64::from(threshold) / 100.0;
    [128, 64, 32, 16, 8, 4, 2, 1]
        .into_iter()
        .find(|r| 1.0 - (1.0 - s.powi(*r as i32)).powi((128 / r) as i32) >= 0.999)
        .unwrap_or(1)
}

/// Myers-Diff: Wortpositionen gelöschter/eingefügter Token, auch bei Wiederholungen.
pub fn word_diff(left: &str, right: &str) -> (Vec<usize>, Vec<usize>) {
    let a: Vec<_> = left.split_whitespace().collect();
    let b: Vec<_> = right.split_whitespace().collect();
    let max = a.len() + b.len();
    let offset = max as isize + 1;
    let mut v = vec![0isize; 2 * max + 3];
    let mut trace = Vec::new();
    let mut distance = 0;
    'outer: for d in 0..=max {
        trace.push(v.clone());
        for k in (-(d as isize)..=d as isize).step_by(2) {
            let i = (offset + k) as usize;
            let mut x = if k == -(d as isize) || (k != d as isize && v[i - 1] < v[i + 1]) {
                v[i + 1]
            } else {
                v[i - 1] + 1
            };
            let mut y = x - k;
            while x < a.len() as isize && y < b.len() as isize && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[i] = x;
            if x == a.len() as isize && y == b.len() as isize {
                distance = d;
                break 'outer;
            }
        }
    }
    let (mut x, mut y) = (a.len() as isize, b.len() as isize);
    let (mut deleted, mut inserted) = (Vec::new(), Vec::new());
    for d in (1..=distance).rev() {
        let previous = &trace[d];
        let k = x - y;
        let i = (offset + k) as usize;
        let pk = if k == -(d as isize) || (k != d as isize && previous[i - 1] < previous[i + 1]) {
            k + 1
        } else {
            k - 1
        };
        let px = previous[(offset + pk) as usize];
        let py = px - pk;
        while x > px && y > py {
            x -= 1;
            y -= 1;
        }
        if x == px {
            y -= 1;
            inserted.push(y as usize);
        } else {
            x -= 1;
            deleted.push(x as usize);
        }
    }
    deleted.reverse();
    inserted.reverse();
    (deleted, inserted)
}
