//! `os-parity coverage`: Status je Spec-ID aus Tests mit `// spec: …`-Tag und Testergebnissen (Abschnitt 5.4).
//!
//! 1. **Tag-Scan** (`scan`): `crates/**/*.rs` und `tests/**/*.rs` nach `// spec: ID[, ID] [M2]` über
//!    Testfunktionen und nach `#[ignore = "…"]`.
//! 2. **Testergebnisse** (`results`): Ausgabe von `cargo test --workspace --exclude openspider-desktop
//!    --no-fail-fast` (`--test-output`) und/oder JUnit von `cargo nextest` (`--junit`). Die Zuordnung
//!    Testname → Quelle läuft über das Test-Binär und die Moduldeklarationen (`targets`).
//! 3. **Status** je ID aus `spec_ids` der Einträge mit `kind = "package"` in `M1-packages.json`:
//!    - `implemented`: mindestens ein getaggter Test mit `ok` und keiner mit `FAILED`;
//!    - `deferred`: alle getaggten Tests `ignored` und jeder Grund beginnt mit `M2:`, `M3:` oder
//!      `Klärung K-xx`;
//!    - sonst `open` (kein Test, kein Ergebnis, `FAILED`, Platzhalter-Ignores wie `wartet auf M1-Pxx`).
//!    - `verified` bleibt erhalten, wenn es schon in der Statusdatei steht und der Test weiter `ok` ist
//!      (P29 setzt es aus dem Paritätsbericht).

pub mod results;
pub mod scan;
pub mod targets;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{json, Value};

use crate::error::ParityError;
use results::{Outcome, TestResult};
use scan::{FileScan, TestFn};
use targets::Resolved;

/// Argumente von `os-parity coverage`.
#[derive(Debug, Clone, clap::Args)]
pub struct CoverageArgs {
    /// Paketliste (`docs/plan/M1-packages.json`).
    #[arg(long)]
    pub packages: PathBuf,
    /// Ausgabe von `cargo test --workspace --exclude openspider-desktop --no-fail-fast`.
    #[arg(long)]
    pub test_output: Option<PathBuf>,
    /// JUnit-XML aus `cargo nextest` (Alternative oder Ergänzung zu `--test-output`).
    #[arg(long)]
    pub junit: Option<PathBuf>,
    /// Statusdatei (`docs/plan/M1-status.json`).
    #[arg(long)]
    pub write: Option<PathBuf>,
    /// Wurzel des Repositories (enthält `crates/` und `tests/`).
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    /// Auch bei ignorierten Tests ohne gültigen Grund mit Exit-Code 1 enden.
    #[arg(long)]
    pub strict: bool,
}

/// Status einer Spec-ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Mindestens ein getaggter Test ist grün.
    Implemented,
    /// Durch Paritätsbericht belegt (von P29 gesetzt).
    Verified,
    /// Zurückgestellt mit gültigem Grund.
    Deferred,
    /// Offen.
    Open,
}

/// Ergebnis eines getaggten Tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestState {
    /// `ok`.
    Ok,
    /// `FAILED`.
    Failed,
    /// `ignored`.
    Ignored,
    /// Kein Ergebnis in der Ausgabe.
    NoResult,
}

/// Ein getaggter Test im Bericht.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestEntry {
    /// Quelldatei.
    pub file: String,
    /// Vollständiger Name innerhalb der Datei (Inline-Module + Funktion).
    pub test: String,
    /// Ergebnis.
    pub result: TestState,
    /// Ignore-Grund (Quelle bzw. Ausgabe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_reason: Option<String>,
    /// `[M2]`/`[M3]` im Tag.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deferred_hint: bool,
}

/// Status und Belege einer ID.
#[derive(Debug, Clone, Serialize)]
pub struct SpecEntry {
    /// Status.
    pub status: Status,
    /// Pakete, die die ID umsetzen.
    pub packages: Vec<String>,
    /// Begründung, wenn nicht `implemented`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Getaggte Tests.
    pub tests: Vec<TestEntry>,
    /// Beleg aus dem Paritätsbericht (aus der bestehenden Statusdatei übernommen).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parity: Option<Value>,
}

/// Ignorierter Test ohne gültigen Grund.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BadIgnore {
    /// Quelldatei.
    pub file: String,
    /// Testname.
    pub test: String,
    /// Zeile.
    pub line: u32,
    /// Angegebener Grund (leer = keiner).
    pub reason: String,
    /// IDs aus dem Tag.
    pub spec_ids: Vec<String>,
    /// Befund.
    pub problem: String,
}

