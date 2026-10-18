//! URL-Umschreibung bei Entdeckung (CRAWL-103/104), vor der Normalisierung.
use std::borrow::Cow;
use std::collections::HashMap;
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

// Bounded across configuration changes; discovery must not compile a regex for every link.
fn cached_regex(pattern: &str) -> Result<Arc<Regex>, fancy_regex::Error> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Regex>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(regex) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pattern)
        .cloned()
    {
        return Ok(regex);
    }
    let regex = Arc::new(compile(pattern)?);
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(pattern.into(), Arc::clone(&regex));
    Ok(regex)
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
        Err(warning) => {
            eprintln!("OpenSpider URL-Umschreibung: {warning}; Original-URL bleibt erhalten");
            Rewritten {
                url: Cow::Borrowed(raw_resolved),
                warning: Some(warning),
            }
        }
    }
}
