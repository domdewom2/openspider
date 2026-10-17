//! Zählerpflege des Writers (P07): Beispiele aus ONPAGE-015/ONPAGE-171 und Neuberechnung nach `replace`.
//! Infrastruktur des Writers, daher ohne eigene Spec-ID (die Spaltenanzeige prüft P20).

mod store_common;

use os_model::*;
use os_store::counters::{self, RecomputedCounters};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn counters_of(file: &CrawlFile, id: i64) -> RecomputedCounters {
    counters::stored(file.conn(), id).unwrap()
}

fn setup(
    dir: &std::path::Path,
    urls: &[(i64, bool)],
) -> (std::path::PathBuf, os_store::SqliteSink) {
    let path = new_crawl(dir, "c.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls(urls.iter().copied())).unwrap();
    (path, sink)
}

fn reader(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

#[test]
fn inlinks_outlinks_and_unique_counts() {
    // A = 1, B = 2, Z = 3, Y = 4, E (extern) = 5.
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(
        dir.path(),
        &[(1, true), (2, true), (3, true), (4, true), (5, false)],
    );
    let mut batch = WriteBatch::default();
    batch.pages.push(page(
        1,
        vec![
            hyperlink(3, 0),
            hyperlink(3, 1),
            hyperlink(3, 2),
            hyperlink(4, 3),
            hyperlink(5, 4),
            hyperlink(5, 5),
            link(None, LinkType::Hyperlink, 6), // ohne Ziel: zählt nirgends
        ],
    ));
    batch.pages.push(page(2, vec![hyperlink(3, 0)]));
    sink.write(batch).unwrap();

    let file = reader(&path);
    let z = counters_of(&file, 3);
    assert_eq!((z.inlinks, z.unique_inlinks, z.inlinks_all), (4, 2, 4));
    let a = counters_of(&file, 1);
    assert_eq!((a.outlinks, a.unique_outlinks), (4, 2));
    assert_eq!((a.external_outlinks, a.unique_external_outlinks), (2, 1));
    // Das externe Ziel hat Inlinks, aber keine eigenen Outlinks.
    let e = counters_of(&file, 5);
    assert_eq!((e.inlinks, e.unique_inlinks), (2, 1));
    assert_eq!(e.outlinks, 0);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

#[test]
fn verification_detects_a_wrong_counter() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true)]);
    sink.write(WriteBatch {
        pages: vec![page(1, vec![hyperlink(2, 0)])],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    rw.conn()
        .execute(
            "UPDATE url_counters SET inlinks = inlinks + 1 WHERE url_id = 2",
            [],
        )
        .unwrap();
    assert_eq!(counters::verify_all(file.conn()).unwrap(), vec![2]);
}

#[test]
fn self_links_count_as_in_and_outlink_and_separately() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true)]);
    let mut batch = WriteBatch::default();
    batch.pages.push(page(
        1,
        vec![hyperlink(1, 0), hyperlink(1, 1), hyperlink(2, 2)],
    ));
    sink.write(batch).unwrap();
    let file = reader(&path);
    let a = counters_of(&file, 1);
    assert_eq!((a.inlinks, a.unique_inlinks, a.self_inlinks), (2, 1, 2));
    assert_eq!((a.outlinks, a.unique_outlinks, a.self_outlinks), (3, 2, 2));
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

#[test]
fn resources_count_only_in_inlinks_all_and_nofollow_counts_as_link() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true), (3, true)]);
    let mut nofollow = hyperlink(2, 2);
    nofollow.follow = false;
    nofollow.rel = Some("nofollow".into());
    let mut batch = WriteBatch::default();
    batch.pages.push(page(
        1,
        vec![
            link(Some(3), LinkType::Image, 0),
            link(Some(3), LinkType::Css, 1),
            nofollow,
            link(Some(2), LinkType::Canonical, 3),
        ],
    ));
    sink.write(batch).unwrap();
    let file = reader(&path);
    let r = counters_of(&file, 3);
    assert_eq!((r.inlinks, r.unique_inlinks, r.inlinks_all), (0, 0, 2));
    let b = counters_of(&file, 2);
    assert_eq!((b.inlinks, b.unique_inlinks, b.inlinks_all), (1, 1, 2));
    let a = counters_of(&file, 1);
    assert_eq!((a.outlinks, a.unique_outlinks), (1, 1));
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

#[test]
fn links_from_external_pages_are_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, false)]);
    let mut batch = WriteBatch::default();
    batch.pages.push(page(2, vec![hyperlink(1, 0)]));
    sink.write(batch).unwrap();
    let file = reader(&path);
    assert_eq!(counters_of(&file, 1), RecomputedCounters::default());
    assert_eq!(counters_of(&file, 2), RecomputedCounters::default());
    assert_eq!(count(&file, "SELECT COUNT(*) FROM links"), 1);
}