/// Gesamtergebnis.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    /// Zusammenfassung.
    pub summary: BTreeMap<String, usize>,
    /// Status je ID.
    pub specs: BTreeMap<String, SpecEntry>,
    /// Ignorierte Tests ohne gültigen Grund.
    pub ignored_without_valid_reason: Vec<BadIgnore>,
    /// Hinweise.
    pub warnings: Vec<String>,
}

/// Ist der Ignore-Grund ein gültiger Zurückstellungsgrund?
pub fn valid_defer_reason(reason: &str) -> bool {
    let r = reason.trim();
    if r.starts_with("M2:") || r.starts_with("M3:") {
        return true;
    }
    for prefix in ["Klärung K-", "Klaerung K-"] {
        if let Some(rest) = r.strip_prefix(prefix) {
            return rest.chars().next().is_some_and(|c| c.is_ascii_digit());
        }
    }
    false
}

/// Spec-IDs je Paket aus `M1-packages.json` (nur Einträge mit `kind = "package"`).
pub fn load_package_specs(
    path: &std::path::Path,
) -> Result<BTreeMap<String, Vec<String>>, ParityError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ParityError::Invalid(format!("{} nicht lesbar: {e}", path.display())))?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| ParityError::Invalid(format!("{}: {e}", path.display())))?;
    let list = doc
        .as_array()
        .or_else(|| doc.get("packages").and_then(|p| p.as_array()))
        .ok_or_else(|| {
            ParityError::Invalid(format!("{}: Liste von Paketen erwartet", path.display()))
        })?;
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in list {
        if p.get("kind").and_then(|k| k.as_str()) != Some("package") {
            continue;
        }
        let pkg = p.get("id").and_then(|i| i.as_str()).unwrap_or("?");
        for id in p
            .get("spec_ids")
            .and_then(|s| s.as_array())
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str())
        {
            let owners = ids.entry(id.to_string()).or_default();
            if !owners.iter().any(|o| o == pkg) {
                owners.push(pkg.to_string());
            }
        }
    }
    Ok(ids)
}

#[derive(Default, Clone)]
struct Agg {
    ok: bool,
    failed: bool,
    ignored: bool,
    reason: Option<String>,
}

