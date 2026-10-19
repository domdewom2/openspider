//! URL-Umschreibung bei Entdeckung (CRAWL-103/104), vor der Normalisierung.
//!
//! Parameternamen werden vor dem exakten, case-sensitiven Vergleich nach
//! `application/x-www-form-urlencoded` dekodiert (`%73id` = `sid`, `+` = Leerzeichen).
//! Die Kodierung der verbleibenden Parameter und das Fragment bleiben unverändert.
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use fancy_regex::{Regex, RegexBuilder};

pub struct Rewritten<'a> {
    pub url: Cow<'a, str>,
    pub warning: Option<String>,
}

pub(crate) fn compile(pattern: &str) -> Result<Regex, fancy_regex::Error> {
    RegexBuilder::new(pattern)
        .backtrack_limit(1_000_000)
        .build()
}

const REGEX_CACHE_CAPACITY: usize = 128;

#[derive(Default)]
struct RegexCache {
    // Least recently used first; bounded across configuration changes.
    entries: VecDeque<(String, Arc<Regex>)>,
}

impl RegexCache {
    fn get(&mut self, pattern: &str) -> Option<Arc<Regex>> {
        let index = self.entries.iter().position(|(key, _)| key == pattern)?;
        let entry = self.entries.remove(index)?;
        let regex = Arc::clone(&entry.1);
        self.entries.push_back(entry);
        Some(regex)
    }

    fn insert(&mut self, pattern: &str, regex: Arc<Regex>) -> Arc<Regex> {
        // Another discovery thread may have compiled the same pattern meanwhile.
        if let Some(existing) = self.get(pattern) {
            return existing;
        }
        if self.entries.len() == REGEX_CACHE_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((pattern.into(), Arc::clone(&regex)));
        regex
    }
}

// Discovery must not compile a regex for every link.
fn cached_regex(pattern: &str) -> Result<Arc<Regex>, fancy_regex::Error> {
    static CACHE: OnceLock<Mutex<RegexCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(RegexCache::default()));
    if let Some(regex) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pattern)
    {
        return Ok(regex);
    }
    let regex = Arc::new(compile(pattern)?);
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(cache.insert(pattern, regex))
}

/// Entfernt Parameter, danach werden die Regex-Regeln der Reihe nach angewendet.
/// Bei Fehlern bleibt die gesamte ursprüngliche URL erhalten.
pub fn apply_discovered<'a>(cfg: &os_model::RewriteConfig, raw_resolved: &'a str) -> Rewritten<'a> {
    if !cfg.remove_all_parameters
        && cfg.remove_parameters.is_empty()
        && cfg.regex_replace.is_empty()
    {
        return Rewritten {
            url: Cow::Borrowed(raw_resolved),
            warning: None,
        };
    }
    let apply = || -> Result<String, String> {
        let mut result = raw_resolved.to_string();
        let fragment = result.find('#').unwrap_or(result.len());
        if let Some(query) = result[..fragment].find('?') {
            if cfg.remove_all_parameters {
                result.replace_range(query..fragment, "");
            } else if !cfg.remove_parameters.is_empty() {
                // Retain the original encoding, order, duplicates and empty values of survivors.
                let remaining: Vec<_> = result[query + 1..fragment]
                    .split('&')
                    .filter(|pair| {
                        let name = pair.split_once('=').map_or(*pair, |(name, _)| name);
                        let decoded = url::form_urlencoded::parse(format!("{name}=").as_bytes())
                            .next()
                            .map(|(name, _)| name.into_owned())
                            .unwrap_or_default();
                        !cfg.remove_parameters.contains(&decoded)
                    })
                    .collect();
                let replacement = if remaining.is_empty() {
                    String::new()
                } else {
                    format!("?{}", remaining.join("&"))
                };
                result.replace_range(query..fragment, &replacement);
            }
        }
        for (index, rule) in cfg.regex_replace.iter().enumerate() {
            let regex =
                cached_regex(&rule.pattern).map_err(|e| format!("Regex {}: {e}", index + 1))?;
            result = regex
                .try_replacen(&result, 0, rule.replacement.as_str())
                .map_err(|e| format!("Regex {}: {e}", index + 1))?
                .into_owned();
        }
        let parsed = url::Url::parse(&result)
            .map_err(|e| format!("Ungültiges Umschreibungsergebnis: {e}"))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err("Umschreibungsergebnis muss eine absolute HTTP(S)-URL sein".into());
        }
        Ok(result)
    };
    match apply() {
        Ok(result) => Rewritten {
            url: if result == raw_resolved {
                Cow::Borrowed(raw_resolved)
            } else {
                Cow::Owned(result)
            },
            warning: None,
        },
        Err(warning) => Rewritten {
            url: Cow::Borrowed(raw_resolved),
            warning: Some(warning),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: CRAWL-104
    #[test]
    fn regex_cache_evicts_only_the_least_recently_used_pattern() {
        let mut cache = RegexCache::default();
        let first = cache.insert("first", Arc::new(compile("first").unwrap()));
        for index in 1..REGEX_CACHE_CAPACITY {
            let pattern = format!("pattern-{index}");
            cache.insert(&pattern, Arc::new(compile(&pattern).unwrap()));
        }
        assert!(Arc::ptr_eq(&cache.get("first").unwrap(), &first));
        cache.insert("overflow", Arc::new(compile("overflow").unwrap()));
        assert_eq!(cache.entries.len(), REGEX_CACHE_CAPACITY);
        assert!(cache.get("pattern-1").is_none());
        assert!(cache.get("pattern-2").is_some());
        assert!(cache.get("overflow").is_some());
        assert!(Arc::ptr_eq(&cache.get("first").unwrap(), &first));

        let duplicate = cache.insert("first", Arc::new(compile("first").unwrap()));
        assert!(Arc::ptr_eq(&duplicate, &first));
        assert_eq!(cache.entries.len(), REGEX_CACHE_CAPACITY);
    }
}
