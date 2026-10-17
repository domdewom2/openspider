//! Berichtstypen und Ausgabe (`report.json` + `report.md`, Abschnitt 5.4).

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::deviations::Scope;
use crate::error::ParityError;

/// Höchstzahl Beispiele je Spalte und je Abweichung.
pub const MAX_EXAMPLES: usize = 10;

/// Umgebungsvariable, die die Zahl der Beispiele je Spalte für die Fehlersuche erhöht (Standard: [`MAX_EXAMPLES`]).
pub const EXAMPLES_ENV: &str = "OS_PARITY_MAX_EXAMPLES";

/// Zahl der Beispiele je Spalte und je Abweichung: [`MAX_EXAMPLES`] oder der Wert von [`EXAMPLES_ENV`].
pub fn max_examples() -> usize {
    std::env::var(EXAMPLES_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(MAX_EXAMPLES)
}

/// Ein abweichender Wert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    /// Zeilenschlüssel (normalisiert).
    pub key: String,
    /// Wert der Referenz (aufbereitet).
    #[serde(rename = "ref")]
    pub reference: String,
    /// Wert von OpenSpider (aufbereitet).
    pub os: String,
}

/// Ergebnis einer verglichenen Spalte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnReport {
    /// Spalte der Referenz.
    pub ref_column: String,
    /// Spalte von OpenSpider.
    pub os_column: String,
    /// Verglichene Zellen (ohne entschuldigte).
    pub compared: u64,
    /// Abweichende Zellen.
    pub errors: u64,
    /// Bis zu zehn Beispiele.
    pub examples: Vec<Example>,
}

/// Ignorierte Spalte mit Begründung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IgnoredColumn {
    /// Spalte der Referenz.
    pub column: String,
    /// Begründung aus dem Mapping.
    pub reason: Option<String>,
}

/// Angewandte Abweichung aus `deviations.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedDeviation {
    /// Kennung.
    pub id: String,
    /// Spec-ID.
    pub spec: String,
    /// Kennzeichen.
    pub kind: String,
    /// Begründung.
    pub note: String,
    /// Geltungsbereich.
    pub scope: Scope,
    /// Anzahl entschuldigter Zellen bzw. Zeilen.
    pub count: u64,
    /// Bis zu zehn betroffene Schlüssel.
    pub keys: Vec<String>,
}

/// Bericht eines Export-Vergleichs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Name des Exports (Mapping-Name).
    pub export: String,
    /// Site, falls bekannt.
    pub site: Option<String>,
    /// Zeilen der Referenz.
    pub ref_rows: u64,
    /// Zeilen von OpenSpider.
    pub os_rows: u64,
    /// Über den Schlüssel zugeordnete Zeilen.
    pub matched: u64,
    /// Schlüssel, die in der Referenz stehen, aber in OpenSpider fehlen.
    pub missing_in_os: Vec<String>,
    /// Schlüssel, die nur OpenSpider hat.
    pub extra_in_os: Vec<String>,
    /// Verglichene Zellen.
    pub cells_compared: u64,
    /// Gleiche Zellen.
    pub cells_equal: u64,
    /// `cells_equal / cells_compared` in Prozent (3 Nachkommastellen).
    pub agreement_pct: f64,
    /// Ergebnis je verglichener Spalte.
    pub columns: Vec<ColumnReport>,
    /// Angewandte Abweichungen.
    pub applied_deviations: Vec<AppliedDeviation>,
    /// Beim Einlesen verworfene Referenzspalten (nur Namen).
    pub dropped_ref_columns: Vec<String>,
    /// Laut Mapping ignorierte Spalten.
    pub ignored_columns: Vec<IgnoredColumn>,
    /// Referenzspalten ohne Mapping (nicht verglichen).
    pub unmapped_ref_columns: Vec<String>,
    /// OpenSpider-Spalten ohne Mapping, u. a. Erweiterungsspalten (nur aufgelistet, nicht verglichen).
    pub unmapped_os_columns: Vec<String>,
    /// Gemappte Spalten, die in der Referenzdatei fehlen (nicht verglichen).
    pub missing_ref_columns: Vec<String>,
    /// Gemappte Spalten, die in der OpenSpider-Datei fehlen (Werte gelten als leer).
    pub missing_os_columns: Vec<String>,
    /// Zeilen der Referenz außerhalb von M1 (Crawl-Overview: Filter, die M1 nicht umsetzt); nur gezählt.
    #[serde(default)]
    pub ignored_ref_rows: Vec<String>,
    /// Zeilen, die nur OpenSpider hat und die laut Mapping kein Fehler sind (Zusatzabschnitte).
    #[serde(default)]
    pub os_only_rows: Vec<String>,
    /// Geforderte Mindest-Übereinstimmung in Prozent.
    pub threshold_pct: f64,
    /// URL-Menge identisch (nichts fehlt, nichts zusätzlich, nach Abweichungen).
    pub url_set_identical: bool,
    /// Schwellen erfüllt.
    pub passed: bool,
    /// Gründe, warum die Schwellen verfehlt wurden.
    pub failures: Vec<String>,
}

/// Schreibt `report.json` und `report.md` in `dir` (wird angelegt).
pub fn write_report(dir: &Path, report: &Report) -> Result<(), ParityError> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| ParityError::Invalid(format!("Bericht nicht serialisierbar: {e}")))?;
    std::fs::write(dir.join("report.json"), json + "\n")?;
    std::fs::write(dir.join("report.md"), render_markdown(report))?;
    Ok(())
}

