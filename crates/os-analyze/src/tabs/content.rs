//! Tab „Content“ (ONPAGE-142, ONPAGE-143, ONPAGE-146; Filter-Kennungen aus `docs/plan/M1-filters.toml`).
//!
//! # Grundmenge (ONPAGE-009, Content-Tab)
//!
//! Interne HTML-Seiten mit Statuscode 2xx, die in den Tabellen stehen (`in_tables = 1`); zusätzlich interne
//! PDFs mit 2xx, solange `pdf.store` gilt (Parameter `:pdf_store`). Weiterleitungen, Fehlerseiten, blockierte
//! URLs, Ressourcen und externe Seiten gehören nicht dazu. Indexierbarkeit schränkt die Grundmenge nicht ein;
//! sie steht als Spalte da.
//!
//! # Spalten (ONPAGE-142)
//!
//! Alle Spalten der Spec in Spec-Reihenfolge. Die Spalten der P1-Funktionen (Satzanzahl und Lesbarkeit
//! ONPAGE-149, Near Duplicates ONPAGE-144, Embeddings INTEG-181/182, Rechtschreibung/Grammatik ONPAGE-153 ff.,
//! Sprache) sind vorhanden, aber leer (`NULL`) und standardmäßig ausgeblendet. `Hash` ist für PDFs leer
//! (Annahme A-53) und sonst `urls.hash_md5` wie im Internal-Tab.
//!
//! # Exact Duplicates (ONPAGE-143)
//!
//! Eine Seite steht im Filter, wenn ihr `Hash` nicht leer ist und mindestens eine *andere* Seite der
//! Vergleichsmenge denselben Hash hat. Die Vergleichsmenge ist die Grundmenge ohne PDFs, bei
//! `duplicates.only_indexable` (Standard) eingeschränkt auf indexierbare Seiten. Die Prüfung ist eine
//! **unkorrelierte Gruppierung** (K-83, M1-N27): `hash_md5 IN (SELECT hash_md5 FROM <Vergleichsmenge> GROUP BY
//! hash_md5 HAVING COUNT(*) > 1)`. SQLite wertet die Unterabfrage je Anweisung einmal aus (ein Lauf über `urls`
//! mit einem Sortierer, O(n log n)), das Prädikat braucht keinen Index über `hash_md5`. Der frühere Weg, ein
//! korreliertes `EXISTS` auf dem Index `urls_hash`, wurde ohne den Index quadratisch (der Planer legt für
//! korreliertes `EXISTS` keinen automatischen Index an) und hielt den Aufbau von `urls_hash` im Endbatch großer
//! Crawls am Leben. Die Treffermenge ist unverändert, auch für eine Seite, die selbst nicht zur Vergleichsmenge
//! gehört: Dann genügt ein Treffer in der Vergleichsmenge (`HAVING COUNT(*) > 0`, zweiter Zweig des Prädikats),
//! sonst braucht es zwei, weil die Seite sich selbst mitzählt. Das Prädikat bleibt **live**: Duplikate erscheinen
//! schon während des Crawls, und das Umschalten von `only_indexable` wirkt ohne neue Abrufe. Der Filter verlangt
//! `extraction.hash` (sonst steht kein Hash in der Datei) und `duplicates.exact_enabled`; fehlt eines von
//! beiden, meldet der Filterzustand `NeedsConfig` statt Treffern. `issues.ignore_non_indexable` wirkt hier
//! nicht: Die Indexierbarkeit regelt allein `duplicates.only_indexable`. Der Hash wird mit `=` (BINARY)
//! verglichen, NULL und leere Hashes zählen nie als Treffer.
//!
//! # Low Content Pages (ONPAGE-146)
//!
//! HTML-Seiten der Grundmenge mit `Word Count` < `content.low_word_count`. PDFs werden nie geprüft
//! (A-53). Der Filter trägt `ignore_non_indexable` (CRAWL-089): bei `issues.ignore_non_indexable` fallen
//! nicht indexierbare Seiten heraus (Annahme A-38).
//!
//! # Benannte Parameter
//!
//! `:pdf_store`, `:dup_only_indexable`, `:low_word_count`.

use os_model::Value;

use crate::defs::{
    ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, SortKey, TabDef,
};

/// Grundmenge der Abfrage über dem Alias `$a` (ONPAGE-009): interne 2xx-HTML-Seiten und, bei `pdf.store`,
/// interne 2xx-PDFs.
macro_rules! population {
    ($a:literal) => {
        concat!(
            $a,
            ".is_internal = 1 AND ",
            $a,
            ".in_tables = 1 AND ",
            $a,
            ".status_code BETWEEN 200 AND 299 AND (",
            $a,
            ".content_class = 'HTML' OR (",
            $a,
            ".content_class = 'PDF' AND :pdf_store = 1))"
        )
    };
}

/// Zugehörigkeit zur Vergleichsmenge (ONPAGE-143) über dem Alias `$a`.
macro_rules! comparison_member {
    ($a:literal) => {
        concat!(
            "(:dup_only_indexable = 0 OR COALESCE(",
            $a,
            ".indexability, '') <> 'Non-Indexable')"
        )
    };
}

