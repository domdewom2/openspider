//! Deterministische englische Flesch-Heuristik und MinHash (ONPAGE-149/144).
use crate::PageCtx;
use os_model::{ExtKey, ParsedPage, Value};

/// Silbengruppen nach ONPAGE-149, ohne sprachabhängige Wörterbücher.
pub fn syllables(word: &str) -> u32 {
    let letters: Vec<char> = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect();
    let vowel = |c| "aeiouyäöü".contains(c);
    let mut count = 0;
    let mut previous = false;
    for &c in &letters {
        let current = vowel(c);
        if current && !previous {
            count += 1;
        }
        previous = current;
    }
    let consonant_le = letters.len() >= 3
        && letters[letters.len() - 2] == 'l'
        && !vowel(letters[letters.len() - 3]);
    if letters.last() == Some(&'e') && !consonant_le && count > 1 {
        count -= 1;
    }
    count.max(1)
}

/// Labels aus der versionierten Banddatei (A-54).
pub fn label(score: f64) -> &'static str {
    include_str!("../../../core/readability/readability_bands.toml")
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(minimum, label)| {
            (score >= minimum.trim().parse::<f64>().ok()?).then(|| label.trim().trim_matches('"'))
        })
        .unwrap_or("Very Difficult")
}

/// Satzgrenzen sind Interpunktion vor Leerraum/Textende und Blockgrenzen (`\\n`).
pub fn facts(text: &str) -> Vec<(ExtKey, Value)> {
    let mut words = 0u32;
    let mut syllable_count = 0u32;
    let mut sentences = 0u32;
    let mut pending = false;
    for block in text.split('\n') {
        for token in block.split_whitespace() {
            if token.chars().any(char::is_alphabetic) {
                words += 1;
                syllable_count += syllables(token);
                pending = true;
            }
            if token.ends_with(['.', '!', '?', '…']) && pending {
                sentences += 1;
                pending = false;
            }
        }
        if pending {
            sentences += 1;
            pending = false;
        }
    }
    if words == 0 {
        return Vec::new();
    }
    let average = f64::from(words) / f64::from(sentences.max(1));
    let score = ((206.835 - 1.015 * average - 84.6 * f64::from(syllable_count) / f64::from(words))
        .clamp(0.0, 100.0)
        * 10.0)
        .round()
        / 10.0;
    vec![
        (
            ExtKey::from_static("readability.sentences"),
            Value::Int(sentences.into()),
        ),
        (
            ExtKey::from_static("readability.average"),
            Value::Float((average * 10.0).round() / 10.0),
        ),
        (
            ExtKey::from_static("readability.flesch"),
            Value::Float(score),
        ),
        (
            ExtKey::from_static("readability.label"),
            Value::Text(label(score).to_owned()),
        ),
    ]
}

pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let text = crate::content::area_text(ctx.doc, &ctx.cfg.content_area);
    if ctx.cfg.readability {
        page.ext.extend(facts(&text));
    }
    if ctx.cfg.minhash {
        page.minhash = Some(minhash(&text));
    }
}

/// 128 feste Hashfunktionen über normalisierte Dreier-Shingles.
pub fn minhash(text: &str) -> Vec<u32> {
    let normalized: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let words: Vec<_> = normalized.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let n = words.len().min(3);
    let hashes: Vec<u64> = words
        .windows(n)
        .map(|w| {
            w.join(" ").bytes().fold(0xcbf29ce484222325, |h, b| {
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
