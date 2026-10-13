//! User-Agent-Voreinstellungen (CRAWL-114).
//!
//! Die Liste liegt als Datendatei vor (`resources/user-agents.toml`, zur Übersetzungszeit eingebettet)
//! und kann ohne Code-Änderung durch eine Datei gleichen Aufbaus ersetzt werden
//! ([`parse_presets`]). Getrennt vom HTTP-User-Agent gibt es das robots.txt-Token, unter dem
//! `robots.txt`-Gruppen und agentenspezifische Meta-Robots ausgewertet werden.

use std::sync::OnceLock;

use os_model::UserAgentConfig;

/// Eingebettete Voreinstellungen.
const EMBEDDED: &str = include_str!("../resources/user-agents.toml");

/// Kennung der Standard-Voreinstellung.
pub const DEFAULT_PRESET: &str = "openspider";
/// Kennung der benutzerdefinierten Voreinstellung.
pub const CUSTOM_PRESET: &str = "custom";
/// Standard-Token der robots.txt-Gruppe (K-12).
pub const DEFAULT_ROBOTS_TOKEN: &str = "OpenSpider";
/// Token, wenn kein eigenes gesetzt ist: nur die Gruppe `User-agent: *` greift.
pub const FALLBACK_ROBOTS_TOKEN: &str = "*";

/// Eine Voreinstellung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAgentPreset {
    /// Stabile Kennung (`user_agent.preset`).
    pub id: String,
    /// Anzeigename.
    pub name: String,
    /// Gruppe in der Auswahlliste.
    pub group: String,
    /// HTTP-User-Agent (`{version}` ersetzt); `None` bei „nur Token“ bzw. benutzerdefiniert.
    pub user_agent: Option<String>,
    /// robots.txt-Token; `*` = kein eigenes Token.
    pub robots_token: String,
}

/// Aufgelöster User-Agent einer Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedUserAgent {
    /// HTTP-User-Agent-String (immer gesetzt).
    pub http: String,
    /// robots.txt-Token.
    pub robots_token: String,
    /// Hinweis für Protokoll und Oberfläche, z. B. „kein Token gesetzt“.
    pub warning: Option<String>,
}

/// Der Standard-User-Agent `Mozilla/5.0 (compatible; OpenSpider/<Version>; +https://openspider.de)`.
pub fn default_user_agent() -> String {
    format!(
        "Mozilla/5.0 (compatible; OpenSpider/{}; +https://openspider.de)",
        env!("CARGO_PKG_VERSION")
    )
}

/// Die eingebetteten Voreinstellungen (einmal geparst).
pub fn presets() -> &'static [UserAgentPreset] {
    static CELL: OnceLock<Vec<UserAgentPreset>> = OnceLock::new();
    CELL.get_or_init(|| {
        parse_presets(EMBEDDED)
            .expect("eingebettete user-agents.toml ist gültig (Test deckt das ab)")
    })
}

/// Sucht eine Voreinstellung nach Kennung oder Anzeigename (Groß-/Kleinschreibung und Trennzeichen egal).
pub fn find_preset<'a>(
    presets: &'a [UserAgentPreset],
    id_or_name: &str,
) -> Option<&'a UserAgentPreset> {
    let wanted = squash(id_or_name);
    if wanted.is_empty() {
        return None;
    }
    presets
        .iter()
        .find(|p| squash(&p.id) == wanted)
        .or_else(|| presets.iter().find(|p| squash(&p.name) == wanted))
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Bestimmt HTTP-User-Agent und robots-Token aus der Konfiguration.
///
/// - HTTP-User-Agent: `user_agent.http`, sonst der der Voreinstellung, sonst der Standard.
/// - Token: `user_agent.robots_token`, sonst das der Voreinstellung. Ein frei eingegebener User-Agent
///   (Voreinstellung `custom` oder unbekannt) ohne Token verwendet `*` und liefert eine Warnung.
pub fn resolve(cfg: &UserAgentConfig) -> ResolvedUserAgent {
    resolve_with(cfg, presets())
}

