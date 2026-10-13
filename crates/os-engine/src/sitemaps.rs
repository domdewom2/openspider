//! Sitemap-Ingest (TECH-210): Index-Rekursion, Zyklen, gzip.
//!
//! [`ingest`] holt die vorgegebenen Sitemaps (Start-/Listen-URL, `sitemaps.urls`, robots.txt-`Sitemap:`-Zeilen)
//! samt den Sitemaps, die Sitemap-Indizes nennen, und wertet sie mit `os_parse::parse_sitemap` aus. IDs
//! (`SitemapId`/`UrlId`), `SitemapWrite` und die Queue-Einträge bildet P15 in `crawl.rs`; dieser Teil kennt weder
//! Crawl-Datei noch Frontier.
//!
//! * **Reihenfolge:** breitenweise in der Reihenfolge der `seeds`; Kinder eines Index folgen in Dateireihenfolge.
//!   `FetchedSitemap::parent` verweist auf den Index in der Ergebnisliste.
//! * **Doppelte:** dieselbe Sitemap-URL (nach `os_url::normalize::normalize`, ohne Fragment) wird genau einmal
//!   abgerufen; bei mehreren Seeds gewinnt der erste (`found_via`). Das gilt auch für Zyklen (ein Index, der sich
//!   selbst oder einen Vorfahren nennt) – sie ergeben eine Warnung und keinen zweiten Abruf.
//! * **Tiefe:** Seeds liegen auf Tiefe 0, die Kinder eines Index auf Tiefe + 1. Abgerufen wird bis
//!   `max_index_depth` (M1: 3); die Einträge eines Index auf dieser Tiefe werden nicht abgerufen und ergeben eine
//!   Warnung (TECH-210, K-27).
//! * **Einträge:** `SitemapDoc::locs` enthält auch ungültige Einträge (`url = None`); für die Rekursion zählen nur
//!   Einträge mit `url`. Beim Zählen für die 50 000er-Grenze gilt `locs.len()` (TECH-217).
//! * **Weiterleitungen:** bis zu [`MAX_REDIRECTS`] HTTP-Weiterleitungen (301/302/303/307/308) werden verfolgt;
//!   `FetchedSitemap::url` bleibt die angeforderte URL, `fetch` ist die letzte Antwort (`fetch.url` die Endadresse).
//! * **Body:** Nach dem Auswerten ist `fetch`s Body leer (Speicher); `size_bytes`, `md5_hex` und Header bleiben.
//!   Die entpackte Größe der Sitemap steht in `doc.size_bytes`.
//! * **`doc`:** `Some`, wenn die Antwort 2xx war; ein unlesbarer Inhalt ergibt `SitemapKind::Invalid` ohne
//!   Einträge. Bei Fehlern, Nicht-2xx und fehlender Antwort ist `doc = None`.

use std::collections::{HashSet, VecDeque};

use os_fetch::{Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult};
use os_model::{SitemapDoc, SitemapKind};

/// Höchstzahl verfolgter Weiterleitungen je Sitemap-Abruf.
pub const MAX_REDIRECTS: u32 = 5;

/// Obergrenze für den Body einer Sitemap (komprimiert, wie übertragen): 256 MiB. Größere Dateien werden
/// abgeschnitten; die Regel „über 50 000 URLs bzw. 50 MB“ (CRAWL-060) bleibt davon unberührt (Verstoß, kein Abbruch).
pub const MAX_BODY_BYTES: u64 = 256 << 20;

/// Ausgangspunkt für den Abruf einer Sitemap.
#[derive(Debug, Clone)]
pub struct SitemapSeed {
    /// URL der Sitemap.
    pub url: url::Url,
    /// Herkunft: `"start"`, `"list"`, `"config"` oder `"robots"`.
    pub found_via: String,
}

