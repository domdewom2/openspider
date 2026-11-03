//! Senden nach Zustimmung (OUT-247): nur nach einem Klick auf „Senden“, nie automatisch. `POST` an
//! `https://openspider.de/api/crash` über die app-weite Proxy-Einstellung (CRAWL-122), Zeitlimit 15 s, kein
//! automatischer Wiederholversuch. Läuft auf der Tokio-Laufzeit der App (nicht im UI-Thread) und berührt keinen Crawl.

use super::service::{self, CrashService};
use super::shared::{self, Report};
use crate::dto::{CmdError, CrashSendArgs, CrashSendFailure, CrashSendOutcome, CrashSendResult};
use crate::App;
use std::time::Duration;

/// Ziel der Berichte.
pub const ENDPOINT: &str = "https://openspider.de/api/crash";
/// Zeitlimit einer Sendung.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// Fester User-Agent ohne Kennung.
pub const USER_AGENT: &str = "OpenSpider-CrashReport";
/// Bei mehreren Berichten werden höchstens so viele (die neuesten) gesendet.
pub const MAX_BATCH: usize = 5;
/// Längen der optionalen Felder.
pub const MAX_NOTE_CHARS: usize = 2000;
pub const MAX_EMAIL_CHARS: usize = 254;

/// Ziel-URL: in Debug-Builds über `OPENSPIDER_CRASH_ENDPOINT` ersetzbar (E2E gegen einen Mock), sonst fest.
fn endpoint(app: &App) -> (String, Duration) {
    let (overridden, timeout) = service::transport(app);
    let env = if cfg!(debug_assertions) {
        std::env::var("OPENSPIDER_CRASH_ENDPOINT")
            .ok()
            .filter(|e| !e.is_empty())
    } else {
        None
    };
    (
        overridden.or(env).unwrap_or_else(|| ENDPOINT.to_string()),
        timeout.unwrap_or(TIMEOUT),
    )
}

/// Einfache Prüfung einer E-Mail-Adresse (ein `@`, Punkt in der Domain, keine Leer- oder Steuerzeichen).
pub fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.chars().count() <= MAX_EMAIL_CHARS
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Körper der Anfrage: `{"report": <Bericht>, "note": "…", "email": "…"}`; `report` ist Byte für Byte der
/// gespeicherte (und angezeigte) Text.
pub fn body(report_text: &str, note: &str, email: &str) -> String {
    format!(
        "{{\"report\":{report_text},\"note\":{},\"email\":{}}}",
        serde_json::Value::from(note),
        serde_json::Value::from(email)
    )
}

/// Prüft die optionalen Felder.
pub fn validate(args: &CrashSendArgs) -> Result<(), CmdError> {
    if args.note.chars().count() > MAX_NOTE_CHARS {
        return Err(CmdError::invalid_input("err-crash-note-too-long", "note")
            .with_arg("max", MAX_NOTE_CHARS));
    }
    let email = args.email.trim();
    if !email.is_empty() && !valid_email(email) {
        return Err(CmdError::invalid_input("err-crash-email-invalid", "email"));
    }
    Ok(())
}

/// Referenz des Servers: kurze Kennung aus Buchstaben und Ziffern.
fn valid_ref(r: &str) -> bool {
    (4..=32).contains(&r.len()) && r.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Sendet einen Text an `url`; liefert die Referenz.
pub async fn post(
    app: &App,
    url: &str,
    timeout: Duration,
    body: String,
) -> Result<String, CrashSendFailure> {
    let network = app.settings_snapshot().network;
    let mut proxy_password = None;
    if network.proxy.username.is_some() {
        let secrets = app.config().secrets.clone();
        proxy_password = app
            .handle()
            .spawn_blocking(move || secrets.try_get(os_fetch::net::PROXY_PASSWORD))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten();
    }
    let request = async {
        let client =
            os_fetch::net::client_builder_with_password(&network, proxy_password.as_deref())
                .user_agent(USER_AGENT)
                .referer(false)
                .redirect(reqwest::redirect::Policy::none())
                .timeout(timeout)
                .build()
                .map_err(|_| CrashSendFailure::Network)?;
        let mut response = client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    CrashSendFailure::Timeout
                } else {
                    CrashSendFailure::Network
                }
            })?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(CrashSendFailure::Http(status));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            if e.is_timeout() {
                CrashSendFailure::Timeout
            } else {
                CrashSendFailure::Network
            }
        })? {
            if bytes.len() + chunk.len() > 4096 {
                return Err(CrashSendFailure::Response);
            }
            bytes.extend_from_slice(&chunk);
        }
        let answer: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| CrashSendFailure::Response)?;
        match (answer["ok"].as_bool(), answer["ref"].as_str()) {
            (Some(true), Some(r)) if valid_ref(r) => Ok(r.to_string()),
            _ => Err(CrashSendFailure::Response),
        }
    };
    match tokio::time::timeout(timeout + Duration::from_secs(1), request).await {
        Ok(result) => result,
        Err(_) => Err(CrashSendFailure::Timeout),
    }
}

