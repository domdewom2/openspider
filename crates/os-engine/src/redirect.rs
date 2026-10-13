//! Weiterleitungsketten, Schleifen und Limits (CRAWL-014, CRAWL-015, CRAWL-017, CRAWL-072).
//!
//! Dieses Modul enthält die reinen Teile: Auswahl der Weiterleitung einer Antwort (HTTP-3xx vor Refresh),
//! Schleifenerkennung über den Weiterleitungsgraphen und die Flag-Namen. Die Einplanung des Ziels
//! (Frontier, Tiefe, Cookie-Sitzung) macht der Worker.
//!
//! * HTTP-3xx (301, 302, 303, 307, 308) mit `Location`: Ziel relativ zur angefragten URL aufgelöst. Der Body
//!   einer 3xx-Antwort wird nie ausgewertet; 3xx ohne (brauchbare) `Location` ergibt keine Weiterleitung.
//! * Refresh: HTTP-Header `Refresh` vor dem ersten Meta-Refresh. Ein Refresh ohne Ziel oder mit der eigenen
//!   URL als Ziel (Vergleich nach TECH-002) ist ein Selbst-Refresh und keine Weiterleitung.
//! * Schleife: Beim Eintragen einer Kante `a → b` wird von `b` aus der Graph abgelaufen; kommt man zu `a`
//!   zurück, bilden die durchlaufenen URLs eine Schleife. Weil jede URL nur einmal abgerufen wird (Seen-Set),
//!   erzeugt eine Schleife nie einen Endlos-Abruf.

use std::collections::{BTreeSet, HashMap};

use os_model::{RedirectType, RefreshRef, RefreshSource, UrlId};
use os_url::NormUrl;
use url::Url;

/// Flag „Weiterleitung ist Teil einer Schleife“ (`url_flags.flag`).
pub const FLAG_LOOP: &str = "redirect_loop";
/// Flag „Kette hat das Limit erreicht“ (`url_flags.flag`).
pub const FLAG_LIMIT: &str = "redirect_limit_reached";

/// Statuscodes, bei denen die Engine `Location` folgt.
pub fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Eine erkannte Weiterleitung.
#[derive(Debug, Clone)]
pub struct RedirectTarget {
    /// Art.
    pub kind: RedirectType,
    /// Ziel, normalisiert.
    pub target: NormUrl,
    /// Rohwert von `Location` bzw. `url=` (für die Linkzeile).
    pub raw: String,
    /// Verzögerung (nur Refresh).
    pub delay: Option<u32>,
}

/// Wertet `Location` einer 3xx-Antwort aus. `None` bei fehlender oder unbrauchbarer `Location`.
pub fn http_target(request: &Url, headers: &[(String, String)]) -> Option<RedirectTarget> {
    let raw = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("location"))
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())?;
    let resolved = os_url::resolve(request, &raw)?;
    if !matches!(resolved.scheme(), "http" | "https") || resolved.host_str().is_none() {
        return None;
    }
    Some(RedirectTarget {
        kind: RedirectType::Http,
        target: os_url::normalize(&resolved),
        raw,
        delay: None,
    })
}

/// Ergebnis der Auswertung der Refresh-Angaben einer Seite.
#[derive(Debug, Clone, Default)]
pub struct RefreshOutcome {
    /// Verzögerung des maßgeblichen Refresh (auch bei Selbst-Refresh), für `meta_refresh_delay`.
    pub delay: Option<u32>,
    /// Die Weiterleitung, falls das Ziel eine andere URL ist.
    pub redirect: Option<RedirectTarget>,
}

/// Bestimmt den maßgeblichen Refresh: erst der HTTP-Header, sonst das erste Meta-Tag.
pub fn refresh_target(own: &Url, refs: &[RefreshRef]) -> RefreshOutcome {
    let chosen = refs
        .iter()
        .find(|r| r.source == RefreshSource::Header)
        .or_else(|| refs.iter().find(|r| r.source == RefreshSource::Meta));
    let Some(r) = chosen else {
        return RefreshOutcome::default();
    };
    let kind = match r.source {
        RefreshSource::Header => RedirectType::HttpRefresh,
        RefreshSource::Meta => RedirectType::MetaRefresh,
    };
    let mut out = RefreshOutcome {
        delay: r.delay,
        redirect: None,
    };
    let Some(target) = r.target.as_deref() else {
        return out;
    };
    let Ok(url) = Url::parse(target) else {
        return out;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return out;
    }
    if os_url::eq_for_compare(own.as_str(), url.as_str()) {
        return out; // Selbst-Refresh
    }
    out.redirect = Some(RedirectTarget {
        kind,
        target: os_url::normalize(&url),
        raw: r.target_raw.clone().unwrap_or_else(|| target.to_string()),
        delay: r.delay,
    });
    out
}

