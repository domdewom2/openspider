//! `RobotsGate`: robots.txt je Origin genau einmal abrufen, Fehler nach CRAWL-100 behandeln und
//! URLs gegen die Regeln prüfen (CRAWL-098/099/100).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_model::{CrawlConfig, RobotsMode, RobotsUnreachablePolicy, RobotsWrite};
use tokio::sync::OnceCell;
use url::Url;

use super::parser::{parse, RobotsTxt};
use super::pattern::MAX_BYTES;
use crate::http::{Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult};

/// Höchstzahl verfolgter Weiterleitungen beim Abruf der robots.txt (CRAWL-098).
const MAX_REDIRECTS: u32 = 5;

/// Konfiguration des `RobotsGate`.
#[derive(Clone)]
pub struct RobotsGateConfig {
    /// robots.txt-Token, mit dem der Crawler Gruppen auswählt.
    pub token: String,
    /// Umgang mit robots.txt.
    pub mode: RobotsMode,
    /// Verhalten bei nicht erreichbarer robots.txt.
    pub unreachable_policy: RobotsUnreachablePolicy,
    /// Wiederholungen bei Serverfehlern.
    pub retries: u32,
    /// Wartezeit zwischen Wiederholungen in Sekunden.
    pub retry_delay_s: u32,
}

impl RobotsGateConfig {
    /// Leitet die Konfiguration aus der Crawl-Konfiguration ab.
    ///
    /// Das Token kommt aus [`crate::user_agents::resolve`]: ein explizites `robots_token`, sonst das der
    /// Voreinstellung, bei einem frei eingegebenen User-Agent ohne Token `*` (nie die Preset-ID).
    pub fn from_crawl_config(cfg: &CrawlConfig) -> Self {
        RobotsGateConfig {
            token: crate::user_agents::resolve(&cfg.user_agent).robots_token,
            mode: cfg.robots.mode,
            unreachable_policy: cfg.robots.unreachable_policy,
            retries: cfg.robots.retries,
            retry_delay_s: cfg.robots.retry_delay_s,
        }
    }
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone)]
pub struct RobotsCheck {
    /// Entscheidung.
    pub decision: RobotsDecision,
    /// Nur beim Erstabruf je Origin: Abrufergebnis und Schreibmodell der robots.txt.
    ///
    /// Das `FetchResult` ist die erste Antwort des letzten Versuchs (bei einer Weiterleitung also die
    /// Weiterleitung selbst, damit die URL-Zeile der `robots.txt` ihre eigene Antwort zeigt); Endstatus,
    /// Versuche, Inhalt und Hinweis stehen im `RobotsWrite`.
    pub robots_fetch: Option<(FetchResult, RobotsWrite)>,
}

/// Entscheidung des Gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RobotsDecision {
    /// Abruf erlaubt.
    Allowed,
    /// Abruf gesperrt.
    Blocked {
        /// Treffende Regel als „Zeile n: Regeltext“.
        matched_line: Option<String>,
        /// URL der robots.txt.
        robots_url: String,
        /// Gesperrt, weil die robots.txt nicht erreichbar war.
        unreachable: bool,
    },
    /// Modus „Ignorieren, aber melden“: abrufen, Sperre nur protokollieren.
    ReportOnly {
        /// Die URL wäre gesperrt gewesen.
        would_block: bool,
        /// Treffende Regel als „Zeile n: Regeltext“.
        matched_line: Option<String>,
        /// URL der robots.txt.
        robots_url: String,
    },
}

/// Was die robots.txt eines Origins für den Crawl bedeutet.
enum OriginRules {
    /// Gültige (ggf. leere) Regeln.
    Rules(RobotsTxt),
    /// Nicht erreichbar, auch nach allen Wiederholungen.
    Unreachable,
}

struct OriginState {
    robots_url: String,
    rules: OriginRules,
}

/// Entscheidet je URL, ob robots.txt den Abruf erlaubt; ruft die robots.txt je Origin genau einmal ab.
///
/// Gleichzeitige Prüfungen für denselben Origin warten auf den einen laufenden Abruf. Die Wartezeit
/// zwischen Wiederholungen läuft über `tokio::time::sleep` und lässt sich in Tests mit
/// `#[tokio::test(start_paused = true)]` überspringen.
pub struct RobotsGate {
    cfg: RobotsGateConfig,
    origins: Mutex<HashMap<String, Arc<OnceCell<OriginState>>>>,
}

/// Endergebnis eines Abrufs samt Weiterleitungen.
enum Final {
    /// 2xx mit Body.
    Body { status: u16, text: String },
    /// Keine robots.txt vorhanden (4xx außer 429, unbrauchbare Weiterleitung): alles erlaubt.
    Unavailable { status: Option<u16>, note: String },
    /// Fehler (5xx, 429, keine Antwort, defekte Kodierung): wiederholen, danach Policy.
    Failed { status: Option<u16>, detail: String },
}

impl RobotsGate {
    /// Bereits gelesene Crawl-delay-Angabe für den wirksamen Agenten.
    pub fn crawl_delay(&self, origin: &str) -> Option<f64> {
        let map = self.origins.lock().unwrap_or_else(|e| e.into_inner());
        match &map.get(origin)?.get()?.rules {
            OriginRules::Rules(txt) => txt.crawl_delay(&self.cfg.token),
            OriginRules::Unreachable => None,
        }
    }
    /// Legt das Gate an.
    pub fn new(cfg: RobotsGateConfig) -> Self {
        RobotsGate {
            cfg,
            origins: Mutex::new(HashMap::new()),
        }
    }

