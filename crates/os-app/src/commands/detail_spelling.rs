//! detail.spellingContext (ONPAGE-153, ONPAGE-157, UI-047) und die App-Seite der Kontextaktionen des Detail-Tabs
//! „Spelling & Grammar Details“. Die Datenbankarbeit liegt in `os_analyze::tabs::content` (Lint 2.3 Regel 1).
//!
//! - **`detail.spellingContext { urlId, findingId }`:** `findingId` ist die Zeilennummer (ab 1) des Befunds im
//!   Detail-Tab in Standardreihenfolge. Antwort: Textausschnitt des Bereichs (Titel, Meta-Description bzw.
//!   gespeicherter Text des Inhaltsbereichs) um die Fundstelle, deren Zeichenpositionen im Ausschnitt und die
//!   Vorschläge (`os_analyze::tabs::content::spelling_context`).
//! - **„Ignorieren (alle)“:** `os_analyze::tabs::content::ignore_spelling_word` (Ignorierliste im Crawl), wirksam
//!   nach der Neu-Prüfung.
//! - **„Regel ignorieren“:** [`ignore_rule`] ergänzt `grammar.disabled_rules` der Crawl-Konfiguration.
//! - **„Zum Wörterbuch hinzufügen“:** [`add_to_dictionary`] schreibt in das persönliche Wörterbuch der Sprache
//!   (`<Einstellungen>/dictionaries/personal/<Code>.txt`); es gilt für alle folgenden Crawls.
//! - **Neu-Prüfung ohne Abruf:** `os_analyze::tabs::content::recheck_spelling` mit dem Prüfer [`checker`]; die
//!   Operation mit Fortschritt in der App gehört zu ONPAGE-158.
//!
//! [`on_startup`] setzt die Wörterbuchorte des Prozesses auf den Einstellungsordner der App.

use std::collections::HashSet;

use os_analyze::tabs::content::{SpellingChecked, SpellingPage};
use os_model::{CrawlConfig, UrlId, Value};
use os_parse::spelling::{self as spell, dictionary, Resources};

use super::catalog::{session, view_error};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "detail.spellingContext",
            args: "SpellingContextArgs",
            ret: "SpellingContext",
            long_running: false,
        },
        spelling_context,
    );
}

/// Wörterbuchorte des Prozesses unter dem Einstellungsordner der App (`dictionaries/`, persönliches Wörterbuch in
/// `dictionaries/personal/`).
pub fn on_startup(app: &App) {
    dictionary::configure(dictionary::under(&app.config().paths.settings));
}

fn spelling_context<'a>(
    app: &'a App,
    ctx: CallCtx,
    args: serde_json::Value,
) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: SpellingContextArgs = parse_args(args)?;
        let s = session(app, &ctx)?;
        app.read_pool()
            .run_snapshot(s.key, move |file, _| {
                if os_store::queries::url_by_id(file.conn(), UrlId(args.url_id))?.is_none() {
                    return Ok(Err(CmdError::new(ErrorCode::NotFound, "err-url-not-found")));
                }
                let found = os_analyze::tabs::content::spelling_context(
                    file,
                    &s.config,
                    UrlId(args.url_id),
                    args.finding_id,
                );
                Ok(match found {
                    Ok(Some(c)) => to_json(&SpellingContext {
                        text: c.text,
                        start: c.start,
                        end: c.end,
                        suggestions: c.suggestions,
                    }),
                    Ok(None) => Err(CmdError::new(ErrorCode::NotFound, "err-finding-not-found")
                        .with_arg("findingId", args.finding_id)),
                    Err(e) => Err(view_error(e)),
                })
            })
            .await?
    })
}

/// „Regel ignorieren“: schaltet die Grammatikregel `rule_id` in der Konfiguration ab.
pub fn ignore_rule(cfg: &mut CrawlConfig, rule_id: &str) {
    let rule_id = rule_id.trim();
    if !rule_id.is_empty() && !cfg.grammar.disabled_rules.iter().any(|r| r == rule_id) {
        cfg.grammar.disabled_rules.push(rule_id.to_string());
    }
}

/// „Zum Wörterbuch hinzufügen“: `word` im persönlichen Wörterbuch der Sprache `language` (Wert der Spalte
/// `Language`, z. B. `de-DE`) speichern.
pub fn add_to_dictionary(language: &str, word: &str, resources: &Resources) -> std::io::Result<()> {
    dictionary::add_personal_word(spell::language_code(language), word, resources)
}