/// Kennungen `REF-FREE-nn` bezeichnen Referenzeinschränkungen der Free-Version (K-53).
pub fn is_ref_free(id: &str) -> bool {
    id.starts_with("REF-FREE-")
}

fn cell(s: &str) -> String {
    let one_line: String = s.replace(['\n', '\r'], " ").replace('|', "\\|");
    if one_line.chars().count() > 120 {
        let cut: String = one_line.chars().take(117).collect();
        format!("{cut}...")
    } else {
        one_line
    }
}

/// Markdown-Fassung des Berichts.
pub fn render_markdown(r: &Report) -> String {
    let mut s = String::new();
    let site = r.site.as_deref().unwrap_or("-");
    let _ = writeln!(s, "# Paritätsbericht `{}` (Site `{site}`)\n", r.export);
    let _ = writeln!(
        s,
        "Ergebnis: **{}**{}\n",
        if r.passed {
            "bestanden"
        } else {
            "nicht bestanden"
        },
        if r.failures.is_empty() {
            String::new()
        } else {
            format!(" – {}", r.failures.join("; "))
        }
    );
    let _ = writeln!(s, "| Kennzahl | Wert |\n|---|---:|");
    let _ = writeln!(s, "| Zeilen Referenz | {} |", r.ref_rows);
    let _ = writeln!(s, "| Zeilen OpenSpider | {} |", r.os_rows);
    let _ = writeln!(s, "| zugeordnet | {} |", r.matched);
    let _ = writeln!(s, "| fehlen in OpenSpider | {} |", r.missing_in_os.len());
    let _ = writeln!(s, "| zusätzlich in OpenSpider | {} |", r.extra_in_os.len());
    let _ = writeln!(s, "| Zellen verglichen | {} |", r.cells_compared);
    let _ = writeln!(s, "| Zellen gleich | {} |", r.cells_equal);
    let _ = writeln!(s, "| Übereinstimmung | {:.3} % |", r.agreement_pct);
    let _ = writeln!(s, "| Schwelle | {:.3} % |\n", r.threshold_pct);

    if !r.columns.is_empty() {
        let _ = writeln!(s, "## Spalten\n");
        let _ = writeln!(
            s,
            "| Referenz | OpenSpider | verglichen | Fehler |\n|---|---|---:|---:|"
        );
        for c in &r.columns {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                cell(&c.ref_column),
                cell(&c.os_column),
                c.compared,
                c.errors
            );
        }
        s.push('\n');
        for c in r.columns.iter().filter(|c| !c.examples.is_empty()) {
            let _ = writeln!(s, "### Beispiele `{}`\n", c.ref_column);
            let _ = writeln!(s, "| Schlüssel | Referenz | OpenSpider |\n|---|---|---|");
            for e in &c.examples {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} |",
                    cell(&e.key),
                    cell(&e.reference),
                    cell(&e.os)
                );
            }
            s.push('\n');
        }
    }
    list_section(&mut s, "Fehlen in OpenSpider", &r.missing_in_os);
    list_section(&mut s, "Zusätzlich in OpenSpider", &r.extra_in_os);
    list_section(
        &mut s,
        "Zeilen der Referenz außerhalb von M1 (nicht verglichen)",
        &r.ignored_ref_rows,
    );
    list_section(
        &mut s,
        "Zeilen nur in OpenSpider (kein Fehler)",
        &r.os_only_rows,
    );
    // Referenzeinschränkungen der Free-Version (K-53) stehen getrennt von den begründeten Abweichungen.
    for (title, free) in [
        ("Angewandte Abweichungen", false),
        ("Referenzeinschränkungen (Free)", true),
    ] {
        let list: Vec<&AppliedDeviation> = r
            .applied_deviations
            .iter()
            .filter(|d| is_ref_free(&d.id) == free)
            .collect();
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(s, "## {title}\n");
        let _ = writeln!(
            s,
            "| Kennung | Spec | Kennzeichen | Anzahl | Begründung |\n|---|---|---|---:|---|"
        );
        for d in list {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} |",
                cell(&d.id),
                cell(&d.spec),
                cell(&d.kind),
                d.count,
                cell(&d.note)
            );
        }
        s.push('\n');
    }
    list_section(&mut s, "Verworfene Referenzspalten", &r.dropped_ref_columns);
    let ignored: Vec<String> = r
        .ignored_columns
        .iter()
        .map(|c| match &c.reason {
            Some(why) => format!("{} ({why})", c.column),
            None => c.column.clone(),
        })
        .collect();
    list_section(&mut s, "Ignorierte Spalten", &ignored);
    list_section(
        &mut s,
        "Referenzspalten ohne Mapping (nicht verglichen)",
        &r.unmapped_ref_columns,
    );
    list_section(
        &mut s,
        "OpenSpider-Spalten ohne Mapping (nicht verglichen)",
        &r.unmapped_os_columns,
    );
    list_section(
        &mut s,
        "Gemappte Spalten fehlen in der Referenz",
        &r.missing_ref_columns,
    );
    list_section(
        &mut s,
        "Gemappte Spalten fehlen in OpenSpider",
        &r.missing_os_columns,
    );
    s
}

fn list_section(s: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(s, "## {title} ({})\n", items.len());
    for i in items.iter().take(50) {
        let _ = writeln!(s, "- `{}`", i.replace('`', "'"));
    }
    if items.len() > 50 {
        let _ = writeln!(
            s,
            "- ... und {} weitere (siehe report.json)",
            items.len() - 50
        );
    }
    s.push('\n');
}