/// Eine abgerufene Sitemap.
#[derive(Debug, Clone)]
pub struct FetchedSitemap {
    /// URL der Sitemap.
    pub url: String,
    /// Index der übergeordneten Sitemap in der Ergebnisliste.
    pub parent: Option<usize>,
    /// Tiefe in der Index-Hierarchie.
    pub depth: u32,
    /// Herkunft wie bei `SitemapSeed`.
    pub found_via: String,
    /// Abrufergebnis.
    pub fetch: FetchResult,
    /// Ausgewertete Datei, falls die Antwort auswertbar war.
    pub doc: Option<SitemapDoc>,
}

/// Hinweis auf eine nicht abgerufene Sitemap; P15 schreibt ihn als `CrawlEvent` mit `kind = "warn"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitemapWarning {
    /// Kennung: `sitemap_index_too_deep` oder `sitemap_cycle`.
    pub code: &'static str,
    /// Lesbarer Text mit der betroffenen URL.
    pub message: String,
}

/// Ergebnis von [`ingest_with_warnings`].
#[derive(Debug, Clone, Default)]
pub struct SitemapIngest {
    /// Abgerufene Sitemaps in Abrufreihenfolge.
    pub sitemaps: Vec<FetchedSitemap>,
    /// Hinweise (zu tiefe Indizes, Zyklen).
    pub warnings: Vec<SitemapWarning>,
}

/// Seeds aus `sitemaps.urls` („Diese Sitemaps crawlen“, `found_via = "config"`). Sie gelten unabhängig von
/// `sitemaps.crawl_linked` (TECH-210: manuell vorgegebene Sitemaps werden immer verarbeitet); ungültige oder
/// nicht-http(s)-Einträge und Leerzeilen entfallen, doppelte bleiben einmal stehen.
pub fn configured_seeds(cfg: &os_model::CrawlConfig) -> Vec<SitemapSeed> {
    let mut seen = HashSet::new();
    cfg.sitemaps
        .urls
        .iter()
        .filter_map(|raw| url::Url::parse(raw.trim()).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.has_host())
        .filter(|u| seen.insert(key(u)))
        .map(|url| SitemapSeed {
            url,
            found_via: "config".to_string(),
        })
        .collect()
}

/// Ruft die Sitemaps samt Unter-Sitemaps ab (Index-Tiefe höchstens `max_index_depth`, M1: 3).
///
/// Hinweise (zu tiefe Indizes, Zyklen) gehen nur ins Log (`tracing::warn!`); wer sie als Ereignis in die
/// Crawl-Datei schreiben will, nimmt [`ingest_with_warnings`].
pub async fn ingest<F: Fetch + ?Sized>(
    fetcher: &F,
    seeds: Vec<SitemapSeed>,
    max_index_depth: u32,
) -> Vec<FetchedSitemap> {
    let out = ingest_with_warnings(fetcher, seeds, max_index_depth).await;
    for w in &out.warnings {
        tracing::warn!(code = w.code, "{}", w.message);
    }
    out.sitemaps
}

/// Wie [`ingest`], liefert die Hinweise aber mit zurück.
pub async fn ingest_with_warnings<F: Fetch + ?Sized>(
    fetcher: &F,
    seeds: Vec<SitemapSeed>,
    max_index_depth: u32,
) -> SitemapIngest {
    struct Todo {
        url: url::Url,
        parent: Option<usize>,
        depth: u32,
        found_via: String,
    }

    let mut out = SitemapIngest::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<Todo> = VecDeque::new();
    for seed in seeds {
        if seen.insert(key(&seed.url)) {
            queue.push_back(Todo {
                url: seed.url,
                parent: None,
                depth: 0,
                found_via: seed.found_via,
            });
        }
    }

    while let Some(todo) = queue.pop_front() {
        let fetch = fetch_following_redirects(fetcher, &todo.url).await;
        let doc = parse(&todo.url, &fetch);
        let mut fetch = fetch;
        drop_body(&mut fetch);
        let index = out.sitemaps.len();

        if let Some(doc) = doc.as_ref().filter(|d| d.kind == SitemapKind::Index) {
            for loc in &doc.locs {
                let Some(child) = loc.url.as_deref().and_then(|u| url::Url::parse(u).ok()) else {
                    continue;
                };
                if todo.depth >= max_index_depth {
                    out.warnings.push(SitemapWarning {
                        code: "sitemap_index_too_deep",
                        message: format!(
                            "Sitemap-Index {} liegt auf Tiefe {} (Höchsttiefe {max_index_depth}): \
                             Unter-Sitemaps werden nicht abgerufen",
                            todo.url, todo.depth
                        ),
                    });
                    break;
                }
                if seen.insert(key(&child)) {
                    queue.push_back(Todo {
                        url: child,
                        parent: Some(index),
                        depth: todo.depth + 1,
                        found_via: todo.found_via.clone(),
                    });
                } else {
                    out.warnings.push(SitemapWarning {
                        code: "sitemap_cycle",
                        message: format!(
                            "Sitemap {child} wird in {} erneut genannt (Zyklus oder Mehrfachangabe) und nur einmal abgerufen",
                            todo.url
                        ),
                    });
                }
            }
        }

        out.sitemaps.push(FetchedSitemap {
            url: todo.url.to_string(),
            parent: todo.parent,
            depth: todo.depth,
            found_via: todo.found_via,
            fetch,
            doc,
        });
    }
    out
}