fn image_row(image: i64, doc_order: u32) -> ImageRow {
    ImageRow {
        image_id: Some(UrlId(image)),
        instance: ImageInstance {
            src_raw: format!("/p{image}"),
            resolved: None,
            source: ImageSource::Img,
            alt_present: true,
            alt: Some("x".into()),
            alt_len: Some(1),
            width_attr: None,
            height_attr: None,
            xpath: format!("/html[1]/body[1]/img[{}]", doc_order + 1),
            doc_order,
            ..Default::default()
        },
    }
}

#[test]
fn img_inlinks_count_distinct_pages() {
    // Bild 4 auf drei Seiten, davon zweimal auf Seite 1.
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true), (3, true), (4, true)]);
    let mut batch = WriteBatch::default();
    for id in 1..=3 {
        let mut p = page(id, vec![]);
        p.images.push(image_row(4, 0));
        if id == 1 {
            p.images.push(image_row(4, 1));
        }
        batch.pages.push(p);
    }
    sink.write(batch).unwrap();
    let file = reader(&path);
    assert_eq!(counters_of(&file, 4).img_inlinks, 3);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());

    // replace ohne das Bild: Zähler sinkt.
    let mut p = page(1, vec![]);
    p.replace = true;
    sink.write(WriteBatch {
        pages: vec![p],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(counters_of(&file, 4).img_inlinks, 2);
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM image_instances WHERE page_id = 1"
        ),
        0
    );
}

#[test]
fn replace_removes_old_children_and_corrects_counters_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true), (3, true), (4, false)]);
    let mut batch = WriteBatch::default();
    batch.pages.push(page(
        1,
        vec![
            hyperlink(2, 0),
            hyperlink(2, 1),
            hyperlink(3, 2),
            hyperlink(4, 3),
        ],
    ));
    batch.pages.push(page(2, vec![hyperlink(3, 0)]));
    sink.write(batch).unwrap();
    let file = reader(&path);
    assert_eq!(counters_of(&file, 2).inlinks, 2);
    assert_eq!(counters_of(&file, 3).unique_inlinks, 2);

    // Seite 1 wird neu abgerufen: nur noch ein Link auf 3, dazu ein Selbstlink.
    let mut again = page(1, vec![hyperlink(3, 0), hyperlink(1, 1)]);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(counters_of(&file, 2).inlinks, 0);
    assert_eq!(counters_of(&file, 2).unique_inlinks, 0);
    assert_eq!(counters_of(&file, 3).inlinks, 2);
    assert_eq!(counters_of(&file, 4).inlinks, 0);
    let a = counters_of(&file, 1);
    assert_eq!(
        (a.outlinks, a.unique_outlinks, a.external_outlinks),
        (2, 2, 0)
    );
    assert_eq!((a.self_inlinks, a.self_outlinks), (1, 1));
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM links WHERE source_id = 1"),
        2
    );
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

/// Einfacher deterministischer Zufallsgenerator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_links(rng: &mut Rng, pages: i64, externals: i64) -> (Vec<LinkRow>, Vec<ImageRow>) {
    let mut links = Vec::new();
    let mut images = Vec::new();
    for position in 0..rng.below(12) {
        let target =
            1 + i64::try_from(rng.below(u64::try_from(pages + externals).unwrap())).unwrap();
        let link_type = match rng.below(6) {
            0 => LinkType::Image,
            1 => LinkType::Css,
            2 => LinkType::Canonical,
            _ => LinkType::Hyperlink,
        };
        let target = if rng.below(10) == 0 {
            None
        } else {
            Some(target)
        };
        links.push(link(target, link_type, u32::try_from(position).unwrap()));
        if rng.below(4) == 0 {
            if let Some(t) = target {
                images.push(image_row(t, u32::try_from(position).unwrap()));
            }
        }
    }
    (links, images)
}

