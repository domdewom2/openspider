//! PERF-002: Durchsatz in zehn Abschnitten (K-72).
//!
//! Der Elternprozess stempelt jede `PROGRESS`-Zeile des Kindes beim Empfang ([`crate::proc::Proc`]). Aus diesen
//! Stützstellen `(Sekunden seit Crawl-Beginn, abgerufene URLs)` bestimmt [`segment_rates`] die Zeitpunkte, zu denen
//! 10 %, 20 % … 100 % der abgerufenen URLs erreicht waren (zwischen zwei Zeilen linear interpoliert), und daraus
//! den Durchsatz je Abschnitt. [`judge`] wendet das Kriterium aus `docs/spec/07-performance.md` an:
//! Mittelwert >= 500 URLs/s und jeder Abschnitt zwischen 70 % und 130 % des Mittelwerts. Reine Funktionen ohne
//! Prozesse und Uhren, damit sie ohne Crawl testbar sind.

/// Anzahl der Abschnitte (PERF-002: zehn zu je 10 % der abgerufenen URLs).
pub const SEGMENTS: usize = 10;
/// PERF-002: Mindestdurchsatz im Mittel (URLs/s).
pub const MIN_MEAN_URLS_PER_S: f64 = 500.0;
/// PERF-002: untere Grenze eines Abschnitts als Anteil des Mittelwerts.
pub const MIN_SEGMENT_SHARE: f64 = 0.7;
/// PERF-002: obere Grenze eines Abschnitts als Anteil des Mittelwerts.
pub const MAX_SEGMENT_SHARE: f64 = 1.3;
/// Das Urteil über Mittelwert und Abschnitte gilt erst ab dieser Seitenzahl (der 1-Mio.-Crawl von PERF-001); kleinere
/// Läufe berichten nur.
pub const JUDGED_FROM_PAGES: u64 = 1_000_000;

/// Eine Stützstelle: Sekunden seit Beginn des Crawls und Stand von `crawled` (nicht abnehmend).
pub type Sample = (f64, u64);

/// Zeitpunkt (Sekunden), zu dem `target` URLs erreicht waren; linear zwischen zwei Stützstellen interpoliert.
fn time_at(samples: &[Sample], target: f64) -> f64 {
    let mut prev = samples[0];
    for &(t, c) in samples {
        if c as f64 >= target {
            let (t0, c0) = prev;
            if c <= c0 {
                return t;
            }
            return t0 + (t - t0) * (target - c0 as f64) / (c - c0) as f64;
        }
        prev = (t, c);
    }
    prev.0
}

/// Durchsatz (URLs/s) der zehn Abschnitte zu je 10 % von `total` abgerufenen URLs.
///
/// `samples` beginnt mit dem Crawl-Beginn (`(0.0, 0)`) und endet mit dem Endstand. Ohne URLs oder ohne mindestens
/// zwei Stützstellen gibt es keine Werte (`None`). Ein Abschnitt, der in null Sekunden endet, liefert einen sehr
/// großen Wert statt unendlich (JSON kennt kein `inf`).
pub fn segment_rates(samples: &[Sample], total: u64) -> Option<Vec<f64>> {
    if total == 0 || samples.len() < 2 {
        return None;
    }
    let mut rates = Vec::with_capacity(SEGMENTS);
    let (mut t_prev, mut c_prev) = (samples[0].0, 0.0);
    for i in 1..=SEGMENTS {
        let target = total as f64 * i as f64 / SEGMENTS as f64;
        let t = time_at(samples, target);
        let dt = (t - t_prev).max(1e-9);
        rates.push((target - c_prev) / dt);
        t_prev = t;
        c_prev = target;
    }
    Some(rates)
}

/// Abweichung des kleinsten und des größten Abschnitts vom Mittelwert in Prozent (`(min, max)`, z. B. `(-16, 13)`).
pub fn deviation_pct(rates: &[f64], mean: f64) -> Option<(f64, f64)> {
    if rates.is_empty() || mean <= 0.0 {
        return None;
    }
    let min = rates.iter().copied().fold(f64::INFINITY, f64::min);
    let max = rates.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(((min / mean - 1.0) * 100.0, (max / mean - 1.0) * 100.0))
}

