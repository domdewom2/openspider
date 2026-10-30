//! Deterministische englische Flesch-Heuristik und MinHash (ONPAGE-149/144).
use crate::PageCtx;
use os_model::{ExtKey, ParsedPage, Value};

/// Silbengruppen nach ONPAGE-149, ohne sprachabhängige Wörterbücher.
pub fn syllables(word: &str) -> u32 {
    syllable_groups(word).max(1)
}

// Preserve separators: merging letters across '-' or '&' changes vowel groups in URLs.
// The calibrated score uses the raw group count (silent "the" contributes zero).
fn syllable_groups(word: &str) -> u32 {
    let lower = word.to_lowercase();
    let mut stem = lower.as_str();
    let vowel = |c: char| "aeiouy".contains(c);
    let mut silent_e = false;
    if stem.ends_with("ed")
        || (stem.ends_with("es")
            && stem
                .chars()
                .rev()
                .nth(2)
                .is_some_and(|c| !vowel(c) && !matches!(c, 'c' | 'g' | 's' | 'x' | 'z'))
            && !stem.ends_with("ches")
            && !stem.ends_with("shes"))
    {
        stem = &stem[..stem.len() - 2];
    } else if stem.ends_with('e') {
        let consonant_le =
            stem.ends_with("le") && stem.chars().rev().nth(2).is_some_and(|c| !vowel(c));
        if !consonant_le && !stem.ends_with("ee") {
            silent_e = true;
        }
    }
    let mut count = 0u32;
    let mut previous = false;
    for c in stem.chars() {
        let current = vowel(c);
        if current && !previous {
            count += 1;
        }
        previous = current;
    }
    let chars: Vec<char> = stem.chars().collect();
    for (i, pair) in chars.windows(2).enumerate() {
        if matches!(pair, ['i', 'a' | 'o']) && (i == 0 || !matches!(chars[i - 1], 'c' | 's' | 't'))
        {
            count += 1;
        }
    }
    if silent_e {
        count = count.saturating_sub(1);
    }
    count
}

/// Labels aus der versionierten Banddatei (A-54).
pub fn label(score: f64) -> &'static str {
    include_str!("../../../core/readability/readability_bands.toml")
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(minimum, label)| {
            (score >= minimum.trim().parse::<f64>().ok()?).then(|| label.trim().trim_matches('"'))
        })
        .unwrap_or("Very Hard")
}

/// Satzgrenzen: Interpunktion, Blockgrenzen und Wortumbruch vor 80 Unicode-Zeichen (Q02).
pub fn facts(text: &str) -> Vec<(ExtKey, Value)> {
    facts_with_words(text, None)
}

fn facts_with_words(text: &str, word_count: Option<u32>) -> Vec<(ExtKey, Value)> {
    if !text.chars().any(char::is_alphabetic) {
        return Vec::new();
    }
    let syllable_count: u32 = text.split_whitespace().map(syllable_groups).sum();
    let sentences: u32 = text.lines().map(sentence_segments).sum();
    let words = word_count.unwrap_or_else(|| text.split_whitespace().count() as u32);
    let average = f64::from(words) / f64::from(sentences.max(1));
    let score = ((206.835 - 1.015 * average - 84.6 * f64::from(syllable_count) / f64::from(words))
        .clamp(0.0, 100.0)
        * 1000.0)
        .round()
        / 1000.0;
    vec![
        (
            ExtKey::from_static("readability.sentences"),
            Value::Int(sentences.into()),
        ),
        (
            ExtKey::from_static("readability.average"),
            Value::Float((average * 1000.0).round() / 1000.0),
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

// Nonempty sentence fragments, including URL fragments; decimal points stay in their number.
fn sentence_segments(line: &str) -> u32 {
    let chars: Vec<char> = line.chars().collect();
    let mut count = 0;
    let mut fragment = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let decimal = c == '.'
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        let initialism = c == '.'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && (i == 1 || chars[i - 2].is_whitespace() || chars[i - 2] == '.')
            && chars.get(i + 1).is_some_and(|c| c.is_alphabetic());
        if matches!(c, '.' | '!' | '?' | '…' | ':') && !decimal && !initialism {
            count += wrapped_lines(&fragment);
            fragment.clear();
        } else {
            fragment.push(c);
        }
    }
    count + wrapped_lines(&fragment)
}

// Sentence fragments wrap before reaching 80 Unicode characters; punctuation is already removed.
fn wrapped_lines(fragment: &str) -> u32 {
    let mut count = 0;
    let mut width = 0;
    for token in fragment.split_whitespace() {
        let size = token.chars().count();
        if width > 0 && width + 1 + size >= 80 {
            width = 0;
        }
        if width == 0 {
            count += 1;
        }
        width += size + usize::from(width > 0);
    }
    count
}

pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let text = crate::content::area_text(ctx.doc, &ctx.cfg.content_area);
    if ctx.cfg.readability {
        page.ext.extend(facts_with_words(&text, page.word_count));
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