#[test]
fn counters_equal_full_recomputation_after_random_rewrites() {
    const PAGES: i64 = 25;
    const EXTERNALS: i64 = 5;
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<(i64, bool)> = (1..=PAGES + EXTERNALS).map(|i| (i, i <= PAGES)).collect();
    let (path, mut sink) = setup(dir.path(), &urls);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    // Erst alle Seiten, dann viele Wiederholungen mit `replace` in gemischten Batches.
    for round in 0..8 {
        let mut batch = WriteBatch::default();
        for id in 1..=PAGES {
            if round > 0 && rng.below(3) != 0 {
                continue;
            }
            let (links, images) = random_links(&mut rng, PAGES, EXTERNALS);
            // Status und Klasse wechseln mit: `html_2xx_sources` folgt dem Zustand der Quelle beim Schreiben.
            let status = [200, 200, 404, 302][usize::try_from(rng.below(4)).unwrap()];
            let class = if rng.below(5) == 0 {
                ContentClass::Css
            } else {
                ContentClass::Html
            };
            let mut p = html_page(id, links, status, class);
            p.images = images;
            p.replace = round > 0;
            batch.pages.push(p);
        }
        sink.write(batch).unwrap();
        let file = reader(&path);
        let bad = counters::verify_all(file.conn()).unwrap();
        assert!(
            bad.is_empty(),
            "Runde {round}: Zähler weichen ab bei URLs {bad:?}"
        );
    }
}

fn html_page(id: i64, links: Vec<LinkRow>, status: u16, class: ContentClass) -> PageWrite {
    let mut p = page(id, links);
    p.fetch.status_code = status;
    p.fetch.content_class = class;
    p
}