/// Prüfer der Neu-Prüfung: `os_parse::spelling::check` auf den gespeicherten Texten einer Seite mit Konfiguration,
/// Ignorierliste und Wörterbuchorten. Bei ausgeschalteten Prüfungen sind alle Werte leer und es gibt keine Befunde.
pub fn checker<'a>(
    cfg: &'a CrawlConfig,
    resources: &'a Resources,
) -> impl Fn(&SpellingPage, &HashSet<String>) -> SpellingChecked + 'a {
    move |page, ignored| {
        let declared = page.declared.clone().or_else(|| {
            page.html.as_ref().and_then(|html| {
                let ct = html
                    .charset
                    .as_ref()
                    .map(|c| format!("text/html; charset={c}"));
                let (text, _) = os_parse::charset::decode(&html.bytes, ct.as_deref(), &[]);
                spell::lang::declared_html(&os_parse::dom::Document::parse(&text))
            })
        });
        let input = spell::Input {
            declared: declared.as_deref().filter(|d| !d.trim().is_empty()),
            title: page.title.as_deref(),
            description: page.description.as_deref(),
            body: &page.body,
        };
        let outcome = spell::check(&input, &cfg.spelling, &cfg.grammar, ignored, resources);
        let mut facts = outcome.facts(declared.as_deref());
        if !(cfg.spelling.enabled || cfg.grammar.enabled) {
            for (key, value) in facts.iter_mut() {
                if key.as_str() != spell::KEY_LANG_SOURCE {
                    *value = Value::Null;
                }
            }
        }
        let writes = spell::take_findings(page.url, &mut facts);
        (facts, writes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pool::CrawlKey, runtime::sessions::SessionInfo, AppConfig, WindowId};
    use os_analyze::tabs::content::{
        ignore_spelling_word, ignored_spelling_words, recheck_spelling,
    };
    use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
    use os_engine::testing::{crawl_fixture, FixtureCrawl};
    use os_model::CancelToken;
    use std::path::PathBuf;
    use std::sync::{Arc, OnceLock};

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-spell")
    }

    /// Prüfungen an, Sprachwahl mit den Defaults der Spec; Mini-Wörterbücher der Fixture über die Umgebung.
    fn spec_cfg() -> CrawlConfig {
        static INIT: OnceLock<()> = OnceLock::new();
        INIT.get_or_init(|| {
            std::env::set_var(dictionary::DICTIONARIES_ENV, fixture().join("dictionaries"));
            dictionary::configure(resources(None));
        });
        let mut cfg = CrawlConfig::default();
        cfg.spelling.enabled = true;
        cfg.grammar.enabled = true;
        cfg.pdf.store = true;
        cfg.spelling.default_language = "en-GB".into();
        cfg.spelling.default_variants = [("en", "en-GB"), ("de", "de-DE"), ("pt", "pt-PT")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        cfg
    }

    /// Wörterbuchorte der Fixture, ohne bzw. mit eigenem persönlichem Wörterbuch (nicht der globale Zustand).
    fn resources(personal: Option<PathBuf>) -> Resources {
        Resources {
            dictionary_dirs: vec![fixture().join("dictionaries")],
            personal_dir: personal,
        }
    }

    fn id(fx: &FixtureCrawl, path: &str) -> UrlId {
        let url = format!("{}{path}", fx.site.base_url());
        os_store::queries::url_by_address(fx.file.conn(), &url)
            .unwrap()
            .unwrap_or_else(|| panic!("{url}"))
            .id
    }

    /// Wert der Content-Spalte `column` für `path` im Filter `filter` (`None` = nicht im Filter).
    fn cell(
        fx: &FixtureCrawl,
        cfg: &CrawlConfig,
        filter: &str,
        path: &str,
        column: &str,
    ) -> Option<Value> {
        let mut q = ViewQuery::new("Content", filter);
        q.columns = ColumnSelection::Named(vec!["Address".into(), column.into()]);
        q.limit = 500;
        let url = Value::Text(format!("{}{path}", fx.site.base_url()));
        query_view(&fx.file, cfg, &q)
            .unwrap()
            .rows
            .into_iter()
            .find(|r| r[0] == url)
            .map(|r| r[1].clone())
    }

    fn errors(fx: &FixtureCrawl, cfg: &CrawlConfig, path: &str, column: &str) -> Value {
        cell(fx, cfg, "All", path, column).unwrap_or_else(|| panic!("{path}"))
    }

    // spec: ONPAGE-157
    // spec: ONPAGE-155
    #[tokio::test]
    async fn ignore_all_and_dictionary_take_effect_after_recheck_without_requests() {
        let cfg = spec_cfg();
        let mut fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        let requests = fx.site.requests().len();
        assert_eq!(
            errors(&fx, &cfg, "/de-errors.html", "Spelling Errors"),
            Value::Int(3)
        );
        assert!(cell(&fx, &cfg, "Spelling Errors", "/de-errors.html", "Address").is_some());

        // „Ignorieren (alle)“ auf „Rechtschreibfeler“: nach Neu-Prüfung in allen Seiten dieses Crawls weg.
        ignore_spelling_word(&fx.file, "Rechtschreibfeler").unwrap();
        assert_eq!(
            ignored_spelling_words(&fx.file).unwrap(),
            ["rechtschreibfeler"]
        );
        let none = resources(None);
        let changed = recheck_spelling(
            &mut fx.file,
            &cfg,
            &checker(&cfg, &none),
            &CancelToken::new(),
        )
        .unwrap();
        assert!(changed.contains(&id(&fx, "/de-errors.html")));
        assert_eq!(
            errors(&fx, &cfg, "/de-errors.html", "Spelling Errors"),
            Value::Int(2)
        );
        assert_eq!(
            errors(&fx, &cfg, "/fehler.pdf", "Spelling Errors"),
            Value::Int(0)
        );

        // „Zum Wörterbuch hinzufügen“ für die übrigen Fehlerwörter.
        let dir = tempfile::tempdir().unwrap();
        let personal = resources(Some(dir.path().to_path_buf()));
        add_to_dictionary("de-DE", "Kontackt", &personal).unwrap();
        add_to_dictionary("de-DE (no dictionary)", "Beschreibng", &personal).unwrap();
        assert!(dir.path().join("de-DE.txt").is_file());
        recheck_spelling(
            &mut fx.file,
            &cfg,
            &checker(&cfg, &personal),
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(
            errors(&fx, &cfg, "/de-errors.html", "Spelling Errors"),
            Value::Int(0)
        );
        assert!(cell(&fx, &cfg, "Spelling Errors", "/de-errors.html", "Address").is_none());
        let de = id(&fx, "/de-errors.html");
        assert!(
            os_analyze::tabs::content::spelling_findings_of(&fx.file, &cfg, de)
                .unwrap()
                .is_empty()
        );
        assert_eq!(fx.site.requests().len(), requests);

        // Ein neuer Crawl kennt die Ignorierliste nicht.
        let again = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        assert!(ignored_spelling_words(&again.file).unwrap().is_empty());
        assert_eq!(
            errors(&again, &cfg, "/de-errors.html", "Spelling Errors"),
            Value::Int(3)
        );
    }

    // spec: ONPAGE-154
    // spec: ONPAGE-157
    #[tokio::test]
    async fn ignoring_a_rule_removes_its_findings_after_recheck() {
        let mut cfg = spec_cfg();
        let mut fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        assert_eq!(
            errors(&fx, &cfg, "/grammar-de.html", "Grammar Errors"),
            Value::Int(7)
        );
        ignore_rule(&mut cfg, "WORD_REPEAT");
        ignore_rule(&mut cfg, "WORD_REPEAT");
        assert_eq!(cfg.grammar.disabled_rules, ["WORD_REPEAT"]);
        let none = resources(None);
        recheck_spelling(
            &mut fx.file,
            &cfg,
            &checker(&cfg, &none),
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(
            errors(&fx, &cfg, "/grammar-de.html", "Grammar Errors"),
            Value::Int(6)
        );
        assert_eq!(
            errors(&fx, &cfg, "/tesst.html", "Grammar Errors"),
            Value::Int(0)
        );
        assert_eq!(
            errors(&fx, &cfg, "/tesst.html", "Spelling Errors"),
            Value::Int(2)
        );
        // Prüfungen aus: Zähler leer, Befunde weg.
        let mut off = cfg.clone();
        off.spelling.enabled = false;
        off.grammar.enabled = false;
        recheck_spelling(
            &mut fx.file,
            &off,
            &checker(&off, &none),
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(
            errors(&fx, &cfg, "/tesst.html", "Spelling Errors"),
            Value::Null
        );
        let tesst = id(&fx, "/tesst.html");
        assert!(
            os_analyze::tabs::content::spelling_findings_of(&fx.file, &cfg, tesst)
                .unwrap()
                .is_empty()
        );
        let cancel = CancelToken::new();
        cancel.cancel();
        let err = recheck_spelling(&mut fx.file, &cfg, &checker(&cfg, &none), &cancel).unwrap_err();
        assert_eq!(view_error(err).code, ErrorCode::Cancelled);
    }

    fn app_for(fx: &FixtureCrawl, cfg: &CrawlConfig, dir: &std::path::Path) -> App {
        let app = App::new(AppConfig::new(dir)).unwrap();
        let key = CrawlKey("fx-ui-spell".into());
        let path = fx.dir.path().join("crawl.ospider");
        app.register_crawl(key.clone(), path.clone());
        app.sessions().set_current(
            WindowId("main".into()),
            SessionInfo {
                key,
                path,
                config: Arc::new(cfg.clone()),
                state: os_model::CrawlState::Completed,
                writable: false,
                engine: None,
            },
        );
        app
    }

    // spec: ONPAGE-153
    // spec: ONPAGE-157
    #[tokio::test]
    async fn spelling_context_marks_the_finding_in_its_section() {
        let cfg = spec_cfg();
        let fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let app = app_for(&fx, &cfg, dir.path());
        let call = |url: UrlId, finding: i64| {
            app.dispatch(
                CallCtx::for_window("main"),
                "detail.spellingContext",
                serde_json::json!({"urlId": url.0, "findingId": finding}),
            )
        };
        let de = id(&fx, "/de-errors.html");
        let body: SpellingContext = serde_json::from_value(call(de, 3).await.unwrap()).unwrap();
        assert_eq!(body.text, "Ein Rechtschreibfeler");
        assert_eq!((body.start, body.end), (4, 21));
        assert_eq!(
            body.suggestions.first().map(String::as_str),
            Some("Rechtschreibfehler")
        );
        let title: SpellingContext = serde_json::from_value(call(de, 1).await.unwrap()).unwrap();
        assert_eq!(
            (title.text.as_str(), title.start, title.end),
            ("Kontackt", 0, 8)
        );
        // Dasselbe Wort dreimal: die dritte Fundstelle.
        let repeat = id(&fx, "/de-repeat.html");
        let third: SpellingContext =
            serde_json::from_value(call(repeat, 3).await.unwrap()).unwrap();
        let before: String = third.text.chars().take(third.start as usize).collect();
        let marked: String = third
            .text
            .chars()
            .skip(third.start as usize)
            .take((third.end - third.start) as usize)
            .collect();
        assert_eq!(marked, "Testfeler");
        assert_eq!(before.matches("Testfeler").count(), 2);
        // Grammatik: Fundstelle über zwei Wörter.
        let tesst: SpellingContext =
            serde_json::from_value(call(id(&fx, "/tesst.html"), 2).await.unwrap()).unwrap();
        assert_eq!(
            (tesst.start, tesst.end, tesst.suggestions.clone()),
            (0, 11, vec!["Tesst".to_string()])
        );
        let missing = call(de, 9).await.unwrap_err();
        assert_eq!(missing.code, ErrorCode::NotFound);
        assert_eq!(missing.message_key, "err-finding-not-found");
        assert_eq!(
            call(UrlId(9_999), 1).await.unwrap_err().message_key,
            "err-url-not-found"
        );
    }

    // spec: ONPAGE-157
    #[tokio::test]
    async fn the_detail_view_shows_the_parity_columns_and_needs_an_active_check() {
        let cfg = spec_cfg();
        let fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let de = id(&fx, "/de-errors.html");
        let spec = serde_json::json!({"kind":"detail","detail":"spelling","urlIds":[de.0],"params":{},"sort":[]});
        let open = |app: &App, columns: serde_json::Value| {
            let args = serde_json::json!({"spec": spec, "columns": columns});
            let app = app.clone();
            async move {
                app.dispatch(CallCtx::for_window("main"), "view.open", args)
                    .await
                    .unwrap()
            }
        };
        let app = app_for(&fx, &cfg, dir.path());
        let h = open(&app, serde_json::json!([])).await;
        let keys: Vec<&str> = h["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, ["Error", "Type", "Detail", "Suggestion"]);
        assert_eq!(h["total"], 3);
        let shown = open(
            &app,
            serde_json::json!(["Error", "Page Section", "Rule ID"]),
        )
        .await;
        assert_eq!(shown["columns"].as_array().unwrap().len(), 3);
        let mut off = cfg.clone();
        off.spelling.enabled = false;
        off.grammar.enabled = false;
        let disabled = open(&app_for(&fx, &off, dir.path()), serde_json::json!([])).await;
        assert_eq!(disabled["emptyState"]["code"], "FEATURE_DISABLED");
        assert_eq!(disabled["emptyState"]["arg"], "spelling.enabled");
    }

    // spec: ONPAGE-157
    #[test]
    fn startup_points_the_dictionaries_to_the_settings_folder() {
        let settings = PathBuf::from("settings");
        let r = dictionary::under(&settings);
        assert_eq!(
            r.personal_dir,
            Some(settings.join("dictionaries").join("personal"))
        );
        assert_eq!(
            r.dictionary_dirs.last(),
            Some(&settings.join("dictionaries"))
        );
        use os_analyze::tabs::content::nth_occurrence;
        assert_eq!(nth_occurrence("ab ab xab ab", "ab", 2), Some((10, 12)));
        assert_eq!(nth_occurrence("äb ab", "ab", 0), Some((3, 5)));
    }

    /// Eine CSV-Zeile mit Feldern in Anführungszeichen (`""` = Anführungszeichen).
    fn csv_line(line: &str) -> Vec<String> {
        let (mut out, mut cell, mut quoted) = (Vec::new(), String::new(), false);
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match (c, quoted) {
                ('"', true) if chars.peek() == Some(&'"') => {
                    cell.push('"');
                    chars.next();
                }
                ('"', _) => quoted = !quoted,
                (',', false) => out.push(std::mem::take(&mut cell)),
                _ => cell.push(c),
            }
        }
        out.push(cell);
        out
    }

    fn export(fx: &FixtureCrawl, cfg: &CrawlConfig, name: &str) -> (String, Vec<Vec<String>>) {
        let dir = tempfile::tempdir().unwrap();
        let files = os_export::export_bulk(&fx.file, cfg, name, dir.path(), os_export::Format::Csv)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let path = &files[0].path;
        let text = std::fs::read_to_string(path).unwrap();
        let rows = text
            .trim_start_matches('\u{feff}')
            .lines()
            .map(csv_line)
            .collect();
        (
            path.file_name().unwrap().to_string_lossy().into_owned(),
            rows,
        )
    }

    // spec: OUT-052
    // spec: ONPAGE-165
    // spec: ONPAGE-157
    #[tokio::test]
    async fn bulk_exports_carry_the_findings_with_page_section() {
        let cfg = spec_cfg();
        let fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        let page = format!("{}/out-052.html", fx.site.base_url());
        let only = |rows: &[Vec<String>]| -> Vec<Vec<String>> {
            rows.iter().filter(|r| r[0] == page).cloned().collect()
        };
        let (_, combined) = export(&fx, &cfg, "Content:Spelling & Grammar Errors");
        assert_eq!(
            combined[0][..7],
            [
                "Address",
                "Language",
                "Type",
                "Error",
                "Detail",
                "Suggestion",
                "Page Section"
            ]
        );
        let mine = only(&combined);
        let cells: Vec<(&str, &str, &str)> = mine
            .iter()
            .map(|r| (r[2].as_str(), r[3].as_str(), r[6].as_str()))
            .collect();
        assert_eq!(
            cells,
            [
                ("Spelling", "Beispeil", "Page Title"),
                ("Spelling", "Fehlerr", "Body"),
                ("Grammar", "ist ist", "Body"),
            ]
        );
        assert!(mine.iter().all(|r| r[1] == "de-DE"));
        let (spelling_file, spelling) = export(&fx, &cfg, "Content:Spelling Errors");
        let (grammar_file, grammar) = export(&fx, &cfg, "Content:Grammar Errors");
        assert_eq!(spelling_file, "content_spelling_errors.csv");
        assert!(
            grammar_file.starts_with("content_grammar_errors"),
            "{grammar_file}"
        );
        assert_eq!(only(&spelling).len(), 2);
        assert_eq!(only(&grammar).len(), 1);
        assert_eq!(combined.len() - 1, spelling.len() - 1 + grammar.len() - 1);
    }

    // spec: OUT-025
    // spec: ONPAGE-165
    #[tokio::test]
    async fn the_summary_counts_urls_and_instances_per_error() {
        let cfg = spec_cfg();
        let fx = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
        let (_, summary) = export(&fx, &cfg, "Content:Spelling & Grammar Errors Summary");
        assert_eq!(summary[0], ["Error", "Type", "URLs", "Instances"]);
        let row = summary.iter().find(|r| r[0] == "Beispeil").unwrap();
        assert_eq!(row[1..], ["Spelling", "3", "5"]);
        let dir = tempfile::tempdir().unwrap();
        let panel = os_export::reports::save_report(
            &fx.file,
            &cfg,
            "Sidebar:Spelling & Grammar",
            dir.path(),
            os_export::Format::Csv,
        )
        .unwrap();
        let text = std::fs::read_to_string(&panel.files[0].path).unwrap();
        assert!(
            text.lines()
                .any(|l| l.trim_start_matches('\u{feff}') == "Beispeil,Spelling,3,5"),
            "{text}"
        );
    }
}