/// Weiterleitungsgraph (nur Quellen von Weiterleitungen) mit den gesammelten Flags.
#[derive(Debug, Default)]
pub struct RedirectGraph {
    next: HashMap<UrlId, UrlId>,
    loops: BTreeSet<UrlId>,
    limits: BTreeSet<UrlId>,
}

impl RedirectGraph {
    /// Trägt die Kante `from → to` ein. Schließt sie eine Schleife, werden deren Mitglieder geflaggt und
    /// zurückgegeben (die URL `from` eingeschlossen).
    pub fn add_edge(&mut self, from: UrlId, to: UrlId) -> Option<Vec<UrlId>> {
        self.next.insert(from, to);
        let mut members = vec![from];
        let mut current = to;
        loop {
            if current == from {
                self.loops.extend(members.iter().copied());
                return Some(members);
            }
            if members.len() > self.next.len() {
                return None; // läuft in eine fremde Schleife, `from` gehört nicht dazu
            }
            members.push(current);
            match self.next.get(&current) {
                Some(&n) => current = n,
                None => return None,
            }
        }
    }

    /// Merkt, dass die Kette bei `id` das Limit erreicht hat (CRAWL-072).
    pub fn flag_limit(&mut self, id: UrlId) {
        self.limits.insert(id);
    }