// spec: PERF-002
#[test]
fn html_2xx_sources_count_distinct_internal_html_pages_with_status_2xx() {
    // K-56: `% of Total` zählt verschiedene interne HTML-Quellseiten mit Status 2xx und mindestens einem Hyperlink.
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(
        dir.path(),
        &[(1, true), (2, true), (3, true), (4, true), (5, false)],
    );
    let mut batch = WriteBatch::default();
    // Seite 1: HTML 200, zwei Hyperlinks auf 3 (eine Quelle), einer auf 4, ein Bild-Link auf 4 (kein Hyperlink)
    batch.pages.push(html_page(
        1,
        vec![
            hyperlink(3, 0),
            hyperlink(3, 1),
            hyperlink(4, 2),
            link(Some(4), LinkType::Image, 3),
        ],
        200,
        ContentClass::Html,
    ));
    // Seite 2: HTML, aber 404; Seite 3: kein HTML (CSS 200)
    batch
        .pages
        .push(html_page(2, vec![hyperlink(3, 0)], 404, ContentClass::Html));
    batch.pages.push(html_page(
        3,
        vec![hyperlink(4, 0), hyperlink(3, 1)],
        200,
        ContentClass::Css,
    ));
    sink.write(batch).unwrap();
    let file = reader(&path);
    assert_eq!(counters_of(&file, 3).html_2xx_sources, 1, "nur Seite 1");
    assert_eq!(
        counters_of(&file, 3).inlinks,
        4,
        "alle Hyperlinks zählen als Inlinks"
    );
    assert_eq!(counters_of(&file, 4).html_2xx_sources, 1);
    assert_eq!(counters_of(&file, 4).image_link_inlinks, 1);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());

    // Seite 2 wird neu abgerufen und ist jetzt 200: ihr alter Beitrag (404) entfällt, der neue zählt.
    let mut again = html_page(2, vec![hyperlink(3, 0)], 200, ContentClass::Html);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(counters_of(&file, 3).html_2xx_sources, 2);
    // Seite 1 wird neu abgerufen und antwortet jetzt mit 500: sie zählt nicht mehr, auch ohne Änderung der Links.
    let mut broken = html_page(
        1,
        vec![hyperlink(3, 0), hyperlink(3, 1), hyperlink(4, 2)],
        500,
        ContentClass::Html,
    );
    broken.replace = true;
    sink.write(WriteBatch {
        pages: vec![broken],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(
        counters_of(&file, 3).html_2xx_sources,
        1,
        "nur noch Seite 2"
    );
    assert_eq!(counters_of(&file, 4).html_2xx_sources, 0);
    assert_eq!(
        counters_of(&file, 4).image_link_inlinks,
        0,
        "der Bild-Link ist weg"
    );
    // Selbstlink einer 2xx-Seite zählt mit (A-9)
    let mut selfish = html_page(2, vec![hyperlink(2, 0)], 200, ContentClass::Html);
    selfish.replace = true;
    sink.write(WriteBatch {
        pages: vec![selfish],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(counters_of(&file, 2).html_2xx_sources, 1);
    assert_eq!(counters_of(&file, 3).html_2xx_sources, 0);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

// spec: PERF-002
#[test]
fn image_link_inlinks_count_image_typed_link_instances_from_internal_sources() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = setup(dir.path(), &[(1, true), (2, true), (3, true), (4, false)]);
    let mut batch = WriteBatch::default();
    batch.pages.push(page(
        1,
        vec![
            link(Some(3), LinkType::Image, 0),
            link(Some(3), LinkType::Image, 1),
            hyperlink(3, 2),
            link(Some(4), LinkType::Image, 3),
        ],
    ));
    batch
        .pages
        .push(page(2, vec![link(Some(3), LinkType::Image, 0)]));
    sink.write(batch).unwrap();
    let file = reader(&path);
    let r = counters_of(&file, 3);
    assert_eq!((r.image_link_inlinks, r.inlinks, r.inlinks_all), (3, 1, 4));
    assert_eq!(counters_of(&file, 4).image_link_inlinks, 1);
    let mut again = page(1, vec![]);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(counters_of(&file, 3).image_link_inlinks, 1);
    assert_eq!(counters_of(&file, 4).image_link_inlinks, 0);
    assert!(counters::verify_all(file.conn()).unwrap().is_empty());
}

// spec: PERF-002
#[test]
fn counters_are_exact_with_and_without_the_link_index() {
    // `recompute_many` und `verify_all` sind mengenweise: gleiches Ergebnis mit und ohne `links_target`.
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<(i64, bool)> = (1..=40).map(|i| (i, i <= 30)).collect();
    let (path, mut sink) = setup(dir.path(), &urls);
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    let mut batch = WriteBatch::default();
    for id in 1..=30 {
        let (links, images) = random_links(&mut rng, 30, 10);
        let mut p = html_page(
            id,
            links,
            [200, 200, 404, 301][usize::try_from(rng.below(4)).unwrap()],
            if rng.below(5) == 0 {
                ContentClass::Css
            } else {
                ContentClass::Html
            },
        );
        p.images = images;
        batch.pages.push(p);
    }
    sink.write(batch).unwrap();
    sink.finish().unwrap();

    let ids: Vec<i64> = (1..=40).collect();
    let without = reader(&path);
    assert!(!os_store::link_index::present(without.conn()).unwrap());
    let a = counters::recompute_many(without.conn(), &ids).unwrap();
    assert!(counters::verify_all(without.conn()).unwrap().is_empty());
    {
        let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        assert!(os_store::link_index::ensure(rw.conn()).unwrap());
    }
    let with = reader(&path);
    assert!(os_store::link_index::present(with.conn()).unwrap());
    let b = counters::recompute_many(with.conn(), &ids).unwrap();
    assert!(counters::verify_all(with.conn()).unwrap().is_empty());
    assert_eq!(a, b);
    assert_eq!(a.len(), 40);
    for id in 1..=40 {
        assert_eq!(a[&id], counters_of(&with, id), "URL {id}");
        assert_eq!(counters::recompute(with.conn(), id).unwrap(), a[&id]);
    }
}

fn trigger_present(file: &CrawlFile) -> bool {
    count(
        file,
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'urls_counters_insert'",
    ) == 1
}

// spec: PERF-002
#[test]
fn the_writer_creates_the_counter_rows_itself_and_restores_the_insert_trigger_at_rest() {
    // Der Trigger kostet je eingefügter URL ein Statement-Journal (50 µs statt 7 µs, Messung M1-P32): Während des
    // Schreibens fehlt er, jede neue URL bekommt ihre Zählerzeile trotzdem; Pause, Ende und Schließen stellen ihn her.
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "t.ospider", StorageProfile::Balanced);
    assert!(trigger_present(&reader(&path)));
    let mut sink = sink(&path, StorageProfile::Balanced);
    assert!(
        trigger_present(sink.file()),
        "vor dem ersten Schreiben unverändert"
    );
    sink.write(batch_with_urls([(1, true), (2, true), (3, false)]))
        .unwrap();
    assert!(
        !trigger_present(sink.file()),
        "während des Schreibens entfernt"
    );
    assert_eq!(
        count(sink.file(), "SELECT COUNT(*) FROM url_counters"),
        3,
        "eine Zählerzeile je neue URL"
    );
    sink.checkpoint().unwrap();
    assert!(trigger_present(sink.file()), "Pause stellt ihn her");
    sink.write(batch_with_urls([(4, true)])).unwrap();
    assert!(
        !trigger_present(sink.file()),
        "der nächste Batch entfernt ihn wieder"
    );
    assert_eq!(count(sink.file(), "SELECT COUNT(*) FROM url_counters"), 4);
    sink.finish().unwrap();
    assert!(trigger_present(sink.file()), "Ende stellt ihn her");
    sink.write(batch_with_urls([(5, true)])).unwrap();
    drop(sink);
    let file = reader(&path);
    assert!(trigger_present(&file), "Schließen stellt ihn her");
    assert_eq!(count(&file, "SELECT COUNT(*) FROM url_counters"), 5);
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM urls u LEFT JOIN url_counters c ON c.url_id = u.id WHERE c.url_id IS NULL"
        ),
        0
    );
}