    /// Prüft eine URL.
    ///
    /// * `Respect`: beim ersten Aufruf je Origin (Schema, Host, Port) wird dessen `/robots.txt` abgerufen
    ///   (`robots_fetch` ist dann `Some`), danach nur noch ausgewertet.
    /// * `Ignore`: kein Abruf, immer `Allowed`.
    /// * `IgnoreButReport`: wie `Respect`, aber die Entscheidung ist immer `ReportOnly`.
    pub async fn check<F: Fetch + ?Sized>(&self, fetcher: &F, url: &Url) -> RobotsCheck {
        if self.cfg.mode == RobotsMode::Ignore || !matches!(url.scheme(), "http" | "https") {
            return RobotsCheck {
                decision: RobotsDecision::Allowed,
                robots_fetch: None,
            };
        }
        let origin = url.origin().ascii_serialization();
        let cell = {
            let mut map = self.origins.lock().unwrap_or_else(|e| e.into_inner());
            Arc::clone(map.entry(origin).or_default())
        };
        let mut first_fetch = None;
        let state = cell
            .get_or_init(|| async {
                let (state, fetched) = self.load(fetcher, url).await;
                first_fetch = fetched;
                state
            })
            .await;

        let (blocked, matched_line, unreachable) = match &state.rules {
            OriginRules::Unreachable => (
                self.cfg.unreachable_policy == RobotsUnreachablePolicy::BlockHost,
                None,
                true,
            ),
            OriginRules::Rules(txt) => match txt.evaluate(&self.cfg.token, &path_and_query(url)) {
                Some(m) if !m.allowed => (true, Some(m.matched_line()), false),
                _ => (false, None, false),
            },
        };
        let robots_url = state.robots_url.clone();
        let decision = match (self.cfg.mode, blocked) {
            (RobotsMode::IgnoreButReport, would_block) => RobotsDecision::ReportOnly {
                would_block,
                matched_line,
                robots_url,
            },
            (_, true) => RobotsDecision::Blocked {
                matched_line,
                robots_url,
                unreachable,
            },
            (_, false) => RobotsDecision::Allowed,
        };
        RobotsCheck {
            decision,
            robots_fetch: first_fetch,
        }
    }

    /// Ruft die robots.txt des Origins von `url` ab (mit Wiederholungen) und bewertet das Ergebnis.
    async fn load<F: Fetch + ?Sized>(
        &self,
        fetcher: &F,
        url: &Url,
    ) -> (OriginState, Option<(FetchResult, RobotsWrite)>) {
        let robots_url = robots_url_for(url);
        let max_attempts = self.cfg.retries.saturating_add(1);
        let mut attempt = 0;
        let (first_result, last) = loop {
            attempt += 1;
            let (first_result, last) = self.fetch_chain(fetcher, &robots_url).await;
            if matches!(last, Final::Failed { .. }) && attempt < max_attempts {
                tracing::debug!(url = %robots_url, attempt, "robots.txt nicht erreichbar, neuer Versuch");
                tokio::time::sleep(Duration::from_secs(u64::from(self.cfg.retry_delay_s))).await;
                continue;
            }
            break (first_result, last);
        };

        let block_host = self.cfg.unreachable_policy == RobotsUnreachablePolicy::BlockHost;
        let origin = robots_url.origin().ascii_serialization();
        let mut write = RobotsWrite {
            origin,
            robots_url: robots_url.to_string(),
            fetch_status: String::new(),
            status_code: None,
            attempts: attempt,
            body: None,
            note: None,
        };
        let rules = match last {
            Final::Body { status, text } => {
                let txt = parse(&text);
                write.fetch_status = "ok".into();
                write.status_code = Some(status);
                let mut notes = Vec::new();
                if txt.rule_count() == 0 && txt.sitemaps().is_empty() {
                    notes.push(format!("robots.txt: {status} – keine Regeln"));
                }
                if let Some(delay) = txt.crawl_delay(&self.cfg.token) {
                    notes.push(format!("Crawl-delay: {delay} s (nicht angewandt)"));
                }
                if !notes.is_empty() {
                    write.note = Some(notes.join("; "));
                }
                write.body = Some(text);
                OriginRules::Rules(txt)
            }
            Final::Unavailable { status, note } => {
                write.fetch_status = "not_found".into();
                write.status_code = status;
                write.note = Some(note);
                OriginRules::Rules(RobotsTxt::default())
            }
            Final::Failed { status, detail } => {
                write.fetch_status = "unreachable".into();
                write.status_code = status;
                write.note = Some(if block_host {
                    format!("robots.txt: {detail} – Host gesperrt")
                } else {
                    "robots.txt nicht erreichbar – ohne Regeln gecrawlt".to_string()
                });
                OriginRules::Unreachable
            }
        };
        let state = OriginState {
            robots_url: write.robots_url.clone(),
            rules,
        };
        (state, Some((first_result, write)))
    }

