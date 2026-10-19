//! Kompatibilitätsprüfung von Metriken und Dimensionen (INTEG-101).
//!
//! Nach jeder Änderung der Auswahl sendet [`CompatibilityChecker`] höchstens 500 ms später genau eine Anfrage an
//! `checkCompatibility` mit allen gewählten Metriken, den Seitendimensionen und den Filterdimensionen. Das Ergebnis
//! ([`Compatibility`]) sagt dem Metrik-Baum, welche Einträge ausgegraut sind, markiert gewählte, inzwischen
//! inkompatible Metriken (ohne sie zu entfernen) und sperrt „OK“, bis der Konflikt gelöst ist. Ist der Endpunkt nicht
//! erreichbar, bleibt die Auswahl frei.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};

use super::settings::Ga4Settings;

/// Entprellzeit nach einer Änderung der Auswahl.
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// Anfrage-Body von `checkCompatibility` für eine Auswahl.
pub fn request_body(settings: &Ga4Settings) -> Value {
    let mut metrics: Vec<&str> = Vec::new();
    for m in &settings.metrics {
        if !metrics.contains(&m.api.as_str()) {
            metrics.push(&m.api);
        }
    }
    let mut body = json!({
        "dimensions": settings.page_dimensions().iter().map(|d| json!({"name": d.api()})).collect::<Vec<_>>(),
        "metrics": metrics.iter().map(|m| json!({"name": m})).collect::<Vec<_>>(),
    });
    if let Some(filter) = settings.dimension_filter(None) {
        body["dimensionFilter"] = filter;
    }
    body
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compatibility {
    /// Fingerabdruck der geprüften Auswahl.
    pub fingerprint: String,
    /// Der Endpunkt hat geantwortet (sonst ist die Auswahl frei).
    pub reachable: bool,
    /// Inkompatible Metriken mit den Dimensionen der Auswahl, mit denen sie nicht kombinierbar sind.
    pub incompatible_metrics: BTreeMap<String, Vec<String>>,
    /// Inkompatible Dimensionen.
    pub incompatible_dimensions: BTreeSet<String>,
}

impl Compatibility {
    /// Ergebnis, wenn der Endpunkt nicht erreichbar war.
    pub fn unreachable(settings: &Ga4Settings) -> Compatibility {
        Compatibility {
            fingerprint: settings.fingerprint(),
            ..Compatibility::default()
        }
    }

    /// Wertet die Antwort aus.
    pub fn from_response(settings: &Ga4Settings, response: &Value) -> Compatibility {
        let mut dims: Vec<String> = settings
            .page_dimensions()
            .iter()
            .map(|d| d.api().to_string())
            .collect();
        for f in &settings.filters {
            if !dims.contains(&f.dimension) {
                dims.push(f.dimension.clone());
            }
        }
        let incompatible = |row: &Value| row["compatibility"].as_str() == Some("INCOMPATIBLE");
        Compatibility {
            fingerprint: settings.fingerprint(),
            reachable: true,
            incompatible_metrics: response["metricCompatibilities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|r| incompatible(r))
                .filter_map(|r| r.pointer("/metricMetadata/apiName").and_then(Value::as_str))
                .map(|m| (m.to_string(), dims.clone()))
                .collect(),
            incompatible_dimensions: response["dimensionCompatibilities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|r| incompatible(r))
                .filter_map(|r| {
                    r.pointer("/dimensionMetadata/apiName")
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
                .collect(),
        }
    }

    /// Ist die Metrik im Baum wählbar?
    pub fn is_selectable(&self, metric: &str) -> bool {
        !self.incompatible_metrics.contains_key(metric)
    }

    /// Tooltip eines ausgegrauten Eintrags: `Nicht kombinierbar mit: <Dimensionen>`.
    pub fn tooltip(&self, metric: &str) -> Option<String> {
        self.incompatible_metrics
            .get(metric)
            .map(|dims| format!("Nicht kombinierbar mit: {}", dims.join(", ")))
    }

    /// Gewählte Metriken, die inkompatibel sind (Warnsymbol, nicht stillschweigend entfernt).
    pub fn conflicts(&self, settings: &Ga4Settings) -> Vec<String> {
        let mut out: Vec<String> = settings
            .metrics
            .iter()
            .filter(|m| !self.is_selectable(&m.api))
            .map(|m| m.api.clone())
            .collect();
        out.dedup();
        out
    }

    /// „OK“ ist aktiv, wenn keine gewählte Metrik inkompatibel ist.
    pub fn ok_enabled(&self, settings: &Ga4Settings) -> bool {
        self.conflicts(settings).is_empty()
    }
}

/// Prüffunktion: führt genau eine Anfrage für eine Auswahl aus.
pub type CheckFn = Arc<
    dyn Fn(Ga4Settings) -> futures_util::future::BoxFuture<'static, Compatibility> + Send + Sync,
>;

/// Entprellte Prüfung der Auswahl im Dialog.
pub struct CompatibilityChecker {
    tx: mpsc::UnboundedSender<Ga4Settings>,
    rx: watch::Receiver<Option<Compatibility>>,
}

impl CompatibilityChecker {
    /// Startet die Prüfschleife; `check` wird je Ruhephase von [`DEBOUNCE`] genau einmal mit der letzten Auswahl
    /// aufgerufen.
    pub fn spawn(check: CheckFn) -> CompatibilityChecker {
        let (tx, mut changes) = mpsc::unbounded_channel::<Ga4Settings>();
        let (out, rx) = watch::channel(None);
        tokio::spawn(async move {
            while let Some(mut latest) = changes.recv().await {
                loop {
                    match tokio::time::timeout(DEBOUNCE, changes.recv()).await {
                        Ok(Some(newer)) => latest = newer,
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                let result = check(latest).await;
                if out.send(Some(result)).is_err() {
                    return;
                }
            }
        });
        CompatibilityChecker { tx, rx }
    }

    /// Meldet eine geänderte Auswahl.
    pub fn update(&self, settings: Ga4Settings) {
        let _ = self.tx.send(settings);
    }

    /// Letztes Ergebnis.
    pub fn latest(&self) -> Option<Compatibility> {
        self.rx.borrow().clone()
    }

    /// Wartet auf das nächste Ergebnis.
    pub async fn changed(&mut self) -> Option<Compatibility> {
        self.rx.changed().await.ok()?;
        self.rx.borrow().clone()
    }
}
