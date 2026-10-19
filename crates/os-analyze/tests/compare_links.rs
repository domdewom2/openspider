//! Inlinks/Outlinks-Vergleich im unteren Fenster (OUT-149; CRAWL-144 verweist hierher). Fixture: `out-m2-compare/v1`
//! (Previous) und `out-m2-compare/v2` (Current): `/start.html` verliert den Link auf `/b.html` und erhält einen auf
//! `/a.html`, `/c.html` behält den Link der Startseite mit geändertem Ankertext, `/deep.html` rutscht von Tiefe 1 auf 2,
//! `/tc.html` wechselt vom Hyperlink zum Canonical auf dasselbe Ziel.

#[path = "compare_common.rs"]
mod common;

use common::*;
use os_analyze::compare::{
    ChangeFilter, Comparison, LinkChange, LinkDirection, LinkRow, LinkSource, Presence, Side,
};
use os_model::Value;

fn text(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Null => String::new(),
        other => format!("{other:?}"),
    }
}

/// `(Quelle, Ziel)` der Zeilen, sortiert.
fn ends(rows: &[&LinkRow]) -> Vec<(String, String)> {
    let mut v: Vec<_> = rows
        .iter()
        .map(|r| (text(&r.values[1]), text(&r.values[2])))
        .collect();
    v.sort();
    v
}

fn hyperlinks(rows: Vec<&LinkRow>) -> Vec<&LinkRow> {
    rows.into_iter()
        .filter(|r| text(&r.values[0]) == "Hyperlink")
        .collect()
}

fn links_of(
    cmp: &Comparison,
    new: &os_store::CrawlFile,
    path: &str,
    d: LinkDirection,
) -> os_analyze::compare::LinkComparison {
    cmp.links(Side::Current, id_of(new, &www(path)), d).unwrap()
}

const ADDED: ChangeFilter = ChangeFilter::Only(LinkChange::Added);
const REMOVED: ChangeFilter = ChangeFilter::Only(LinkChange::Removed);
const ANCHOR: ChangeFilter = ChangeFilter::Only(LinkChange::AnchorChanged);
const SAME: ChangeFilter = ChangeFilter::Only(LinkChange::Unchanged);

// spec: OUT-149
// spec: OUT-141
#[tokio::test(flavor = "multi_thread")]
async fn swapping_keeps_link_identities_mapped_on_the_original_staging_crawl() {
    let (old, new) = (staging().await, live().await);
    let rules = [
        rule(r"^http://staging\.site\.test", "http://www.site.test"),
        rule(r"(\.test/.+)/$", "$1"),
    ];
    let mut cmp = compare(&old, &new, &rules);
    for swaps in 0..3 {
        if swaps > 0 {
            cmp.swap();
        }
        let (file, a) = if swaps == 1 {
            (&old.file, "http://staging.site.test/a/")
        } else {
            (&new.file, "http://www.site.test/a")
        };
        // /a hat einen Outlink auf /shop/a und einen Inlink von der Startseite.
        for direction in [LinkDirection::Outlinks, LinkDirection::Inlinks] {
            let links = cmp.links(Side::Current, id_of(file, a), direction).unwrap();
            assert_eq!(links.presence, Presence::Both);
            for source in [LinkSource::Current, LinkSource::Previous] {
                let rows = links.view(source, Side::Current, ChangeFilter::All);
                assert!(!rows.is_empty(), "{swaps} {direction:?} {source:?}");
                assert!(
                    rows.iter().all(|r| r.change == Some(LinkChange::Unchanged)),
                    "{rows:?}"
                );
                assert!(links.view(source, Side::Current, ADDED).is_empty());
                assert!(links.view(source, Side::Current, REMOVED).is_empty());
            }
        }
    }
}

// spec: OUT-149
#[tokio::test(flavor = "multi_thread")]
async fn page_one_level_deeper_shows_the_removed_link_from_the_homepage() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let deep = links_of(&cmp, &new.file, "/deep.html", LinkDirection::Inlinks);
    let removed = deep.view(LinkSource::Auto, Side::Current, REMOVED);
    assert_eq!(ends(&removed), [(www("/"), www("/deep.html"))]);
    assert!(removed.iter().all(|r| r.crawl == Side::Previous));
    let added = deep.view(LinkSource::Auto, Side::Current, ADDED);
    assert_eq!(ends(&added), [(www("/a.html"), www("/deep.html"))]);
}

// spec: OUT-149
// Quelle „Auto“ folgt der Auswahl Current/Previous des oberen Fensters.
#[tokio::test(flavor = "multi_thread")]
async fn auto_source_follows_the_upper_window() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let deep = links_of(&cmp, &new.file, "/deep.html", LinkDirection::Inlinks);
    let all = ChangeFilter::All;
    let previous = deep.view(LinkSource::Auto, Side::Previous, all);
    assert_eq!(ends(&previous), [(www("/"), www("/deep.html"))]);
    let current = deep.view(LinkSource::Auto, Side::Current, all);
    assert_eq!(ends(&current), [(www("/a.html"), www("/deep.html"))]);
    // Feste Quelle ignoriert das obere Fenster.
    let fixed = deep.view(LinkSource::Current, Side::Previous, all);
    assert_eq!(ends(&fixed), ends(&current));
    // Auswahl einer Previous-URL im oberen Fenster liefert dieselbe Ansicht.
    let from_previous = cmp
        .links(
            Side::Previous,
            id_of(&old.file, &www("/deep.html")),
            LinkDirection::Inlinks,
        )
        .unwrap();
    assert_eq!(from_previous, deep);
}