    /// Ein Abrufversuch inklusive Weiterleitungen. Liefert die erste Antwort und das Endergebnis.
    async fn fetch_chain<F: Fetch + ?Sized>(
        &self,
        fetcher: &F,
        robots_url: &Url,
    ) -> (FetchResult, Final) {
        let mut current = robots_url.clone();
        let mut first: Option<FetchResult> = None;
        let mut redirects = 0;
        loop {
            let result = fetcher
                .fetch(FetchRequest {
                    url: current.clone(),
                    purpose: FetchPurpose::RobotsTxt,
                    cookies: None,
                    max_body_bytes: MAX_BYTES as u64,
                    attempt: 0,
                    body_limit: Default::default(),
                })
                .await;
            let step = classify(&result, &current);
            if first.is_none() {
                first = Some(result);
            }
            match step {
                Step::Done(last) => {
                    let first = first.unwrap_or_else(|| unreachable_result(robots_url));
                    return (first, last);
                }
                Step::Redirect { status, target } => {
                    redirects += 1;
                    let next = target.filter(|_| redirects <= MAX_REDIRECTS);
                    match next {
                        Some(next) => current = next,
                        None => {
                            let first = first.unwrap_or_else(|| unreachable_result(robots_url));
                            let note = if redirects > MAX_REDIRECTS {
                                format!(
                                    "robots.txt: mehr als {MAX_REDIRECTS} Weiterleitungen – keine Regeln"
                                )
                            } else {
                                format!("robots.txt: {status} ohne gültiges Ziel – keine Regeln")
                            };
                            return (
                                first,
                                Final::Unavailable {
                                    status: Some(status),
                                    note,
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

enum Step {
    Done(Final),
    Redirect { status: u16, target: Option<Url> },
}

/// Ordnet eine einzelne Antwort ein (CRAWL-100).
fn classify(result: &FetchResult, current: &Url) -> Step {
    let resp = match &result.outcome {
        FetchOutcome::NoResponse { reason, detail, .. } => {
            return Step::Done(Final::Failed {
                status: None,
                detail: if detail.is_empty() {
                    reason.status_text().to_string()
                } else {
                    format!("{} ({detail})", reason.status_text())
                },
            });
        }
        FetchOutcome::Response(resp) => resp,
    };
    let status = resp.status;
    match status {
        200..=299 => {
            if let Some(err) = &resp.decode_error {
                return Step::Done(Final::Failed {
                    status: Some(status),
                    detail: format!("{status} (Dekodierfehler: {err})"),
                });
            }
            let len = resp.body.len().min(MAX_BYTES);
            Step::Done(Final::Body {
                status,
                text: String::from_utf8_lossy(&resp.body[..len]).into_owned(),
            })
        }
        300..=399 => {
            let target = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                .and_then(|(_, v)| current.join(v.trim()).ok())
                .filter(|u| matches!(u.scheme(), "http" | "https"));
            Step::Redirect { status, target }
        }
        429 => Step::Done(Final::Failed {
            status: Some(status),
            detail: status.to_string(),
        }),
        400..=499 => Step::Done(Final::Unavailable {
            status: Some(status),
            note: format!("robots.txt: {status} – keine Regeln"),
        }),
        _ => Step::Done(Final::Failed {
            status: Some(status),
            detail: status.to_string(),
        }),
    }
}

/// Platzhalter, falls ein Fetcher nie geantwortet hat (kann nicht vorkommen, hält die Typen einfach).
fn unreachable_result(url: &Url) -> FetchResult {
    FetchResult {
        url: url.clone(),
        started_at: os_model::Timestamp::UNIX_EPOCH,
        outcome: FetchOutcome::NoResponse {
            reason: os_model::NoResponseReason::ConnectionError,
            detail: String::new(),
            elapsed_ms: 0,
        },
    }
}

/// `scheme://host[:port]/robots.txt` für den Origin der URL.
fn robots_url_for(url: &Url) -> Url {
    let mut u = url.clone();
    u.set_path("/robots.txt");
    u.set_query(None);
    u.set_fragment(None);
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u
}

/// Pfad mit Query, wie ihn die Regeln vergleichen.
fn path_and_query(url: &Url) -> String {
    match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;

    use bytes::Bytes;
    use os_model::{NoResponseReason, Timestamp};
    use tokio::time::Instant;

    use super::*;
    use crate::http::HttpResponse;

    /// Skriptierte Antwort des Mock-Fetchers.
    #[derive(Clone)]
    enum Reply {
        /// Status, Content-Type, Body.
        Body(u16, &'static str, String),
        /// Weiterleitung mit Status und Location.
        Redirect(u16, String),
        /// Redirect ohne Location-Header.
        RedirectWithoutTarget(u16),
        /// Keine Antwort.
        Fail(NoResponseReason),
    }

    fn ok(body: &str) -> Reply {
        Reply::Body(200, "text/plain", body.to_string())
    }

    fn redirect(code: u16, location: &str) -> Reply {
        Reply::Redirect(code, location.to_string())
    }

    fn status(code: u16) -> Reply {
        Reply::Body(code, "text/plain", String::new())
    }

    /// Antwortet je URL nach Skript (der letzte Eintrag wiederholt sich) und protokolliert Abrufe samt
    /// (virtueller) Zeit.
    #[derive(Default)]
    struct Mock {
        script: Mutex<HashMap<String, VecDeque<Reply>>>,
        calls: Mutex<Vec<(String, Instant)>>,
    }

    impl Mock {
        fn with(entries: &[(&str, Vec<Reply>)]) -> Self {
            let mock = Mock::default();
            for (url, replies) in entries {
                mock.script
                    .lock()
                    .unwrap()
                    .insert((*url).to_string(), replies.iter().cloned().collect());
            }
            mock
        }

        fn urls(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .map(|(u, _)| u.clone())
                .collect()
        }

        fn times(&self) -> Vec<Instant> {
            self.calls.lock().unwrap().iter().map(|(_, t)| *t).collect()
        }

        fn count(&self, url: &str) -> usize {
            self.urls().iter().filter(|u| *u == url).count()
        }
    }

    impl Fetch for Mock {
        fn fetch(
            &self,
            req: FetchRequest,
        ) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
            Box::pin(async move {
                assert_eq!(req.purpose, FetchPurpose::RobotsTxt);
                assert!(req.max_body_bytes >= 500 * 1024);
                let key = req.url.to_string();
                self.calls
                    .lock()
                    .unwrap()
                    .push((key.clone(), Instant::now()));
                let reply = {
                    let mut script = self.script.lock().unwrap();
                    match script.get_mut(&key) {
                        Some(q) if q.len() > 1 => q.pop_front(),
                        Some(q) => q.front().cloned(),
                        None => None,
                    }
                }
                .unwrap_or_else(|| status(404));
                let outcome = match reply {
                    Reply::Body(code, ct, body) => {
                        FetchOutcome::Response(response(code, vec![("content-type", ct)], body))
                    }
                    Reply::Redirect(code, loc) => FetchOutcome::Response(response(
                        code,
                        vec![("location", loc.as_str())],
                        String::new(),
                    )),
                    Reply::RedirectWithoutTarget(code) => {
                        FetchOutcome::Response(response(code, vec![], String::new()))
                    }
                    Reply::Fail(reason) => FetchOutcome::NoResponse {
                        reason,
                        detail: "Mock".to_string(),
                        elapsed_ms: 1,
                    },
                };
                FetchResult {
                    url: req.url,
                    started_at: Timestamp::UNIX_EPOCH,
                    outcome,
                }
            })
        }
    }

    fn response(code: u16, headers: Vec<(&str, &str)>, body: String) -> HttpResponse {
        let size = body.len() as u64;
        HttpResponse {
            status: code,
            reason_phrase: None,
            status_text: String::new(),
            http_version: "HTTP/1.1".to_string(),
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Bytes::from(body),
            size_bytes: size,
            transferred_bytes: size,
            response_time_ms: 1,
            truncated: false,
            decode_error: None,
            md5_hex: String::new(),
            request_headers: Vec::new(),
            set_cookies: Vec::new(),
            content_length: None,
            untruncated_bytes: None,
            synthetic: None,
            attempts: 1,
        }
    }

    fn cfg(mode: RobotsMode, policy: RobotsUnreachablePolicy) -> RobotsGateConfig {
        RobotsGateConfig {
            token: "OpenSpider".to_string(),
            mode,
            unreachable_policy: policy,
            retries: 2,
            retry_delay_s: 5,
        }
    }

    fn respect() -> RobotsGate {
        RobotsGate::new(cfg(RobotsMode::Respect, RobotsUnreachablePolicy::BlockHost))
    }

    fn allow_all() -> RobotsGate {
        RobotsGate::new(cfg(RobotsMode::Respect, RobotsUnreachablePolicy::AllowAll))
    }

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    const ROBOTS: &str = "http://h.test/robots.txt";
    const SEITE: &str = "http://h.test/seite";

    fn is_unreachable_block(d: &RobotsDecision) -> bool {
        matches!(
            d,
            RobotsDecision::Blocked {
                matched_line: None,
                unreachable: true,
                ..
            }
        )
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn unreachable_block_host_after_two_retries_five_seconds_apart() {
        let failures = [
            ("500", status(500)),
            ("503", status(503)),
            ("429", status(429)),
            ("599", status(599)),
            ("Timeout", Reply::Fail(NoResponseReason::ConnectionTimeout)),
            (
                "Antwort-Timeout",
                Reply::Fail(NoResponseReason::ResponseTimeout),
            ),
            ("Abbruch", Reply::Fail(NoResponseReason::ConnectionError)),
            ("DNS", Reply::Fail(NoResponseReason::DnsLookupFailed)),
            (
                "Verbindung verweigert",
                Reply::Fail(NoResponseReason::ConnectionRefused),
            ),
            ("TLS", Reply::Fail(NoResponseReason::TlsError)),
        ];
        for (name, reply) in failures {
            let mock = Mock::with(&[(ROBOTS, vec![reply])]);
            let gate = respect();
            let t0 = Instant::now();
            let check = gate.check(&mock, &u(SEITE)).await;
            assert!(is_unreachable_block(&check.decision), "{name}: {check:?}");
            assert_eq!(
                mock.count(ROBOTS),
                3,
                "{name}: 1 Versuch + 2 Wiederholungen"
            );
            assert_eq!(mock.urls().len(), 3, "{name}: nichts anderes abgerufen");
            let t = mock.times();
            assert!(t[1] - t[0] >= Duration::from_secs(5), "{name}");
            assert!(t[2] - t[1] >= Duration::from_secs(5), "{name}");
            assert!(Instant::now() - t0 >= Duration::from_secs(10), "{name}");
            let (_, write) = check.robots_fetch.expect("Erstabruf");
            assert_eq!(write.fetch_status, "unreachable", "{name}");
            assert_eq!(write.attempts, 3, "{name}");
            assert!(
                write
                    .note
                    .as_deref()
                    .unwrap_or("")
                    .ends_with("Host gesperrt"),
                "{name}: {write:?}"
            );
            // Weitere URLs desselben Hosts: gesperrt, ohne neuen Abruf.
            let again = gate.check(&mock, &u("http://h.test/andere")).await;
            assert!(is_unreachable_block(&again.decision), "{name}");
            assert!(again.robots_fetch.is_none());
            assert_eq!(mock.count(ROBOTS), 3, "{name}");
        }
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn unreachable_allow_all_policy_crawls_without_rules() {
        let mock = Mock::with(&[(ROBOTS, vec![status(503)])]);
        let gate = allow_all();
        let check = gate.check(&mock, &u(SEITE)).await;
        assert_eq!(check.decision, RobotsDecision::Allowed);
        assert_eq!(mock.count(ROBOTS), 3);
        let (_, write) = check.robots_fetch.expect("Erstabruf");
        assert_eq!(write.fetch_status, "unreachable");
        assert_eq!(
            write.note.as_deref(),
            Some("robots.txt nicht erreichbar – ohne Regeln gecrawlt")
        );
        let t = mock.times();
        assert!(t[1] - t[0] >= Duration::from_secs(5));
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn second_attempt_success_applies_rules() {
        let mock = Mock::with(&[(
            ROBOTS,
            vec![status(503), ok("User-agent: *\nDisallow: /seite\n")],
        )]);
        let gate = respect();
        let check = gate.check(&mock, &u(SEITE)).await;
        assert_eq!(
            check.decision,
            RobotsDecision::Blocked {
                matched_line: Some("Zeile 2: Disallow: /seite".to_string()),
                robots_url: ROBOTS.to_string(),
                unreachable: false,
            }
        );
        assert_eq!(mock.count(ROBOTS), 2);
        let (_, write) = check.robots_fetch.expect("Erstabruf");
        assert_eq!((write.fetch_status.as_str(), write.attempts), ("ok", 2));
        assert_eq!(write.status_code, Some(200));
        // Eine freie Seite desselben Hosts wird normal gecrawlt.
        let free = gate.check(&mock, &u("http://h.test/frei")).await;
        assert_eq!(free.decision, RobotsDecision::Allowed);
        assert_eq!(mock.count(ROBOTS), 2);
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn client_errors_allow_everything_without_retry() {
        for code in [400, 401, 403, 404, 410, 418, 451, 499] {
            let mock = Mock::with(&[(ROBOTS, vec![status(code)])]);
            for gate in [respect(), allow_all()] {
                let check = gate.check(&mock, &u(SEITE)).await;
                assert_eq!(check.decision, RobotsDecision::Allowed, "{code}");
                let (_, write) = check.robots_fetch.expect("Erstabruf");
                assert_eq!(write.fetch_status, "not_found", "{code}");
                assert_eq!(write.status_code, Some(code));
                assert_eq!(write.attempts, 1);
                assert_eq!(
                    write.note,
                    Some(format!("robots.txt: {code} – keine Regeln"))
                );
            }
            assert_eq!(mock.count(ROBOTS), 2, "{code}: je Gate genau ein Abruf");
        }
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn empty_and_html_bodies_allow_everything() {
        let html = Reply::Body(
            200,
            "text/html; charset=utf-8",
            "<!doctype html><html><body><h1>Nicht gefunden</h1></body></html>".to_string(),
        );
        for reply in [ok(""), html, Reply::Body(204, "", String::new())] {
            let mock = Mock::with(&[(ROBOTS, vec![reply])]);
            let check = respect().check(&mock, &u(SEITE)).await;
            assert_eq!(check.decision, RobotsDecision::Allowed);
            assert_eq!(mock.count(ROBOTS), 1);
            let (_, write) = check.robots_fetch.expect("Erstabruf");
            assert_eq!(write.fetch_status, "ok");
            assert!(write
                .note
                .as_deref()
                .unwrap_or("")
                .ends_with("keine Regeln"));
        }
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn redirect_to_error_is_treated_like_that_error() {
        let to_404 = Mock::with(&[
            (ROBOTS, vec![redirect(301, "/gone")]),
            ("http://h.test/gone", vec![status(404)]),
        ]);
        let check = respect().check(&to_404, &u(SEITE)).await;
        assert_eq!(check.decision, RobotsDecision::Allowed);
        assert_eq!(to_404.urls().len(), 2);

        let to_503 = Mock::with(&[
            (ROBOTS, vec![redirect(302, "/down")]),
            ("http://h.test/down", vec![status(503)]),
        ]);
        let check = respect().check(&to_503, &u(SEITE)).await;
        assert!(is_unreachable_block(&check.decision), "{check:?}");
        assert_eq!(to_503.count(ROBOTS), 3);
        assert_eq!(to_503.count("http://h.test/down"), 3);
        let t = to_503.times();
        assert!(
            t[2] - t[1] >= Duration::from_secs(5),
            "Abstand zwischen den Versuchen"
        );

        let to_fail = Mock::with(&[
            (ROBOTS, vec![redirect(307, "https://other.test/robots.txt")]),
            (
                "https://other.test/robots.txt",
                vec![Reply::Fail(NoResponseReason::DnsLookupFailed)],
            ),
        ]);
        let check = respect().check(&to_fail, &u(SEITE)).await;
        assert!(is_unreachable_block(&check.decision));
    }

    fn chain(hops: usize, last: Reply) -> Mock {
        let url = |i: usize| {
            if i == 0 {
                ROBOTS.to_string()
            } else {
                format!("http://h.test/hop{i}")
            }
        };
        let mut entries: Vec<(String, Vec<Reply>)> = (0..hops)
            .map(|i| (url(i), vec![redirect(301, &format!("/hop{}", i + 1))]))
            .collect();
        entries.push((url(hops), vec![last]));
        let refs: Vec<(&str, Vec<Reply>)> = entries
            .iter()
            .map(|(u, r)| (u.as_str(), r.clone()))
            .collect();
        Mock::with(&refs)
    }

    // spec: CRAWL-098, CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn follows_five_redirects_but_not_six() {
        let rules = "User-agent: *\nDisallow: /seite\n";
        let five = chain(5, ok(rules));
        let check = respect().check(&five, &u(SEITE)).await;
        assert!(matches!(
            check.decision,
            RobotsDecision::Blocked {
                unreachable: false,
                ..
            }
        ));
        assert_eq!(five.urls().len(), 6);

        // Mehr als 5 Weiterleitungen: wie 404 (alles erlaubt), kein erneuter Versuch.
        let six = chain(6, ok(rules));
        let check = respect().check(&six, &u(SEITE)).await;
        assert_eq!(check.decision, RobotsDecision::Allowed);
        assert_eq!(
            six.urls().len(),
            6,
            "der sechste Hop wird nicht mehr verfolgt"
        );
        let (first, write) = check.robots_fetch.expect("Erstabruf");
        assert_eq!(write.fetch_status, "not_found");
        assert_eq!(write.attempts, 1);
        // `FetchResult` ist die erste Antwort: die Weiterleitung der robots.txt selbst.
        assert_eq!(first.url.as_str(), ROBOTS);
        assert!(matches!(first.outcome, FetchOutcome::Response(ref r) if r.status == 301));
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn redirect_targets_are_resolved_and_may_leave_the_host() {
        let mock = Mock::with(&[
            (ROBOTS, vec![redirect(308, "https://cdn.test/a/robots.txt")]),
            (
                "https://cdn.test/a/robots.txt",
                vec![redirect(302, "../real.txt")],
            ),
            (
                "https://cdn.test/real.txt",
                vec![ok("User-agent: *\nDisallow: /seite\n")],
            ),
        ]);
        let check = respect().check(&mock, &u(SEITE)).await;
        assert!(matches!(check.decision, RobotsDecision::Blocked { .. }));
        assert_eq!(mock.urls().len(), 3);
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn redirect_without_usable_target_means_no_robots_txt() {
        for reply in [
            Reply::RedirectWithoutTarget(301),
            Reply::RedirectWithoutTarget(304),
            redirect(301, "ftp://h.test/robots.txt"),
            redirect(301, "http://[::bad"),
        ] {
            let mock = Mock::with(&[(ROBOTS, vec![reply])]);
            let check = respect().check(&mock, &u(SEITE)).await;
            assert_eq!(check.decision, RobotsDecision::Allowed);
            assert_eq!(mock.urls().len(), 1);
        }
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn any_text_content_type_is_read_as_robots_txt() {
        for ct in ["text/plain", "text/html", "application/octet-stream", ""] {
            let mock = Mock::with(&[(
                ROBOTS,
                vec![Reply::Body(
                    200,
                    ct,
                    "User-agent: *\nDisallow: /seite\n".to_string(),
                )],
            )]);
            let check = respect().check(&mock, &u(SEITE)).await;
            assert!(
                matches!(check.decision, RobotsDecision::Blocked { .. }),
                "{ct:?}"
            );
        }
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn fetches_each_origin_exactly_once_and_returns_first_fetch_only() {
        let mock = Mock::with(&[
            (
                ROBOTS,
                vec![ok("User-agent: openspider\nDisallow: /privat/\n")],
            ),
            ("http://h.test:8080/robots.txt", vec![ok("")]),
            ("https://h.test/robots.txt", vec![ok("")]),
            ("http://sub.h.test/robots.txt", vec![ok("")]),
        ]);
        let gate = respect();
        let first = gate.check(&mock, &u("http://h.test/privat/a")).await;
        assert!(matches!(first.decision, RobotsDecision::Blocked { .. }));
        let (res, write) = first.robots_fetch.expect("Erstabruf");
        assert_eq!(res.url.as_str(), ROBOTS);
        assert_eq!(write.origin, "http://h.test");
        assert_eq!(write.robots_url, ROBOTS);
        assert_eq!(
            write.body.as_deref(),
            Some("User-agent: openspider\nDisallow: /privat/\n")
        );
        for p in ["/a", "/privat/b?x=1", "/robots.txt"] {
            let c = gate.check(&mock, &u(&format!("http://h.test{p}"))).await;
            assert!(c.robots_fetch.is_none(), "{p}");
        }
        assert_eq!(mock.count(ROBOTS), 1);
        // Anderer Port, anderes Schema, andere Subdomain: eigene robots.txt.
        for other in [
            "http://h.test:8080/x",
            "https://h.test/x",
            "http://sub.h.test/x",
        ] {
            let c = gate.check(&mock, &u(other)).await;
            assert!(c.robots_fetch.is_some(), "{other}");
            assert_eq!(c.decision, RobotsDecision::Allowed, "{other}");
        }
        assert_eq!(mock.urls().len(), 4);
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn concurrent_checks_share_one_fetch() {
        let mock = Mock::with(&[(ROBOTS, vec![ok("User-agent: *\nDisallow: /privat/\n")])]);
        let gate = respect();
        let urls: Vec<Url> = (0..16)
            .map(|i| u(&format!("http://h.test/privat/{i}")))
            .collect();
        let checks =
            futures_util::future::join_all(urls.iter().map(|x| gate.check(&mock, x))).await;
        assert_eq!(mock.count(ROBOTS), 1);
        assert_eq!(
            checks.iter().filter(|c| c.robots_fetch.is_some()).count(),
            1
        );
        assert!(checks
            .iter()
            .all(|c| matches!(c.decision, RobotsDecision::Blocked { .. })));
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn matches_path_with_query_and_uses_configured_token() {
        let body = "User-agent: *\nDisallow: /\nUser-agent: spezial\nDisallow: /*?sessionid=\n";
        let mock = Mock::with(&[(ROBOTS, vec![ok(body)])]);
        let mut c = cfg(RobotsMode::Respect, RobotsUnreachablePolicy::BlockHost);
        c.token = "Spezial".to_string();
        let gate = RobotsGate::new(c);
        let blocked = gate.check(&mock, &u("http://h.test/a?sessionid=1")).await;
        assert!(matches!(
            blocked.decision,
            RobotsDecision::Blocked { matched_line: Some(ref l), .. } if l == "Zeile 4: Disallow: /*?sessionid="
        ));
        let free = gate.check(&mock, &u("http://h.test/a?x=1#anker")).await;
        assert_eq!(free.decision, RobotsDecision::Allowed);
    }

    // spec: CRAWL-099, CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn ignore_mode_fetches_nothing() {
        for body in [status(500), ok("User-agent: *\nDisallow: /\n")] {
            let mock = Mock::with(&[(ROBOTS, vec![body])]);
            let gate = RobotsGate::new(cfg(RobotsMode::Ignore, RobotsUnreachablePolicy::BlockHost));
            for p in ["/seite", "/privat/x", "/robots.txt"] {
                let c = gate.check(&mock, &u(&format!("http://h.test{p}"))).await;
                assert_eq!(c.decision, RobotsDecision::Allowed);
                assert!(c.robots_fetch.is_none());
            }
            assert!(mock.urls().is_empty(), "Modus Ignorieren ruft nichts ab");
        }
    }

    // spec: CRAWL-099
    #[tokio::test(start_paused = true)]
    async fn ignore_but_report_returns_report_only() {
        let body = "User-agent: *\nAllow: /privat/oeffentlich\nDisallow: /privat/\n";
        let mock = Mock::with(&[(ROBOTS, vec![ok(body)])]);
        let gate = RobotsGate::new(cfg(
            RobotsMode::IgnoreButReport,
            RobotsUnreachablePolicy::BlockHost,
        ));
        let blocked = gate.check(&mock, &u("http://h.test/privat/x")).await;
        assert_eq!(
            blocked.decision,
            RobotsDecision::ReportOnly {
                would_block: true,
                matched_line: Some("Zeile 3: Disallow: /privat/".to_string()),
                robots_url: ROBOTS.to_string(),
            }
        );
        assert!(blocked.robots_fetch.is_some());
        for p in ["/privat/oeffentlich", "/frei"] {
            let c = gate.check(&mock, &u(&format!("http://h.test{p}"))).await;
            assert_eq!(
                c.decision,
                RobotsDecision::ReportOnly {
                    would_block: false,
                    matched_line: None,
                    robots_url: ROBOTS.to_string(),
                },
                "{p}"
            );
            assert!(c.robots_fetch.is_none());
        }
        assert_eq!(mock.count(ROBOTS), 1);
    }

    // spec: CRAWL-099, CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn ignore_but_report_with_unreachable_robots_txt() {
        let mock = Mock::with(&[(ROBOTS, vec![status(500)])]);
        let block = RobotsGate::new(cfg(
            RobotsMode::IgnoreButReport,
            RobotsUnreachablePolicy::BlockHost,
        ));
        let c = block.check(&mock, &u(SEITE)).await;
        assert!(matches!(
            c.decision,
            RobotsDecision::ReportOnly {
                would_block: true,
                matched_line: None,
                ..
            }
        ));
        let allow = RobotsGate::new(cfg(
            RobotsMode::IgnoreButReport,
            RobotsUnreachablePolicy::AllowAll,
        ));
        let c = allow.check(&mock, &u(SEITE)).await;
        assert!(matches!(
            c.decision,
            RobotsDecision::ReportOnly {
                would_block: false,
                ..
            }
        ));
    }

    // spec: CRAWL-100
    #[tokio::test(start_paused = true)]
    async fn retries_and_delay_come_from_the_configuration() {
        let mock = Mock::with(&[(ROBOTS, vec![status(500)])]);
        let mut c = cfg(RobotsMode::Respect, RobotsUnreachablePolicy::BlockHost);
        c.retries = 0;
        let check = RobotsGate::new(c).check(&mock, &u(SEITE)).await;
        assert!(is_unreachable_block(&check.decision));
        assert_eq!(mock.urls().len(), 1);

        let mock = Mock::with(&[(ROBOTS, vec![status(500)])]);
        let mut c = cfg(RobotsMode::Respect, RobotsUnreachablePolicy::BlockHost);
        c.retries = 4;
        c.retry_delay_s = 1;
        let t0 = Instant::now();
        let _ = RobotsGate::new(c).check(&mock, &u(SEITE)).await;
        assert_eq!(mock.urls().len(), 5);
        assert_eq!(Instant::now() - t0, Duration::from_secs(4));
    }

    // spec: CRAWL-099, CRAWL-114
    #[test]
    fn config_is_derived_from_crawl_config() {
        let mut crawl = CrawlConfig::default();
        crawl.robots.mode = RobotsMode::IgnoreButReport;
        crawl.robots.unreachable_policy = RobotsUnreachablePolicy::AllowAll;
        crawl.robots.retries = 7;
        crawl.robots.retry_delay_s = 3;
        crawl.user_agent.robots_token = Some("MeinBot".to_string());
        let g = RobotsGateConfig::from_crawl_config(&crawl);
        assert_eq!(g.token, "MeinBot");
        assert_eq!(g.mode, RobotsMode::IgnoreButReport);
        assert_eq!(g.unreachable_policy, RobotsUnreachablePolicy::AllowAll);
        assert_eq!((g.retries, g.retry_delay_s), (7, 3));

        // Ohne explizites Token gilt das der Voreinstellung (nie die Preset-ID).
        crawl.user_agent.robots_token = None;
        crawl.user_agent.preset = "googlebot-smartphone".to_string();
        assert_eq!(
            RobotsGateConfig::from_crawl_config(&crawl).token,
            "Googlebot"
        );

        // Eigener User-Agent ohne Token: nur die Gruppe `*`.
        crawl.user_agent.preset = "custom".to_string();
        crawl.user_agent.http = Some("MeinCrawler/1.0".to_string());
        assert_eq!(RobotsGateConfig::from_crawl_config(&crawl).token, "*");

        // Ein explizites Token gewinnt auch bei `custom`.
        crawl.user_agent.robots_token = Some("MeinBot".to_string());
        assert_eq!(RobotsGateConfig::from_crawl_config(&crawl).token, "MeinBot");
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn crawl_delay_is_reported_in_note_but_not_applied() {
        let body = "User-agent: *\nCrawl-delay: 5\nDisallow: /x\n";
        let mock = Mock::with(&[(ROBOTS, vec![ok(body)])]);
        let t0 = Instant::now();
        let c = respect().check(&mock, &u("http://h.test/y")).await;
        assert_eq!(c.decision, RobotsDecision::Allowed);
        let (_, write) = c.robots_fetch.expect("Erstabruf");
        assert_eq!(
            write.note.as_deref(),
            Some("Crawl-delay: 5 s (nicht angewandt)")
        );
        assert_eq!(Instant::now(), t0, "Crawl-delay verzögert nichts");

        // Mit vorhandenem Hinweistext (nur Crawl-delay, keine Regeln) wird per „; “ verbunden.
        let mock = Mock::with(&[(ROBOTS, vec![ok("User-agent: *\nCrawl-delay: 0.5\n")])]);
        let c = respect().check(&mock, &u("http://h.test/y")).await;
        let (_, write) = c.robots_fetch.expect("Erstabruf");
        assert_eq!(
            write.note.as_deref(),
            Some("robots.txt: 200 – keine Regeln; Crawl-delay: 0.5 s (nicht angewandt)")
        );

        // Ohne Crawl-delay bleibt der Hinweis leer.
        let mock = Mock::with(&[(ROBOTS, vec![ok("User-agent: *\nDisallow: /x\n")])]);
        let c = respect().check(&mock, &u("http://h.test/y")).await;
        assert_eq!(c.robots_fetch.expect("Erstabruf").1.note, None);
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn non_http_schemes_are_not_checked() {
        let mock = Mock::default();
        let c = respect().check(&mock, &u("ftp://h.test/x")).await;
        assert_eq!(c.decision, RobotsDecision::Allowed);
        assert!(mock.urls().is_empty());
    }

    // spec: CRAWL-098
    #[tokio::test(start_paused = true)]
    async fn huge_body_is_cut_to_500_kib_and_does_not_crash() {
        let mut big = String::from("User-agent: *\nDisallow: /vorne/\n");
        while big.len() < 3 * 1024 * 1024 {
            big.push_str("# Fuellzeile\n");
        }
        big.push_str("Disallow: /hinten/\n");
        let mock = Mock::with(&[(ROBOTS, vec![ok(&big)])]);
        let gate = respect();
        let c = gate.check(&mock, &u("http://h.test/vorne/a")).await;
        assert!(matches!(c.decision, RobotsDecision::Blocked { .. }));
        let (_, write) = c.robots_fetch.expect("Erstabruf");
        assert!(write.body.expect("Body").len() <= 500 * 1024);
        let c = gate.check(&mock, &u("http://h.test/hinten/a")).await;
        assert_eq!(c.decision, RobotsDecision::Allowed);
    }
}