/// PERF-002/K-72: Verletzungen für den Lauf mit `pages` Seiten. Bei weniger als [`JUDGED_FROM_PAGES`] Seiten leer
/// (nur Bericht). `rates` leer (nicht messbar) ist bei einem beurteilten Lauf selbst eine Verletzung.
pub fn judge(pages: u64, mean: f64, rates: &[f64]) -> Vec<String> {
    let mut out = Vec::new();
    if pages < JUDGED_FROM_PAGES {
        return out;
    }
    if mean < MIN_MEAN_URLS_PER_S {
        out.push(format!(
            "PERF-002: Mittelwert {mean:.0} URLs/s < {MIN_MEAN_URLS_PER_S:.0} URLs/s bei {pages} Seiten"
        ));
    }
    if rates.is_empty() {
        out.push("PERF-002: Abschnittswerte nicht messbar".to_string());
        return out;
    }
    let (lo, hi) = (mean * MIN_SEGMENT_SHARE, mean * MAX_SEGMENT_SHARE);
    for (i, &r) in rates.iter().enumerate() {
        if r < lo || r > hi {
            out.push(format!(
                "PERF-002: Abschnitt {} ({} % bis {} %) mit {r:.0} URLs/s = {:.0} % des Mittelwerts {mean:.0} URLs/s, erlaubt 70 % bis 130 %",
                i + 1,
                i * 10,
                (i + 1) * 10,
                r / mean * 100.0
            ));
        }
    }
    out
}

fn round(v: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (v * f).round() / f
}