/// Die Vergleichsmenge der Exact Duplicates über dem Alias `d`: interne 2xx-HTML-Seiten in den Tabellen mit
/// nicht leerem Hash, bei `duplicates.only_indexable` nur indexierbare. Das unäre Plus verbietet dem Planer die
/// Indizes auf Klasse, Status und Hash: Er liest `urls` einmal sequenziell und gruppiert mit einem Sortierer, statt
/// über einen Index zufällig auf die Zeilen zuzugreifen (K-83, Bericht `probe-n27`).
macro_rules! comparison_set {
    () => {
        concat!(
            "SELECT +d.hash_md5 FROM urls d WHERE +d.is_internal = 1 AND +d.in_tables = 1 AND ",
            "+d.status_code BETWEEN 200 AND 299 AND +d.content_class = 'HTML' AND ",
            "d.hash_md5 IS NOT NULL AND +d.hash_md5 <> '' AND ",
            comparison_member!("d")
        )
    };
}

/// Prädikat „Exact Duplicates“ (ONPAGE-143, K-83): unkorrelierte Gruppierung der Vergleichsmenge. Der erste Zweig
/// gilt für Seiten, die selbst zur Vergleichsmenge gehören (zwei Treffer, einer davon die Seite selbst); der zweite
/// für Seiten außerhalb (ein Treffer genügt). Die Zugehörigkeit von `u` prüft nur Spalten der eigenen Zeile.
const EXACT_DUPLICATES: &str = concat!(
    "(u.content_class = 'HTML' AND u.hash_md5 IS NOT NULL AND u.hash_md5 <> '' AND ",
    comparison_member!("u"),
    " AND (u.hash_md5 IN (",
    comparison_set!(),
    " GROUP BY +d.hash_md5 HAVING COUNT(*) > 1) OR (NOT (COALESCE(u.is_internal, 0) = 1 AND ",
    "COALESCE(u.in_tables, 0) = 1 AND COALESCE(u.status_code, 0) BETWEEN 200 AND 299) AND ",
    "u.hash_md5 IN (",
    comparison_set!(),
    "))))"
);

/// Prädikat „Low Content Pages“ (ONPAGE-146): nur HTML, `Word Count` < Schwelle.
const LOW_CONTENT: &str = "(u.content_class = 'HTML' AND u.word_count < :low_word_count)";

fn column(
    key: &'static str,
    sql: &'static str,
    kind: ColumnKind,
    default_visible: bool,
) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible,
        extension: false,
        ..ColumnDef::META
    }
}

/// Spalte einer P1-Funktion: leer und standardmäßig ausgeblendet (ONPAGE-142).
fn p1_column(key: &'static str, kind: ColumnKind) -> ColumnDef {
    column(key, "NULL", kind, false)
}

/// Definition des Tabs „Content“.
pub fn tab() -> TabDef {
    TabDef {
        id: "Content",
        population: population!("u"),
        columns: vec![
            column("Address", "u.address", ColumnKind::Url, true),
            column("Word Count", "u.word_count", ColumnKind::Int, true),
            p1_column("Sentence Count", ColumnKind::Int),
            p1_column(
                "Average Words Per Sentence",
                ColumnKind::Float { decimals: 1 },
            ),
            p1_column(
                "Flesch Reading Ease Score",
                ColumnKind::Float { decimals: 1 },
            ),
            p1_column("Readability", ColumnKind::Text),
            p1_column(
                "Closest Similarity Match",
                ColumnKind::Float { decimals: 1 },
            ),
            p1_column("No. Near Duplicates", ColumnKind::Int),
            p1_column("Closest Semantically Similar Address", ColumnKind::Url),
            p1_column(
                "Semantic Similarity Score",
                ColumnKind::Float { decimals: 3 },
            ),
            p1_column("No. Semantically Similar", ColumnKind::Int),
            p1_column(
                "Semantic Relevance Score",
                ColumnKind::Float { decimals: 3 },
            ),
            p1_column("Total Language Errors", ColumnKind::Int),
            p1_column("Spelling Errors", ColumnKind::Int),
            p1_column("Grammar Errors", ColumnKind::Int),
            p1_column("Language", ColumnKind::Text),
            // PDFs tragen keinen Hash (A-53).
            column(
                "Hash",
                "CASE WHEN u.content_class = 'PDF' THEN NULL ELSE u.hash_md5 END",
                ColumnKind::Hash,
                true,
            ),
            column("Indexability", "u.indexability", ColumnKind::Text, true),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
                true,
            ),
        ],
        filters: vec![
            FilterDef {
                id: "All",
                predicate: "1",
                kind: FilterKind::Live,
                requires: Vec::new(),
                ignore_non_indexable: false,
            },
            FilterDef {
                id: "Exact Duplicates",
                predicate: EXACT_DUPLICATES,
                kind: FilterKind::Live,
                requires: vec![
                    Requirement::Config("extraction.hash"),
                    Requirement::Config("duplicates.exact_enabled"),
                ],
                ignore_non_indexable: false,
            },
            FilterDef {
                id: "Low Content Pages",
                predicate: LOW_CONTENT,
                kind: FilterKind::Live,
                requires: Vec::new(),
                ignore_non_indexable: true,
            },
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: |cfg| {
            vec![
                ("pdf_store", Value::Int(i64::from(cfg.pdf.store))),
                (
                    "dup_only_indexable",
                    Value::Int(i64::from(cfg.duplicates.only_indexable)),
                ),
                (
                    "low_word_count",
                    Value::Int(i64::from(cfg.content.low_word_count)),
                ),
            ]
        },
    }
}