/// Trägt `sent` und `server_ref` in den gespeicherten Bericht ein.
fn mark_sent(path: &std::path::Path, mut report: Report, server_ref: &str) -> std::io::Result<()> {
    report.sent = Some(shared::now_iso());
    report.server_ref = Some(server_ref.to_string());
    shared::write_atomic(path, report.to_text().as_bytes())
}

/// `crash.send`: sendet die gewählten Berichte nacheinander (bei mehreren die neuesten 5). Eine Richtlinie
/// `DisableCrashReports` verhindert jede Anfrage. Ein Fehler beendet die Reihe; der Bericht bleibt ungesendet.
pub async fn send(app: &App, args: CrashSendArgs) -> Result<CrashSendResult, CmdError> {
    validate(&args)?;
    if service::disabled_by_policy(app) {
        return Ok(CrashSendResult {
            results: args
                .ids
                .iter()
                .map(|id| CrashSendOutcome {
                    id: id.clone(),
                    server_ref: None,
                    failure: Some(CrashSendFailure::Disabled),
                })
                .collect(),
        });
    }
    let service = app.ext::<CrashService>();
    let _gate = service.send_gate.lock().await;
    let stored = shared::list(&service::dir(app));
    let mut chosen: Vec<&shared::Stored> = stored
        .iter()
        .filter(|s| args.ids.contains(&s.report.id))
        .collect();
    if args.ids.len() > 1 {
        chosen.truncate(MAX_BATCH);
    }
    let mut results = Vec::new();
    for id in args
        .ids
        .iter()
        .filter(|id| !stored.iter().any(|s| &s.report.id == *id))
    {
        results.push(CrashSendOutcome {
            id: id.clone(),
            server_ref: None,
            failure: Some(CrashSendFailure::NotFound),
        });
    }
    let (url, timeout) = endpoint(app);
    let note = args.note.as_str();
    let email = args.email.trim();
    for stored in chosen {
        let text = &stored.text;
        let outcome = match post(app, &url, timeout, body(text, note, email)).await {
            Ok(server_ref) => {
                if let Err(e) = mark_sent(&stored.path, stored.report.clone(), &server_ref) {
                    tracing::warn!(error = %e.kind(), "Absturzbericht gesendet, Vermerk nicht gespeichert");
                }
                tracing::info!(server_ref = %server_ref, "Absturzbericht gesendet");
                CrashSendOutcome {
                    id: stored.report.id.clone(),
                    server_ref: Some(server_ref),
                    failure: None,
                }
            }
            Err(failure) => {
                tracing::info!(?failure, "Absturzbericht nicht gesendet");
                results.push(CrashSendOutcome {
                    id: stored.report.id.clone(),
                    server_ref: None,
                    failure: Some(failure),
                });
                break;
            }
        };
        results.push(outcome);
    }
    Ok(CrashSendResult { results })
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-247
    #[test]
    fn body_embeds_the_report_text_byte_for_byte() {
        let text = "{\n  \"format\": 1,\n  \"message\": \"ä \\\"x\\\"\"\n}";
        let b = body(text, "zuletzt \"Export\"\n", "a@b.example");
        assert!(b.starts_with(&format!("{{\"report\":{text},")));
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["note"], "zuletzt \"Export\"\n");
        assert_eq!(v["email"], "a@b.example");
        assert_eq!(v["report"]["format"], 1);
    }

    // spec: OUT-247
    #[test]
    fn note_and_email_limits() {
        let ok = CrashSendArgs {
            ids: vec![],
            note: "ä".repeat(2000),
            email: String::new(),
        };
        assert!(validate(&ok).is_ok());
        let long = CrashSendArgs {
            note: "ä".repeat(2001),
            ..ok.clone()
        };
        assert_eq!(validate(&long).unwrap_err().field.as_deref(), Some("note"));
        for bad in [
            "kein-at",
            "a@b",
            "a b@c.de",
            "a@b.de\r\nBcc: x@y.de",
            &format!("{}@x.de", "a".repeat(250)),
        ] {
            let args = CrashSendArgs {
                email: bad.into(),
                ..ok.clone()
            };
            assert_eq!(
                validate(&args).unwrap_err().field.as_deref(),
                Some("email"),
                "{bad}"
            );
        }
        let fine = CrashSendArgs {
            email: "ops@example.com".into(),
            ..ok
        };
        assert!(validate(&fine).is_ok());
    }
}