/// Berichtsfelder (ohne Präfix `crawler_`): `segment_urls_per_s` (Liste), `segment_dev_min_pct`,
/// `segment_dev_max_pct`. Nicht messbar: `null`-Werte, damit das Feld im Bericht erscheint.
pub fn report_fields(rates: Option<&[f64]>, mean: f64) -> Vec<(&'static str, serde_json::Value)> {
    use serde_json::json;
    match rates {
        Some(r) if !r.is_empty() => {
            let dev = deviation_pct(r, mean);
            vec![
                (
                    "segment_urls_per_s",
                    json!(r.iter().map(|v| round(*v, 1)).collect::<Vec<_>>()),
                ),
                ("segment_dev_min_pct", json!(dev.map(|d| round(d.0, 1)))),
                ("segment_dev_max_pct", json!(dev.map(|d| round(d.1, 1)))),
            ]
        }
        _ => vec![
            ("segment_urls_per_s", json!(null)),
            ("segment_dev_min_pct", json!(null)),
            ("segment_dev_max_pct", json!(null)),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gleichmäßiger Verlauf: `per_s` URLs/s über `total` URLs, Stützstellen alle `step` URLs.
    fn steady(total: u64, per_s: f64, step: u64) -> Vec<Sample> {
        let mut v = vec![(0.0, 0)];
        let mut c = 0;
        while c < total {
            c = (c + step).min(total);
            v.push((c as f64 / per_s, c));
        }
        v
    }

    // spec: PERF-002
    #[test]
    fn a_steady_crawl_has_ten_equal_segments_at_the_mean() {
        let total = 1_000_000;
        let samples = steady(total, 800.0, 3_000);
        let rates = segment_rates(&samples, total).unwrap();
        assert_eq!(rates.len(), SEGMENTS);
        for r in &rates {
            assert!((r - 800.0).abs() < 0.5, "{rates:?}");
        }
        let mean = total as f64 / samples.last().unwrap().0;
        let (lo, hi) = deviation_pct(&rates, mean).unwrap();
        assert!(lo.abs() < 0.1 && hi.abs() < 0.1, "{lo} {hi}");
        assert!(judge(total, mean, &rates).is_empty());
        assert_eq!(report_fields(Some(&rates), mean).len(), 3);
    }

    // spec: PERF-002
    #[test]
    fn a_collapse_in_the_last_segment_is_a_violation_naming_the_section() {
        // 90 % mit 1 000 URLs/s, das letzte Zehntel mit 100 URLs/s
        let total = 1_000_000u64;
        let mut samples = steady(900_000, 1_000.0, 2_000);
        let t0 = samples.last().unwrap().0;
        let mut c = 900_000;
        while c < total {
            c += 2_000;
            samples.push((t0 + (c - 900_000) as f64 / 100.0, c));
        }
        let rates = segment_rates(&samples, total).unwrap();
        assert!((rates[0] - 1_000.0).abs() < 1.0 && (rates[8] - 1_000.0).abs() < 1.0);
        assert!((rates[9] - 100.0).abs() < 1.0, "{rates:?}");
        let mean = total as f64 / samples.last().unwrap().0;
        assert!(mean > 500.0 && mean < 600.0, "{mean}");
        let v = judge(total, mean, &rates);
        assert!(!v.is_empty());
        assert!(v.iter().all(|m| m.contains("PERF-002")), "{v:?}");
        assert!(v.iter().any(|m| m.contains("Abschnitt 10")), "{v:?}");
        // die ersten neun liegen über 130 % des Mittelwerts? 1000 / 553 = 181 % -> ebenfalls verletzt
        assert!(v.iter().any(|m| m.contains("Abschnitt 1 ")), "{v:?}");
        let (lo, hi) = deviation_pct(&rates, mean).unwrap();
        assert!(lo < -70.0 && hi > 70.0, "{lo} {hi}");
    }

    // spec: PERF-002
    #[test]
    fn a_slow_mean_fails_even_when_the_sections_are_even() {
        let total = 1_000_000;
        let samples = steady(total, 300.0, 5_000);
        let rates = segment_rates(&samples, total).unwrap();
        let mean = total as f64 / samples.last().unwrap().0;
        let v = judge(total, mean, &rates);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("Mittelwert") && v[0].contains("PERF-002"));
    }

    // spec: PERF-002
    #[test]
    fn smaller_runs_are_reported_but_not_judged() {
        let samples = steady(10_000, 50.0, 100);
        let rates = segment_rates(&samples, 10_000).unwrap();
        assert!(judge(10_000, 50.0, &rates).is_empty());
        assert!(judge(999_999, 1.0, &[]).is_empty());
        assert!(!judge(1_000_000, 900.0, &[]).is_empty());
    }

    // spec: PERF-002
    #[test]
    fn boundaries_fall_between_two_progress_lines_by_linear_interpolation() {
        // Zwei Stützstellen: 0 URLs bei 0 s, 100 URLs bei 10 s, dann 100 -> 200 URLs in 1 s.
        let samples = vec![(0.0, 0), (10.0, 100), (11.0, 200)];
        // Zehntel = 20 URLs: Grenzen bei 2, 4 … 10 s (erste Hälfte), 10.2 … 11 s (zweite Hälfte)
        let rates = segment_rates(&samples, 200).unwrap();
        for r in &rates[..5] {
            assert!((r - 10.0).abs() < 1e-9, "{rates:?}");
        }
        for r in &rates[5..] {
            assert!((r - 100.0).abs() < 1e-6, "{rates:?}");
        }
        // Grenze genau auf einer stützstelle und eine Stützstelle mit gleichem Stand (DONE nach letztem PROGRESS)
        let flat = vec![(0.0, 0), (5.0, 50), (5.0, 50)];
        let r = segment_rates(&flat, 50).unwrap();
        assert!(r.iter().all(|v| (v - 10.0).abs() < 1e-9), "{r:?}");
    }

    // spec: PERF-002
    #[test]
    fn without_urls_or_samples_there_are_no_segments() {
        assert!(segment_rates(&[(0.0, 0)], 100).is_none());
        assert!(segment_rates(&[(0.0, 0), (1.0, 0)], 0).is_none());
        let f = report_fields(None, 0.0);
        assert!(f.iter().all(|(_, v)| v.is_null()));
    }
}