/// Wie [`resolve`] mit einer eigenen Voreinstellungsliste.
pub fn resolve_with(cfg: &UserAgentConfig, list: &[UserAgentPreset]) -> ResolvedUserAgent {
    let preset = find_preset(list, &cfg.preset);
    let custom_http = cfg
        .http
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let explicit_token = cfg
        .robots_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let http = custom_http
        .clone()
        .or_else(|| preset.and_then(|p| p.user_agent.clone()))
        .unwrap_or_else(default_user_agent);

    let mut warning = None;
    if preset.is_none() && custom_http.is_none() {
        warning = Some(format!(
            "unbekannte User-Agent-Voreinstellung {:?}, es gilt der Standard",
            cfg.preset
        ));
    }
    let robots_token = if let Some(t) = explicit_token {
        t
    } else if let Some(p) = preset.filter(|p| p.id != CUSTOM_PRESET) {
        p.robots_token.clone()
    } else {
        if custom_http.is_some() {
            warning = Some(
                "eigener User-Agent ohne robots.txt-Token: es gilt nur die Gruppe „User-agent: *“"
                    .to_string(),
            );
        }
        FALLBACK_ROBOTS_TOKEN.to_string()
    };
    ResolvedUserAgent {
        http,
        robots_token,
        warning,
    }
}

/// Liest Voreinstellungen aus dem TOML-Teilformat der Datendatei (`[[preset]]` mit Zeichenketten-Werten).
pub fn parse_presets(text: &str) -> Result<Vec<UserAgentPreset>, String> {
    #[derive(Default)]
    struct Raw {
        id: Option<String>,
        name: Option<String>,
        group: Option<String>,
        user_agent: Option<String>,
        robots_token: Option<String>,
    }
    fn finish(raw: Raw, out: &mut Vec<UserAgentPreset>) -> Result<(), String> {
        let id = raw.id.ok_or("Eintrag ohne `id`")?;
        let version = env!("CARGO_PKG_VERSION");
        out.push(UserAgentPreset {
            name: raw.name.unwrap_or_else(|| id.clone()),
            group: raw.group.unwrap_or_default(),
            user_agent: raw
                .user_agent
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.replace("{version}", version)),
            robots_token: raw
                .robots_token
                .unwrap_or_else(|| FALLBACK_ROBOTS_TOKEN.to_string()),
            id,
        });
        Ok(())
    }

    let mut out = Vec::new();
    let mut current: Option<Raw> = None;
    for (idx, line) in text.lines().enumerate() {
        let n = idx + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[preset]]" {
            if let Some(raw) = current.take() {
                finish(raw, &mut out).map_err(|e| format!("Zeile {n}: {e}"))?;
            }
            current = Some(Raw::default());
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            return Err(format!("Zeile {n}: `Schlüssel = \"Wert\"` erwartet"));
        };
        let value = parse_string(rest.trim()).map_err(|e| format!("Zeile {n}: {e}"))?;
        let raw = current
            .as_mut()
            .ok_or_else(|| format!("Zeile {n}: Wert außerhalb von [[preset]]"))?;
        match key.trim() {
            "id" => raw.id = Some(value),
            "name" => raw.name = Some(value),
            "group" => raw.group = Some(value),
            "user_agent" => raw.user_agent = Some(value),
            "robots_token" => raw.robots_token = Some(value),
            other => return Err(format!("Zeile {n}: unbekannter Schlüssel {other:?}")),
        }
    }
    if let Some(raw) = current.take() {
        finish(raw, &mut out).map_err(|e| format!("Dateiende: {e}"))?;
    }
    Ok(out)
}