// spec: OUT-149
#[tokio::test(flavor = "multi_thread")]
async fn start_loses_b_and_gains_a() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let start = links_of(&cmp, &new.file, "/start.html", LinkDirection::Outlinks);
    let added = hyperlinks(start.view(LinkSource::Auto, Side::Current, ADDED));
    assert_eq!(ends(&added), [(www("/start.html"), www("/a.html"))]);
    let removed = hyperlinks(start.view(LinkSource::Auto, Side::Current, REMOVED));
    assert_eq!(ends(&removed), [(www("/start.html"), www("/b.html"))]);
    let all_current = start.view(LinkSource::Current, Side::Current, ChangeFilter::All);
    assert!(!ends(&all_current).contains(&(www("/start.html"), www("/b.html"))));
}

// spec: OUT-149
#[tokio::test(flavor = "multi_thread")]
async fn changed_anchor_is_its_own_change_kind() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let c = links_of(&cmp, &new.file, "/c.html", LinkDirection::Inlinks);
    let changed = c.view(LinkSource::Current, Side::Current, ANCHOR);
    assert_eq!(changed.len(), 1);
    let row = changed[0];
    assert_eq!(row.change, Some(LinkChange::AnchorChanged));
    assert_eq!(text(&row.values[5]), "Neu");
    assert_eq!(row.previous_anchor.as_deref(), Some("Alt"));
    let export = row.export_values();
    assert_eq!(text(&export[15]), "Anchor Changed");
    assert_eq!(text(&export[16]), "Current");
    assert_eq!(text(&export[17]), "Alt");
    for other in [ADDED, REMOVED, SAME] {
        assert!(c.view(LinkSource::Current, Side::Current, other).is_empty());
    }
}

// spec: OUT-149
// Ein Wechsel nur des `Type` (Hyperlink → Canonical auf dasselbe Ziel) ist „Entfernt“ + „Hinzugefügt“.
#[tokio::test(flavor = "multi_thread")]
async fn type_change_is_removed_plus_added() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let tc = links_of(&cmp, &new.file, "/tc.html", LinkDirection::Outlinks);
    let target = www("/tc-target.html");
    let added = tc.view(LinkSource::Auto, Side::Current, ADDED);
    assert!(added
        .iter()
        .any(|r| text(&r.values[0]) == "Canonical" && text(&r.values[2]) == target));
    let removed = tc.view(LinkSource::Auto, Side::Current, REMOVED);
    assert!(removed
        .iter()
        .any(|r| text(&r.values[0]) == "Hyperlink" && text(&r.values[2]) == target));
}

// spec: OUT-149
// Invariante je URL über alle URLs, die in beiden Crawls existieren.
#[tokio::test(flavor = "multi_thread")]
async fn per_url_invariant_for_every_paired_url() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let mut checked = 0;
    for id in cmp.pairing().paired_current.iter() {
        for d in [LinkDirection::Inlinks, LinkDirection::Outlinks] {
            let l = cmp.links(Side::Current, i64::from(id), d).unwrap();
            assert_eq!(l.presence, Presence::Both);
            let n = |s: LinkSource, f: ChangeFilter| l.view(s, Side::Current, f).len();
            assert_eq!(
                n(LinkSource::Current, ChangeFilter::All),
                n(LinkSource::Current, ADDED)
                    + n(LinkSource::Current, ANCHOR)
                    + n(LinkSource::Current, SAME)
            );
            assert_eq!(
                n(LinkSource::Previous, ChangeFilter::All),
                n(LinkSource::Previous, REMOVED)
                    + n(LinkSource::Previous, ANCHOR)
                    + n(LinkSource::Previous, SAME)
            );
            checked += 1;
        }
    }
    assert!(checked >= 20);
}

// spec: OUT-149
// Existiert die URL nur in einem Crawl, sind die Änderungsfilter ausgegraut und die Tabelle zeigt dessen Links.
#[tokio::test(flavor = "multi_thread")]
async fn url_in_one_crawl_only_shows_its_own_links() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let fresh = links_of(&cmp, &new.file, "/new1.html", LinkDirection::Inlinks);
    assert_eq!(fresh.presence, Presence::CurrentOnly);
    assert!(!fresh.filters_enabled());
    let rows = fresh.view(LinkSource::Previous, Side::Previous, REMOVED);
    assert_eq!(ends(&rows), [(www("/"), www("/new1.html"))]);
    assert!(rows.iter().all(|r| r.change.is_none()));

    let gone = cmp
        .links(
            Side::Previous,
            id_of(&old.file, &www("/gone1.html")),
            LinkDirection::Inlinks,
        )
        .unwrap();
    assert_eq!(gone.presence, Presence::PreviousOnly);
    let rows = gone.view(LinkSource::Current, Side::Current, ChangeFilter::All);
    assert_eq!(ends(&rows), [(www("/"), www("/gone1.html"))]);
    assert!(rows.iter().all(|r| r.crawl == Side::Previous));
}