    /// Alle Flags `(url_id, flag)` in stabiler Reihenfolge.
    pub fn flags(&self) -> Vec<(UrlId, &'static str)> {
        let mut out: Vec<(UrlId, &'static str)> = self
            .loops
            .iter()
            .map(|id| (*id, FLAG_LOOP))
            .chain(self.limits.iter().map(|id| (*id, FLAG_LIMIT)))
            .collect();
        out.sort();
        out
    }

    /// Ob die URL als Schleifenmitglied geflaggt ist.
    pub fn is_loop(&self, id: UrlId) -> bool {
        self.loops.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{HintSource, RefreshRef};

    fn id(n: i64) -> UrlId {
        UrlId(n)
    }

    fn refresh(
        source: RefreshSource,
        delay: Option<u32>,
        target: Option<&str>,
        order: u32,
    ) -> RefreshRef {
        RefreshRef {
            source,
            raw: format!("{delay:?};{target:?}"),
            delay,
            target_raw: target.map(str::to_string),
            target: target.map(str::to_string),
            in_head: true,
            order,
        }
    }

    // spec: CRAWL-014
    #[test]
    fn location_is_resolved_against_the_requested_url() {
        let req = Url::parse("https://site.test/dir/r-relativ").unwrap();
        let h = |v: &str| vec![("location".to_string(), v.to_string())];
        let t = http_target(&req, &h("../ziel")).unwrap();
        assert_eq!(t.target.encoded, "https://site.test/ziel");
        assert_eq!(t.kind, RedirectType::Http);
        let t = http_target(&req, &h("/zïel")).unwrap();
        assert_eq!(t.target.encoded, "https://site.test/z%C3%AFel");
        let t = http_target(&req, &h("/mit leerzeichen")).unwrap();
        assert_eq!(t.target.encoded, "https://site.test/mit%20leerzeichen");
        let t = http_target(&req, &h("https://other.test/#frag")).unwrap();
        assert_eq!(t.target.encoded, "https://other.test/");
        // fehlend, leer, nicht http(s)
        assert!(http_target(&req, &[]).is_none());
        assert!(http_target(&req, &h("  ")).is_none());
        assert!(http_target(&req, &h("mailto:a@b.test")).is_none());
    }

    // spec: CRAWL-014
    #[test]
    fn only_the_five_statuses_are_followed() {
        for s in [301, 302, 303, 307, 308] {
            assert!(is_redirect_status(s), "{s}");
        }
        for s in [200, 300, 304, 305, 306, 309, 404] {
            assert!(!is_redirect_status(s), "{s}");
        }
    }

    // spec: CRAWL-015
    #[test]
    fn header_refresh_beats_meta_and_only_the_first_meta_counts() {
        let own = Url::parse("https://site.test/p").unwrap();
        let refs = vec![
            refresh(RefreshSource::Meta, Some(5), Some("https://site.test/a"), 0),
            refresh(RefreshSource::Meta, Some(0), Some("https://site.test/b"), 1),
        ];
        let out = refresh_target(&own, &refs);
        let r = out.redirect.unwrap();
        assert_eq!(r.target.encoded, "https://site.test/a");
        assert_eq!(r.kind, RedirectType::MetaRefresh);
        assert_eq!(out.delay, Some(5));

        let mut with_header = refs.clone();
        with_header.push(refresh(
            RefreshSource::Header,
            Some(0),
            Some("https://site.test/h"),
            2,
        ));
        let r = refresh_target(&own, &with_header).redirect.unwrap();
        assert_eq!(r.target.encoded, "https://site.test/h");
        assert_eq!(r.kind, RedirectType::HttpRefresh);
    }

    // spec: CRAWL-015
    #[test]
    fn refresh_without_target_or_to_itself_is_a_self_refresh() {
        let own = Url::parse("https://site.test/p").unwrap();
        let none = refresh_target(&own, &[refresh(RefreshSource::Meta, Some(30), None, 0)]);
        assert!(none.redirect.is_none());
        assert_eq!(none.delay, Some(30), "Verzögerung bleibt erhalten");
        let same = refresh_target(
            &own,
            &[refresh(
                RefreshSource::Meta,
                Some(0),
                Some("https://site.test/p#x"),
                0,
            )],
        );
        assert!(same.redirect.is_none());
        assert_eq!(same.delay, Some(0));
        assert!(refresh_target(&own, &[]).redirect.is_none());
        let _ = HintSource::Html;
    }

    // spec: CRAWL-017
    #[test]
    fn closing_edge_of_a_cycle_flags_all_members() {
        let mut g = RedirectGraph::default();
        // /x → /y, danach /y → /x
        assert_eq!(g.add_edge(id(1), id(2)), None);
        let members = g.add_edge(id(2), id(1)).expect("Schleife");
        assert_eq!(members.len(), 2);
        assert!(g.is_loop(id(1)) && g.is_loop(id(2)));
        assert_eq!(g.flags(), vec![(id(1), FLAG_LOOP), (id(2), FLAG_LOOP)]);
    }

    // spec: CRAWL-017
    #[test]
    fn self_redirect_is_a_loop_and_a_plain_chain_is_not() {
        let mut g = RedirectGraph::default();
        assert_eq!(g.add_edge(id(5), id(5)), Some(vec![id(5)]));
        assert!(g.is_loop(id(5)));
        let mut g = RedirectGraph::default();
        assert_eq!(g.add_edge(id(1), id(2)), None);
        assert_eq!(g.add_edge(id(2), id(3)), None);
        assert_eq!(g.add_edge(id(3), id(4)), None);
        assert!(g.flags().is_empty());
    }

    // spec: CRAWL-017
    #[test]
    fn longer_cycle_and_entry_into_a_foreign_cycle() {
        let mut g = RedirectGraph::default();
        assert_eq!(g.add_edge(id(1), id(2)), None);
        assert_eq!(g.add_edge(id(2), id(3)), None);
        let members = g.add_edge(id(3), id(1)).expect("Schleife aus drei");
        assert_eq!(members.len(), 3);
        // 9 → 1: führt in die Schleife hinein, ist aber kein Mitglied
        assert_eq!(g.add_edge(id(9), id(1)), None);
        assert!(!g.is_loop(id(9)));
        assert_eq!(g.flags().len(), 3);
    }

    // spec: CRAWL-072
    #[test]
    fn limit_flags_are_collected_and_sorted() {
        let mut g = RedirectGraph::default();
        g.flag_limit(id(7));
        g.add_edge(id(1), id(1));
        assert_eq!(g.flags(), vec![(id(1), FLAG_LOOP), (id(7), FLAG_LIMIT)]);
    }
}
