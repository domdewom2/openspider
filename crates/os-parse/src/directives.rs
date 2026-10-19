//! Robots-Direktiven aus `<meta name=robots|<token>>` und `X-Robots-Tag` (TECH-050 ff.).
//!
//! # Token
//!
//! Ein Wert wird an Kommas getrennt, jedes Stück getrimmt und kleingeschrieben: `NoIndex , FOLLOW` ergibt
//! `noindex`, `follow`. Wertbehaftete Direktiven (`max-snippet`, `max-image-preview`, `max-video-preview`,
//! `unavailable_after`) werden als `name:wert` gespeichert (`max-snippet: 50` → `max-snippet:50`). Das Datum von
//! `unavailable_after` darf Kommas enthalten (`Thursday, 31-Dec-26 …`): Folgestücke, die mit einer Ziffer
//! beginnen, gehören noch zum Datum.
//!
//! # Agenten
//!
//! `<meta name="robots">` ist generisch (`agent = None`); `<meta name="googlebot">` und andere bekannte
//! Crawler-Namen sowie die übergebenen Token binden die Angabe an diesen Agenten. Im `X-Robots-Tag` bindet das
//! Präfix `<agent>:` die folgenden Direktiven an den Agenten, bis ein weiteres Präfix folgt
//! (`googlebot: noindex, otherbot: nofollow` ergibt zwei Directives). `Directive::raw` ist der Rohwert des
//! `content`-Attributs bzw. – bei mehreren Agenten in einem Header – der Abschnitt dieses Agenten samt Präfix.
//! `applies` gilt für generische Angaben und für Agenten, die zu einem der übergebenen Token passen.
//!
//! Meta-Elemente ohne `content` oder mit leerem Inhalt, solche in `<template>` und Kommentare liefern nichts;
//! `in_head` entscheidet der Tree-Builder (TECH-003), auch Meta-Robots im Body werden erfasst (TECH-065).

use std::sync::OnceLock;

use os_model::{Directive, DirectiveSource};
use scraper::Selector;

use crate::dom::{self, Document};

/// Wertbehaftete Direktiven (Name vor dem Doppelpunkt).
const VALUED: &[&str] = &[
    "max-snippet",
    "max-image-preview",
    "max-video-preview",
    "unavailable_after",
];

/// Bekannte Crawler-Namen, die als `<meta name=…>` angezeigt werden (auch wenn sie nicht zum Token passen).
const KNOWN_AGENTS: &[&str] = &[
    "googlebot",
    "googlebot-news",
    "googlebot-image",
    "googlebot-video",
    "google",
    "bingbot",
    "msnbot",
    "adidxbot",
    "bingpreview",
    "slurp",
    "duckduckbot",
    "baiduspider",
    "yandex",
    "yandexbot",
    "applebot",
    "facebookexternalhit",
    "twitterbot",
    "linkedinbot",
    "gptbot",
    "claudebot",
    "ccbot",
    "petalbot",
    "seznambot",
    "naverbot",
    "yeti",
    "bytespider",
    "otherbot",
];

/// Ein Abschnitt eines Direktivenwerts, der zu genau einem Agenten gehört.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    /// Agent (kleingeschrieben); `None` = generisch.
    pub agent: Option<String>,
    /// Normalisierte Token.
    pub tokens: Vec<String>,
    /// Rohtext des Abschnitts.
    pub raw: String,
}

/// Noch offener Abschnitt während des Zerlegens.
struct Pending {
    agent: Option<String>,
    tokens: Vec<String>,
    start: usize,
    end: usize,
}

impl Pending {
    fn new(agent: Option<String>, start: usize, end: usize) -> Self {
        Pending {
            agent,
            tokens: Vec::new(),
            start,
            end,
        }
    }
}