/// Schlüssel zum Erkennen doppelter Sitemaps.
fn key(u: &url::Url) -> String {
    os_url::normalize::normalize(u).encoded
}

/// Ruft die URL ab und folgt bis zu [`MAX_REDIRECTS`] Weiterleitungen.
async fn fetch_following_redirects<F: Fetch + ?Sized>(
    fetcher: &F,
    start: &url::Url,
) -> FetchResult {
    let mut url = start.clone();
    let mut hops = 0;
    loop {
        let result = fetcher
            .fetch(FetchRequest {
                url: url.clone(),
                purpose: FetchPurpose::Sitemap,
                cookies: None,
                max_body_bytes: MAX_BODY_BYTES,
            })
            .await;
        let next = match &result.outcome {
            FetchOutcome::Response(r) if matches!(r.status, 301 | 302 | 303 | 307 | 308) => r
                .headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case("location"))
                .and_then(|(_, v)| result.url.join(v.trim()).ok())
                .filter(|u| matches!(u.scheme(), "http" | "https")),
            _ => None,
        };
        match next {
            Some(next) if hops < MAX_REDIRECTS => {
                hops += 1;
                url = next;
            }
            _ => return result,
        }
    }
}

/// Wertet die Antwort aus, falls sie 2xx war.
fn parse(requested: &url::Url, fetch: &FetchResult) -> Option<SitemapDoc> {
    let FetchOutcome::Response(r) = &fetch.outcome else {
        return None;
    };
    if !(200..300).contains(&r.status) {
        return None;
    }
    let content_type = r
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str());
    // Relative Angaben in Sitemaps lösen sich gegen die Endadresse der Antwort auf.
    let base = if fetch.url.as_str().is_empty() {
        requested
    } else {
        &fetch.url
    };
    Some(os_parse::parse_sitemap(base, &r.body, content_type))
}