/// Eine einfach gequotete TOML-Zeichenkette mit den Escapes `\"`, `\\`, `\n`, `\t`; Rest der Zeile
/// nach dem schließenden Anführungszeichen darf nur ein Kommentar sein.
fn parse_string(s: &str) -> Result<String, String> {
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return Err("Zeichenkette in Anführungszeichen erwartet".to_string());
    }
    let mut out = String::new();
    loop {
        match chars.next() {
            None => return Err("Zeichenkette nicht geschlossen".to_string()),
            Some('"') => break,
            Some('\\') => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                other => return Err(format!("unbekannte Escape-Folge {other:?}")),
            },
            Some(c) => out.push(c),
        }
    }
    let rest: String = chars.collect();
    let rest = rest.trim();
    if rest.is_empty() || rest.starts_with('#') {
        Ok(out)
    } else {
        Err(format!("unerwartet nach der Zeichenkette: {rest:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(preset: &str, http: Option<&str>, token: Option<&str>) -> UserAgentConfig {
        UserAgentConfig {
            preset: preset.to_string(),
            http: http.map(str::to_string),
            robots_token: token.map(str::to_string),
        }
    }

    // spec: CRAWL-114
    #[test]
    fn embedded_list_contains_all_required_presets() {
        let ids: Vec<&str> = presets().iter().map(|p| p.id.as_str()).collect();
        for required in [
            "openspider",
            "googlebot-smartphone",
            "googlebot-desktop",
            "googlebot-image",
            "googlebot-news",
            "googlebot-video",
            "google-inspectiontool",
            "google-extended",
            "bingbot-mobile",
            "bingbot-desktop",
            "yandex",
            "baiduspider",
            "duckduckbot",
            "applebot",
            "gptbot",
            "claudebot",
            "perplexitybot",
            "chrome-desktop",
            "chrome-mobile",
            "safari-iphone",
            "firefox",
            "edge",
            "custom",
        ] {
            assert!(ids.contains(&required), "Voreinstellung {required} fehlt");
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "Kennungen müssen eindeutig sein");
    }

    // spec: CRAWL-114
    #[test]
    fn default_preset_is_openspider_with_version_and_token() {
        let r = resolve(&UserAgentConfig::default());
        assert_eq!(r.http, default_user_agent());
        assert!(r.http.starts_with("Mozilla/5.0 (compatible; OpenSpider/"));
        assert!(r.http.contains(env!("CARGO_PKG_VERSION")));
        assert!(!r.http.contains("{version}"));
        assert_eq!(r.robots_token, "OpenSpider");
        assert_eq!(r.warning, None);
    }

    // spec: CRAWL-114
    #[test]
    fn googlebot_smartphone_uses_googlebot_token() {
        let r = resolve(&cfg("googlebot-smartphone", None, None));
        assert_eq!(r.robots_token, "Googlebot");
        assert!(r.http.contains("Googlebot/2.1"));
        // Anzeigename wird ebenfalls erkannt
        let r2 = resolve(&cfg("Googlebot Smartphone", None, None));
        assert_eq!(r, r2);
    }

    // spec: CRAWL-114
    #[test]
    fn free_user_agent_without_token_falls_back_to_star_with_warning() {
        let r = resolve(&cfg("custom", Some("MeinBot/1.0"), None));
        assert_eq!(r.http, "MeinBot/1.0");
        assert_eq!(r.robots_token, "*");
        assert!(r.warning.is_some());
        // mit Token keine Warnung
        let r = resolve(&cfg("custom", Some("MeinBot/1.0"), Some("MeinBot")));
        assert_eq!(r.robots_token, "MeinBot");
        assert_eq!(r.warning, None);
    }

    // spec: CRAWL-114
    #[test]
    fn token_only_and_unknown_presets_fall_back_to_default_agent() {
        let r = resolve(&cfg("google-extended", None, None));
        assert_eq!(r.http, default_user_agent());
        assert_eq!(r.robots_token, "Google-Extended");
        let r = resolve(&cfg("gibt-es-nicht", None, None));
        assert_eq!(r.http, default_user_agent());
        assert!(r.warning.is_some());
    }

    // spec: CRAWL-114
    #[test]
    fn explicit_http_overrides_preset_but_keeps_its_token() {
        let r = resolve(&cfg("openspider", Some("Spezial/2"), None));
        assert_eq!(r.http, "Spezial/2");
        assert_eq!(r.robots_token, "OpenSpider");
    }

    // spec: CRAWL-114
    #[test]
    fn list_can_be_replaced_without_code_change() {
        let list = parse_presets(
            "# eigene Liste\n[[preset]]\nid = \"x\"\nname = \"X \\\"Bot\\\"\"\nuser_agent = \"XBot/{version}\" # Kommentar\nrobots_token = \"XBot\"\n",
        )
        .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "X \"Bot\"");
        assert_eq!(
            list[0].user_agent.as_deref(),
            Some(format!("XBot/{}", env!("CARGO_PKG_VERSION")).as_str())
        );
        let r = resolve_with(&cfg("x", None, None), &list);
        assert_eq!(r.robots_token, "XBot");
        assert!(parse_presets("id = \"a\"").is_err());
        assert!(parse_presets("[[preset]]\nid = a").is_err());
        assert!(parse_presets("[[preset]]\nfoo = \"a\"").is_err());
        assert!(parse_presets("[[preset]]\nname = \"a\"").is_err());
    }
}