fn is_agent_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Zerlegt einen Direktivenwert in Abschnitte je Agent (siehe Moduldoku). Mit `allow_agents = false` (Meta)
/// wird kein Agentenpräfix erkannt.
pub(crate) fn parse_value(value: &str, allow_agents: bool) -> Vec<Group> {
    // Stücke samt Byte-Bereich im Original
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    for (i, c) in value.char_indices() {
        if c == ',' {
            pieces.push((start, i));
            start = i + 1;
        }
    }
    pieces.push((start, value.len()));

    let mut groups: Vec<Group> = Vec::new();
    let mut current: Option<Pending> = None;
    let flush = |current: &mut Option<Pending>, groups: &mut Vec<Group>| {
        if let Some(p) = current.take() {
            if !p.tokens.is_empty() {
                groups.push(Group {
                    agent: p.agent,
                    tokens: p.tokens,
                    raw: value[p.start..p.end].trim().to_string(),
                });
            }
        }
    };

    let mut i = 0usize;
    while i < pieces.len() {
        let (ps, pe) = pieces[i];
        let mut end = pe;
        let mut piece = value[ps..pe].trim();
        i += 1;
        if piece.is_empty() {
            continue;
        }

        // Agentenpräfix: `<agent>: <direktive>`
        if allow_agents {
            if let Some((head, rest)) = piece.split_once(':') {
                let name = head.trim().to_ascii_lowercase();
                if !VALUED.contains(&name.as_str()) && is_agent_name(&name) {
                    flush(&mut current, &mut groups);
                    current = Some(Pending::new(Some(name), ps, pe));
                    piece = rest.trim();
                    if piece.is_empty() {
                        continue;
                    }
                }
            }
        }
        if current.is_none() {
            current = Some(Pending::new(None, ps, pe));
        }

        // Direktive, ggf. wertbehaftet
        let token = match piece.split_once(':') {
            Some((name, val)) => {
                let name = name.trim().to_ascii_lowercase();
                let mut val = collapse_ws(val);
                if name == "unavailable_after" {
                    while i < pieces.len() {
                        let (ns, ne) = pieces[i];
                        let next = value[ns..ne].trim();
                        if next.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                            val.push(',');
                            val.push_str(&collapse_ws(next));
                            end = ne;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
                format!("{name}:{}", val.to_ascii_lowercase())
            }
            None => piece.to_ascii_lowercase(),
        };
        if let Some(cur) = current.as_mut() {
            cur.tokens.push(token);
            cur.end = end;
        }
    }
    flush(&mut current, &mut groups);
    groups
}

/// Passt der Agent zu einem der Token (ohne Beachtung der Schreibweise)?
pub(crate) fn agent_applies(agent: &Option<String>, tokens: &[String]) -> bool {
    match agent {
        None => true,
        Some(a) => tokens.iter().any(|t| t.eq_ignore_ascii_case(a)),
    }
}

/// Extrahiert die Direktiven aus den Meta-Elementen für die angegebenen Agent-Token.
pub fn extract(doc: &Document, tokens: &[String]) -> Vec<Directive> {
    static SEL: OnceLock<Selector> = OnceLock::new();
    let sel =
        SEL.get_or_init(|| Selector::parse("meta[name][content]").expect("gültiger Selektor"));
    let mut out = Vec::new();
    for el in doc.html().select(sel) {
        if !crate::links::is_html(el) || dom::in_template(el) {
            continue;
        }
        let name = el
            .value()
            .attr("name")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let agent = if name == "robots" {
            None
        } else if tokens.iter().any(|t| t.eq_ignore_ascii_case(&name))
            || KNOWN_AGENTS.contains(&name.as_str())
        {
            Some(name)
        } else {
            continue;
        };
        let content = el.value().attr("content").unwrap_or_default();
        let parsed: Vec<String> = parse_value(content, false)
            .into_iter()
            .flat_map(|g| g.tokens)
            .collect();
        if parsed.is_empty() {
            continue;
        }
        out.push(Directive {
            source: DirectiveSource::MetaRobots,
            applies: agent_applies(&agent, tokens),
            agent,
            raw: content.trim().to_string(),
            tokens: parsed,
            in_head: dom::in_head(el),
            order: out.len() as u32,
        });
    }
    out
}

/// Datum einer `unavailable_after`-Angabe (TECH-063), ohne zeitabhängige Wirkung.
/// Akzeptiert ISO 8601 sowie RFC 822/1123 und RFC 850. Ungültige Angaben bleiben
/// als Rohwert erhalten; der Aufrufer bekommt dann `None`.
pub fn parse_unavailable_after(value: &str) -> Option<os_model::Timestamp> {
    let value = value.trim();
    let value = value
        .split_once(':')
        .filter(|(name, _)| name.eq_ignore_ascii_case("unavailable_after"))
        .map_or(value, |(_, date)| date.trim());
    if let Ok(date) = os_model::Timestamp::from_db(value) {
        return Some(date);
    }
    if value.len() == 10 && value.as_bytes().get(4) == Some(&b'-') {
        return os_model::Timestamp::from_db(&format!("{value}T00:00:00Z")).ok();
    }
    let value = value.split_once(',').map_or(value, |(_, date)| date).trim();
    let parts: Vec<&str> = value.split_whitespace().collect();
    let (day, month, year, clock, zone) = match parts.as_slice() {
        [day, month, year, clock, zone] => (*day, *month, *year, *clock, *zone),
        [date, clock, zone] => {
            let mut date = date.split('-');
            let result = (date.next()?, date.next()?, date.next()?, *clock, *zone);
            if date.next().is_some() {
                return None;
            }
            result
        }
        _ => return None,
    };
    let day: u8 = day.parse().ok()?;
    let mut year: u16 = year.parse().ok()?;
    if year < 100 {
        year += if year >= 70 { 1900 } else { 2000 };
    }
    let month = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .position(|m| month.eq_ignore_ascii_case(m))?
        + 1;
    let zone = if ["gmt", "utc", "ut", "z"]
        .iter()
        .any(|z| zone.eq_ignore_ascii_case(z))
    {
        "Z".to_string()
    } else if let Some((_, offset)) = [
        ("est", "-05:00"),
        ("edt", "-04:00"),
        ("cst", "-06:00"),
        ("cdt", "-05:00"),
        ("mst", "-07:00"),
        ("mdt", "-06:00"),
        ("pst", "-08:00"),
        ("pdt", "-07:00"),
    ]
    .iter()
    .find(|(name, _)| zone.eq_ignore_ascii_case(name))
    {
        offset.to_string()
    } else if zone.len() == 5
        && matches!(zone.as_bytes()[0], b'+' | b'-')
        && zone.as_bytes()[1..].iter().all(u8::is_ascii_digit)
    {
        format!("{}:{}", &zone[..3], &zone[3..])
    } else {
        return None;
    };
    let clock = if clock.len() == 5 {
        format!("{clock}:00")
    } else {
        clock.to_string()
    };
    os_model::Timestamp::from_db(&format!("{year:04}-{month:02}-{day:02}T{clock}{zone}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(v: &str) -> Vec<(Option<String>, Vec<String>, String)> {
        parse_value(v, true)
            .into_iter()
            .map(|g| (g.agent, g.tokens, g.raw))
            .collect()
    }

    #[test]
    fn plain_tokens_are_trimmed_and_lowercased() {
        let g = groups(" NoIndex , FOLLOW ");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].1, ["noindex", "follow"]);
        assert_eq!(g[0].2, "NoIndex , FOLLOW");
    }

    #[test]
    fn valued_directives_keep_their_value() {
        let g = groups("max-snippet: 50, max-image-preview:large,max-video-preview:-1");
        assert_eq!(
            g[0].1,
            [
                "max-snippet:50",
                "max-image-preview:large",
                "max-video-preview:-1"
            ]
        );
    }

    #[test]
    fn unavailable_after_date_keeps_its_commas() {
        let g = groups("noindex, unavailable_after: Thursday, 31-Dec-26 23:59:59 GMT, nofollow");
        assert_eq!(g.len(), 1);
        assert_eq!(
            g[0].1,
            [
                "noindex",
                "unavailable_after:thursday,31-dec-26 23:59:59 gmt",
                "nofollow"
            ]
        );
        let g = groups("unavailable_after: 2026-12-31");
        assert_eq!(g[0].1, ["unavailable_after:2026-12-31"]);
    }

    #[test]
    fn agent_prefix_binds_following_directives() {
        let g = groups("googlebot: noindex, nofollow, otherbot: noarchive");
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0.as_deref(), Some("googlebot"));
        assert_eq!(g[0].1, ["noindex", "nofollow"]);
        assert_eq!(g[0].2, "googlebot: noindex, nofollow");
        assert_eq!(g[1].0.as_deref(), Some("otherbot"));
        assert_eq!(g[1].2, "otherbot: noarchive");
    }

    #[test]
    fn generic_part_before_agent_prefix() {
        let g = groups("nofollow, bingbot: noindex");
        assert_eq!(g[0].0, None);
        assert_eq!(g[0].1, ["nofollow"]);
        assert_eq!(g[1].0.as_deref(), Some("bingbot"));
    }

    #[test]
    fn agent_with_unavailable_after_value() {
        let g = groups("googlebot: unavailable_after: 25 Jun 2030 15:00:00 PST");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0.as_deref(), Some("googlebot"));
        assert_eq!(g[0].1, ["unavailable_after:25 jun 2030 15:00:00 pst"]);
    }

    #[test]
    fn meta_never_reads_agent_prefixes() {
        let g = parse_value("googlebot: noindex", false);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].agent, None);
        assert_eq!(g[0].tokens, ["googlebot:noindex"]);
    }

    #[test]
    fn empty_input_has_no_groups() {
        assert!(groups("").is_empty());
        assert!(groups(" , ,").is_empty());
    }
}