/// Gibt den Body frei, nachdem er ausgewertet wurde.
fn drop_body(fetch: &mut FetchResult) {
    if let FetchOutcome::Response(r) = &mut fetch.outcome {
        r.body = bytes::Bytes::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;
    use std::sync::Arc;

    use os_fetch::{FetchConfig, HttpFetcher, MemorySecretStore};
    use os_model::{CrawlConfig, FixedClock, Timestamp};
    use os_testsite::TestSite;

    fn gzip(data: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    fn fetcher() -> HttpFetcher {
        let clock = Arc::new(FixedClock::new(
            Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap(),
            std::time::Duration::from_millis(1),
        ));
        let cfg = FetchConfig::from_crawl_config(
            &CrawlConfig::default(),
            &MemorySecretStore::new(),
            clock,
        )
        .unwrap();
        HttpFetcher::new(cfg).unwrap()
    }

    fn urlset(urls: &[&str]) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for u in urls {
            s.push_str(&format!("<url><loc>{u}</loc></url>\n"));
        }
        s.push_str("</urlset>\n");
        s
    }

    fn index(children: &[&str]) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for c in children {
            s.push_str(&format!("<sitemap><loc>{c}</loc></sitemap>\n"));
        }
        s.push_str("</sitemapindex>\n");
        s
    }

    fn write(dir: &Path, name: &str, content: impl AsRef<[u8]>) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    fn seed(site: &TestSite, path: &str, via: &str) -> SitemapSeed {
        SitemapSeed {
            url: url::Url::parse(&site.url(path)).unwrap(),
            found_via: via.to_string(),
        }
    }

    fn status(s: &FetchedSitemap) -> u16 {
        match &s.fetch.outcome {
            FetchOutcome::Response(r) => r.status,
            FetchOutcome::NoResponse { .. } => 0,
        }
    }

    /// Eine Site, deren Sitemaps aus Dateien in einem Temp-Verzeichnis kommen (Seiten sind `{{BASE_URL}}`-frei).
    async fn site_with(files: impl FnOnce(&Path, &str)) -> (TestSite, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        // Die Basis-URL ist erst nach dem Start bekannt; Platzhalter `{{BASE_URL}}` ersetzt die Testsite in
        // textuellen Dateien (xml) selbst. gzip-Dateien enthalten absolute Adressen und brauchen die Basis vorab.
        files(dir.path(), "{{BASE_URL}}");
        let site = TestSite::start_dir(dir.path(), 0).await.unwrap();
        (site, dir)
    }

    // spec: TECH-210
    #[tokio::test]
    async fn index_is_resolved_recursively_with_gzip_children() {
        let (site, _dir) = site_with(|d, base| {
            write(
                d,
                "sitemap_index.xml",
                index(&[
                    &format!("{base}/sitemap-pages.xml"),
                    &format!("{base}/sitemap-posts.xml.gz"),
                ]),
            );
            write(
                d,
                "sitemap-pages.xml",
                urlset(&[&format!("{base}/a"), &format!("{base}/b")]),
            );
        })
        .await;
        // Das gzip-Kind nennt absolute Adressen mit der Basis der laufenden Site.
        let dir = _dir.path();
        write(
            dir,
            "sitemap-posts.xml.gz",
            gzip(&urlset(&[
                &site.url("/p1"),
                &site.url("/p2"),
                &site.url("/p3"),
            ])),
        );

        let out = ingest(
            &fetcher(),
            vec![seed(&site, "/sitemap_index.xml", "robots")],
            3,
        )
        .await;
        assert_eq!(out.len(), 3);

        let root = &out[0];
        assert_eq!(root.parent, None);
        assert_eq!(root.depth, 0);
        assert_eq!(root.found_via, "robots");
        assert_eq!(root.doc.as_ref().unwrap().kind, SitemapKind::Index);
        assert_eq!(root.doc.as_ref().unwrap().locs.len(), 2);

        assert_eq!(out[1].url, site.url("/sitemap-pages.xml"));
        assert_eq!((out[1].parent, out[1].depth), (Some(0), 1));
        assert_eq!(out[1].found_via, "robots");
        assert_eq!(out[1].doc.as_ref().unwrap().kind, SitemapKind::Urlset);
        assert_eq!(out[1].doc.as_ref().unwrap().locs.len(), 2);

        // gzip wird anhand der Magic-Bytes entpackt; die Größe ist die entpackte (TECH-210).
        assert_eq!(out[2].url, site.url("/sitemap-posts.xml.gz"));
        let posts = out[2].doc.as_ref().unwrap();
        assert_eq!(posts.kind, SitemapKind::Urlset);
        assert_eq!(posts.locs.len(), 3);
        let plain = urlset(&[&site.url("/p1"), &site.url("/p2"), &site.url("/p3")]);
        assert_eq!(posts.size_bytes, plain.len() as u64);
        // Der Body wurde nach dem Auswerten freigegeben, die Kopfdaten bleiben.
        let FetchOutcome::Response(r) = &out[2].fetch.outcome else {
            panic!("Antwort erwartet");
        };
        assert!(r.body.is_empty());
        assert_eq!(r.status, 200);
        assert!(r.size_bytes > 0);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn index_depth_is_limited_and_deeper_indexes_warn() {
        // Kette index0 → index1 → index2 → index3 → index4 → leaf.xml (Seed auf Tiefe 0).
        let (site, _dir) = site_with(|d, base| {
            for i in 0..4 {
                write(
                    d,
                    &format!("index{i}.xml"),
                    index(&[&format!("{base}/index{}.xml", i + 1)]),
                );
            }
            write(d, "index4.xml", index(&[&format!("{base}/leaf.xml")]));
            write(d, "leaf.xml", urlset(&[&format!("{base}/x")]));
        })
        .await;

        let out =
            ingest_with_warnings(&fetcher(), vec![seed(&site, "/index0.xml", "start")], 3).await;
        let urls: Vec<_> = out.sitemaps.iter().map(|s| s.url.clone()).collect();
        assert_eq!(
            urls,
            (0..4)
                .map(|i| site.url(&format!("/index{i}.xml")))
                .collect::<Vec<_>>(),
            "Tiefe 0 … 3 werden abgerufen, index4 (Tiefe 4) nicht"
        );
        assert_eq!(out.sitemaps.last().unwrap().depth, 3);
        assert_eq!(site.count("127.0.0.1", "/index4.xml"), 0);
        assert_eq!(site.count("127.0.0.1", "/leaf.xml"), 0);
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].code, "sitemap_index_too_deep");
        assert!(out.warnings[0].message.contains("index3.xml"));

        // Tiefe 1: nur der Seed und seine Kinder.
        let out = ingest(&fetcher(), vec![seed(&site, "/index0.xml", "start")], 1).await;
        assert_eq!(out.len(), 2);
        // Tiefe 0: nur der Seed selbst.
        let out = ingest(&fetcher(), vec![seed(&site, "/index0.xml", "start")], 0).await;
        assert_eq!(out.len(), 1);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn three_index_levels_with_leaf_are_fully_read() {
        // "Index in Index" über drei Ebenen (CRAWL-060: Tiefe 3 wird verfolgt).
        let (site, _dir) = site_with(|d, base| {
            write(d, "l1.xml", index(&[&format!("{base}/l2.xml")]));
            write(d, "l2.xml", index(&[&format!("{base}/l3.xml")]));
            write(d, "l3.xml", index(&[&format!("{base}/leaf.xml")]));
            write(d, "leaf.xml", urlset(&[&format!("{base}/x")]));
        })
        .await;
        let out = ingest_with_warnings(&fetcher(), vec![seed(&site, "/l1.xml", "start")], 3).await;
        assert_eq!(out.sitemaps.len(), 4);
        assert_eq!(out.sitemaps[3].depth, 3);
        assert_eq!(out.sitemaps[3].parent, Some(2));
        assert_eq!(out.sitemaps[3].doc.as_ref().unwrap().locs.len(), 1);
        assert!(out.warnings.is_empty());
    }

    // spec: TECH-210
    #[tokio::test]
    async fn cycles_are_detected_and_fetched_once() {
        let (site, _dir) = site_with(|d, base| {
            write(
                d,
                "a.xml",
                index(&[&format!("{base}/b.xml"), &format!("{base}/a.xml")]),
            );
            write(d, "b.xml", index(&[&format!("{base}/a.xml")]));
        })
        .await;
        let out = ingest_with_warnings(&fetcher(), vec![seed(&site, "/a.xml", "config")], 3).await;
        assert_eq!(out.sitemaps.len(), 2);
        assert_eq!(site.count("127.0.0.1", "/a.xml"), 1);
        assert_eq!(site.count("127.0.0.1", "/b.xml"), 1);
        assert_eq!(out.warnings.len(), 2);
        assert!(out.warnings.iter().all(|w| w.code == "sitemap_cycle"));
    }

    // spec: TECH-210
    #[tokio::test]
    async fn duplicate_seeds_are_fetched_once_and_keep_the_first_source() {
        let (site, _dir) = site_with(|d, base| {
            write(d, "s.xml", urlset(&[&format!("{base}/a")]));
        })
        .await;
        let seeds = vec![
            seed(&site, "/s.xml", "config"),
            seed(&site, "/s.xml#frag", "robots"),
            seed(&site, "/s.xml", "robots"),
        ];
        let out = ingest(&fetcher(), seeds, 3).await;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].found_via, "config");
        assert_eq!(site.count("127.0.0.1", "/s.xml"), 1);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn invalid_loc_entries_are_counted_but_not_followed() {
        let (site, _dir) = site_with(|d, base| {
            write(
                d,
                "idx.xml",
                index(&[
                    "  not a url  ",
                    &format!("  {base}/ok.xml  "),
                    "ftp://example.test/x.xml",
                ]),
            );
            write(d, "ok.xml", urlset(&[&format!("{base}/a"), "kaputt"]));
        })
        .await;
        let out = ingest(&fetcher(), vec![seed(&site, "/idx.xml", "start")], 3).await;
        assert_eq!(out.len(), 2, "nur der gültige Eintrag wird abgerufen");
        let idx = out[0].doc.as_ref().unwrap();
        assert_eq!(idx.locs.len(), 3);
        assert_eq!(idx.invalid_entries, 2);
        // Getrimmt abgerufen.
        assert_eq!(out[1].url, site.url("/ok.xml"));
        let ok = out[1].doc.as_ref().unwrap();
        assert_eq!(ok.locs.len(), 2);
        assert_eq!(ok.invalid_entries, 1);
        assert_eq!(ok.locs.iter().filter(|l| l.url.is_some()).count(), 1);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn gzip_is_detected_by_magic_bytes_not_by_name_or_type() {
        let (site, _dir) = site_with(|_, _| {}).await;
        write(
            _dir.path(),
            "versteckt.xml",
            gzip(&urlset(&[&site.url("/a"), &site.url("/b")])),
        );
        let out = ingest(&fetcher(), vec![seed(&site, "/versteckt.xml", "list")], 3).await;
        assert_eq!(out.len(), 1);
        let doc = out[0].doc.as_ref().unwrap();
        assert_eq!(doc.kind, SitemapKind::Urlset);
        assert_eq!(doc.locs.len(), 2);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn start_url_and_listed_sitemaps_are_processed_without_crawl_linked() {
        // Ein Start-/Listen-URL-Seed wird unabhängig von `sitemaps.crawl_linked` abgerufen: `ingest` kennt die
        // Konfiguration nicht, die Entscheidung liegt bei den Seeds (robots_glue::sitemap_seeds).
        let (site, _dir) = site_with(|d, base| {
            write(
                d,
                "extra-sitemap.xml",
                urlset(&[&format!("{base}/nur-in-sitemap")]),
            );
        })
        .await;
        let out = ingest(
            &fetcher(),
            vec![
                seed(&site, "/extra-sitemap.xml", "list"),
                seed(&site, "/fehlt.xml", "config"),
            ],
            3,
        )
        .await;
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].found_via, "list");
        assert_eq!(
            out[0].doc.as_ref().unwrap().locs[0].url.as_deref(),
            Some(site.url("/nur-in-sitemap").as_str())
        );
        // 404: kein Dokument, der Abruf bleibt sichtbar.
        assert_eq!(status(&out[1]), 404);
        assert!(out[1].doc.is_none());
    }

    // spec: TECH-210
    #[tokio::test]
    async fn redirects_are_followed_and_failures_leave_no_document() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "echt.xml", urlset(&["{{BASE_URL}}/seite"]));
        write(
            dir.path(),
            "scenario.toml",
            "[[route]]\npath = \"/alt.xml\"\nstatus = 301\nlocation = \"/echt.xml\"\n\n\
             [[route]]\npath = \"/kreis-a.xml\"\nstatus = 302\nlocation = \"/kreis-b.xml\"\n\n\
             [[route]]\npath = \"/kreis-b.xml\"\nstatus = 302\nlocation = \"/kreis-a.xml\"\n\n\
             [[route]]\npath = \"/kaputt.xml\"\nstatus = 500\n",
        );
        let site = TestSite::start_dir(dir.path(), 0).await.unwrap();

        let out = ingest(
            &fetcher(),
            vec![
                seed(&site, "/alt.xml", "start"),
                seed(&site, "/kreis-a.xml", "config"),
                seed(&site, "/kaputt.xml", "config"),
            ],
            3,
        )
        .await;
        assert_eq!(out.len(), 3);
        // Weiterleitung verfolgt: angeforderte URL bleibt, Antwort ist die des Ziels.
        assert_eq!(out[0].url, site.url("/alt.xml"));
        assert_eq!(out[0].fetch.url.as_str(), site.url("/echt.xml"));
        assert_eq!(out[0].doc.as_ref().unwrap().locs.len(), 1);
        // Weiterleitungsschleife: nach MAX_REDIRECTS bleibt es bei einer 3xx-Antwort ohne Dokument.
        assert!((300..400).contains(&status(&out[1])));
        assert!(out[1].doc.is_none());
        assert_eq!(
            site.count("127.0.0.1", "/kreis-a.xml") as u32
                + site.count("127.0.0.1", "/kreis-b.xml") as u32,
            MAX_REDIRECTS + 1
        );
        // 5xx: kein Dokument.
        assert_eq!(status(&out[2]), 500);
        assert!(out[2].doc.is_none());
    }

    // spec: TECH-210
    #[tokio::test]
    async fn broken_xml_keeps_entries_read_so_far() {
        let (site, _dir) = site_with(|d, base| {
            let mut s = String::from("<?xml version=\"1.0\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">");
            for i in 0..10 {
                s.push_str(&format!("<url><loc>{base}/e{i}</loc></url>"));
            }
            s.push_str("<url><loc>");
            write(d, "broken.xml", s);
        })
        .await;
        let out = ingest(&fetcher(), vec![seed(&site, "/broken.xml", "start")], 3).await;
        let doc = out[0].doc.as_ref().unwrap();
        assert_eq!(doc.kind, SitemapKind::Urlset);
        assert_eq!(doc.locs.len(), 10);
    }

    // spec: TECH-210
    #[tokio::test]
    async fn unreachable_host_yields_no_document() {
        // Port 1 auf Loopback ist nicht belegt: Verbindung abgelehnt, keine Antwort.
        let f = fetcher();
        let out = ingest(
            &f,
            vec![SitemapSeed {
                url: url::Url::parse("http://127.0.0.1:1/s.xml").unwrap(),
                found_via: "config".to_string(),
            }],
            3,
        )
        .await;
        assert_eq!(out.len(), 1);
        assert!(out[0].doc.is_none());
        assert!(matches!(
            out[0].fetch.outcome,
            FetchOutcome::NoResponse { .. }
        ));
    }

    // spec: TECH-210
    #[test]
    fn configured_sitemap_urls_are_seeds_regardless_of_crawl_linked() {
        let mut cfg = CrawlConfig::default();
        assert!(!cfg.sitemaps.crawl_linked);
        cfg.sitemaps.urls = vec![
            " https://site.test/extra-sitemap.xml ".to_string(),
            "https://site.test/extra-sitemap.xml".to_string(),
            "kein url".to_string(),
            "ftp://site.test/x.xml".to_string(),
            String::new(),
            "https://other.test/s.xml".to_string(),
        ];
        let seeds = configured_seeds(&cfg);
        let urls: Vec<_> = seeds.iter().map(|s| s.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://site.test/extra-sitemap.xml",
                "https://other.test/s.xml"
            ]
        );
        assert!(seeds.iter().all(|s| s.found_via == "config"));
        assert!(configured_seeds(&CrawlConfig::default()).is_empty());
    }

    // spec: TECH-210
    #[tokio::test]
    async fn no_seeds_means_no_requests() {
        let out = ingest(&fetcher(), Vec::new(), 3).await;
        assert!(out.is_empty());
    }
}