/// Rechnet den Status aus Quellen und Ergebnissen.
pub fn compute(
    package_specs: &BTreeMap<String, Vec<String>>,
    scans: &BTreeMap<String, FileScan>,
    targets_found: Vec<targets::Target>,
    results: &[TestResult],
    previous: Option<&Value>,
) -> CoverageReport {
    let tests: Vec<TestFn> = scans.values().flat_map(|s| s.tests.clone()).collect();
    let index = targets::build_index(targets_found, scans, &tests);
    let mut warnings: Vec<String> = Vec::new();
    let mut aggs: HashMap<usize, Agg> = HashMap::new();
    let mut unmapped = 0usize;
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    // Pro Ausgabeblock die Testpfade sammeln, um gleichnamige Ziele verschiedener Pakete zu trennen.
    let mut block_paths: HashMap<usize, Vec<&str>> = HashMap::new();
    for r in results {
        block_paths
            .entry(r.block)
            .or_default()
            .push(r.path.as_str());
    }
    for r in results {
        let cands = index.narrow(index.candidates(&r.binary), &block_paths[&r.block]);
        if cands.is_empty() {
            unmapped += 1;
            continue;
        }
        match index.resolve(&cands, &r.path) {
            Resolved::None => unmapped += 1,
            Resolved::Ambiguous(ts) => {
                let names: Vec<String> = ts
                    .iter()
                    .map(|&t| format!("{}::{}", tests[t].file, tests[t].name))
                    .collect();
                ambiguous.insert(format!(
                    "Testname {:?} nicht eindeutig zuordenbar (Kandidaten: {}); keine Gutschrift",
                    r.path,
                    names.join(", ")
                ));
            }
            Resolved::One(t) => {
                let a = aggs.entry(t).or_default();
                match &r.outcome {
                    Outcome::Ok => a.ok = true,
                    Outcome::Failed => a.failed = true,
                    Outcome::Ignored(reason) => {
                        a.ignored = true;
                        if a.reason.is_none() {
                            a.reason = reason.clone();
                        }
                    }
                }
            }
        }
    }
    warnings.extend(ambiguous);

    // Tests je ID.
    let mut per_id: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, t) in tests.iter().enumerate() {
        for id in &t.ids {
            per_id.entry(id.as_str()).or_default().push(i);
        }
    }
    let state_of = |i: usize| -> TestState {
        match aggs.get(&i) {
            Some(a) if a.failed => TestState::Failed,
            Some(a) if a.ok => TestState::Ok,
            Some(a) if a.ignored => TestState::Ignored,
            _ => TestState::NoResult,
        }
    };
    let reason_of = |i: usize| -> Option<String> {
        let src = tests[i].ignore.clone().filter(|r| !r.is_empty());
        src.or_else(|| aggs.get(&i).and_then(|a| a.reason.clone()))
    };

    let mut specs: BTreeMap<String, SpecEntry> = BTreeMap::new();
    for (id, packages) in package_specs {
        let idxs = per_id.get(id.as_str()).cloned().unwrap_or_default();
        let entries: Vec<TestEntry> = idxs
            .iter()
            .map(|&i| TestEntry {
                file: tests[i].file.clone(),
                test: tests[i]
                    .module
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(tests[i].name.as_str()))
                    .collect::<Vec<_>>()
                    .join("::"),
                result: state_of(i),
                ignore_reason: reason_of(i),
                deferred_hint: tests[i].deferred_hint,
            })
            .collect();
        let states: Vec<TestState> = idxs.iter().map(|&i| state_of(i)).collect();
        let (mut status, note) = if idxs.is_empty() {
            (
                Status::Open,
                Some("kein Test mit `// spec:`-Tag gefunden".to_string()),
            )
        } else if states.contains(&TestState::Failed) {
            (
                Status::Open,
                Some("getaggter Test schlägt fehl (FAILED)".to_string()),
            )
        } else if states.contains(&TestState::Ok) {
            (Status::Implemented, None)
        } else if states.iter().all(|s| *s == TestState::Ignored) {
            let all_valid = idxs
                .iter()
                .all(|&i| reason_of(i).is_some_and(|r| valid_defer_reason(&r)));
            if all_valid {
                (Status::Deferred, None)
            } else {
                (
                    Status::Open,
                    Some(
                        "alle getaggten Tests ignoriert, Grund fehlt oder ist kein M2:/M3:/Klärung K-xx"
                            .to_string(),
                    ),
                )
            }
        } else {
            (
                Status::Open,
                Some("getaggte Tests ohne Ergebnis in der Testausgabe".to_string()),
            )
        };
        let mut parity = None;
        if status == Status::Implemented {
            let prev = previous
                .and_then(|p| p.get("specs"))
                .and_then(|s| s.get(id.as_str()));
            if prev.and_then(|p| p.get("status")).and_then(|s| s.as_str()) == Some("verified") {
                status = Status::Verified;
                parity = prev.and_then(|p| p.get("parity")).cloned();
            }
        }
        specs.insert(
            id.clone(),
            SpecEntry {
                status,
                packages: packages.clone(),
                note,
                tests: entries,
                parity,
            },
        );
    }

    // Unbekannte IDs in Tags.
    let unknown: BTreeSet<&str> = per_id
        .keys()
        .copied()
        .filter(|id| !package_specs.contains_key(*id))
        .collect();
    if !unknown.is_empty() {
        warnings.push(format!(
            "Tags mit IDs, die in keinem Paket stehen: {}",
            unknown.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if results.is_empty() {
        warnings.push(
            "keine Testergebnisse eingelesen: alle getaggten Tests gelten als ohne Ergebnis".into(),
        );
    }
    if unmapped > 0 {
        warnings.push(format!(
            "{unmapped} Testergebnisse ohne getaggte Quelle bzw. unbekanntes Test-Binär (ohne Wirkung)"
        ));
    }

    // Ignorierte Tests ohne gültigen Grund (alle Tests, nicht nur getaggte).
    let mut bad: Vec<BadIgnore> = Vec::new();
    for t in &tests {
        let Some(reason) = &t.ignore else { continue };
        let problem = if reason.trim().is_empty() {
            Some("#[ignore] ohne Grund")
        } else if !t.ids.is_empty() && !valid_defer_reason(reason) {
            Some("Grund beginnt nicht mit M2:/M3:/Klärung K-xx (Platzhalter zählen nicht als deferred)")
        } else {
            None
        };
        if let Some(p) = problem {
            bad.push(BadIgnore {
                file: t.file.clone(),
                test: t
                    .module
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(t.name.as_str()))
                    .collect::<Vec<_>>()
                    .join("::"),
                line: t.line,
                reason: reason.clone(),
                spec_ids: t.ids.clone(),
                problem: p.to_string(),
            });
        }
    }
    bad.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    let mut summary: BTreeMap<String, usize> = BTreeMap::new();
    summary.insert("total".into(), specs.len());
    for (name, st) in [
        ("implemented", Status::Implemented),
        ("verified", Status::Verified),
        ("deferred", Status::Deferred),
        ("open", Status::Open),
    ] {
        summary.insert(
            name.into(),
            specs.values().filter(|s| s.status == st).count(),
        );
    }
    CoverageReport {
        summary,
        specs,
        ignored_without_valid_reason: bad,
        warnings,
    }
}

impl CoverageReport {
    /// JSON-Fassung (deterministisch, ohne Zeitstempel).
    pub fn to_json(&self) -> Value {
        json!({
            "generated_by": "os-parity coverage",
            "summary": self.summary,
            "specs": self.specs,
            "ignored_without_valid_reason": self.ignored_without_valid_reason,
            "warnings": self.warnings,
        })
    }

    /// Anzahl offener IDs.
    pub fn open_count(&self) -> usize {
        self.summary.get("open").copied().unwrap_or(0)
    }

    /// Kurzfassung für die Konsole.
    pub fn summary_text(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let g = |k: &str| self.summary.get(k).copied().unwrap_or(0);
        let _ = writeln!(
            s,
            "Spec-Abdeckung: {} IDs – implemented {}, verified {}, deferred {}, open {}",
            g("total"),
            g("implemented"),
            g("verified"),
            g("deferred"),
            g("open")
        );
        let open: Vec<_> = self
            .specs
            .iter()
            .filter(|(_, e)| e.status == Status::Open)
            .collect();
        for (id, e) in open.iter().take(60) {
            let _ = writeln!(s, "  open {id}: {}", e.note.as_deref().unwrap_or(""));
        }
        if open.len() > 60 {
            let _ = writeln!(s, "  ... und {} weitere offene IDs", open.len() - 60);
        }
        if !self.ignored_without_valid_reason.is_empty() {
            let _ = writeln!(
                s,
                "Ignorierte Tests ohne gültigen Grund: {}",
                self.ignored_without_valid_reason.len()
            );
            for b in self.ignored_without_valid_reason.iter().take(60) {
                let _ = writeln!(s, "  {}:{} {} – {}", b.file, b.line, b.test, b.problem);
            }
        }
        for w in &self.warnings {
            let _ = writeln!(s, "Warnung: {w}");
        }
        s
    }
}

/// Ermittelt den Status; `Ok(true)` = keine ID mit Status `open`.
pub fn run(args: &CoverageArgs) -> Result<bool, ParityError> {
    let package_specs = load_package_specs(&args.packages)?;
    let root = &args.root;

    let mut scans: BTreeMap<String, FileScan> = BTreeMap::new();
    for dir in ["crates", "tests"] {
        for rel in scan::rust_files(root, dir) {
            let fs = scan::scan_file(root, &rel)?;
            scans.insert(rel, fs);
        }
    }
    let targets_found = targets::discover_targets(root)?;

    let mut results: Vec<TestResult> = Vec::new();
    if let Some(p) = &args.test_output {
        let bytes = std::fs::read(p)
            .map_err(|e| ParityError::Invalid(format!("{} nicht lesbar: {e}", p.display())))?;
        results.extend(results::parse_cargo_output(&String::from_utf8_lossy(
            &bytes,
        )));
    }
    if let Some(p) = &args.junit {
        let xml = std::fs::read_to_string(p)
            .map_err(|e| ParityError::Invalid(format!("{} nicht lesbar: {e}", p.display())))?;
        results.extend(results::parse_junit(&xml).map_err(|e| match e {
            ParityError::Invalid(m) => ParityError::Invalid(format!("{}: {m}", p.display())),
            other => other,
        })?);
    }

    let previous: Option<Value> = args
        .write
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok());
    let report = compute(
        &package_specs,
        &scans,
        targets_found,
        &results,
        previous.as_ref(),
    );

    if let Some(out) = &args.write {
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&report.to_json())
            .map_err(|e| ParityError::Invalid(format!("Status nicht serialisierbar: {e}")))?;
        std::fs::write(out, text + "\n")?;
    }
    print!("{}", report.summary_text());
    let strict_ok = !args.strict || report.ignored_without_valid_reason.is_empty();
    Ok(report.open_count() == 0 && strict_ok)
}
