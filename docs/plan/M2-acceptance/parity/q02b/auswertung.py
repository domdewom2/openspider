"""Q02b: Restaufträge/B3-Belege aus dem unveränderten os-parity-Journal.

Aufruf nach coverage: python3 <diese Datei> <report.json>
Keine eigene Paritäts- oder verified-Entscheidung; beide kommen aus os-parity.
Referenzdateien werden hier nicht eingelesen.
"""

import csv
import gzip
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]


def write(name, data):
    (HERE / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


# Zuordnung ist eine Triage, kein nachträgliches Grünrechnen des Journals.
# Jede offene Abweichung bleibt ein Auftrag; ein Auftrag ist kein DEV.
RULES = {
    "bulk_links_internal_nofollow_outlinks.csv": (
        "G02", ["OUT-031", "ONPAGE-012"],
        "crates/os-export/src/catalog/bulk_links.rs",
        "Größenwert des nicht gecrawlten nofollow-Ziels prüfen: Referenz 0, OS leer. Status Not Crawled ist separat DEV-051. Keine unbelegte Null-/Leer-Ausnahme ergänzen."),
    "serp_summary.csv": (
        "G19", ["OUT-098", "ONPAGE-009"],
        "crates/os-export/src/reports/serp_summary.rs",
        "real404.html wird als zusätzliche HTML-Fehlerseite exportiert. Referenzmenge gegen die 2xx-Grundmenge der Element-Tabs prüfen; falls konsistent, Spec-Präzisierung nach K2-96 und Filterfix. Pixelmetrik separat nach UI-038 gemessen."),
    "redirects_redirect_canonical_chains.csv": (
        "G23-R", ["OUT-083", "TECH-001"],
        "crates/os-export/src/reports/redirect_canonical_chains.rs",
        "Zusätzlicher HTTP-Selbst-Refresh refresh-self.html nach N09. Spec verlangt Redirected; Referenz lässt die Startzeile aus. Mit dem Selbst-Refresh der Alt-Site gegenprüfen, dann enges DEV oder Referenzartefakt entscheiden; keine ganze Reportmenge ausnehmen."),
    "canonicals_non_indexable_canonicals.csv": (
        "G17", ["OUT-087", "TECH-020"],
        "crates/os-export/src/reports/non_indexable_canonicals.rs",
        "canonical-unlinked.html → canonical-unlinked-target.html ist indexierbar, aber unlinked. OUT-087 verlangt ausdrücklich auch unlinked-Ziele; Referenzreport lässt die Beziehung aus. Reportabgrenzung zu Unlinked Canonicals nach Leitlinie/K2-96 entscheiden, danach Fix oder begründetes DEV."),
    "report_hreflang_non_200_hreflang_urls.csv": (
        "G17", ["OUT-091", "TECH-074"],
        "crates/os-export/src/reports/hreflang/mod.rs",
        "Die Referenz enthält / → pagination-404-target.html zusätzlich zum eigentlichen Hreflang-Ziel. Fixture-Annotation und Linktyp prüfen: möglicher Pagination-Beifang der Referenz. Nur mit diesem Nachweis REF-ART, sonst Reportfix; kein pauschaler Hreflang-DEV."),
    "report_hreflang_missing_return_links.csv": (
        "G02", ["OUT-093"],
        "tests/parity/mappings/hreflang_missing_returns.toml",
        "Referenzspalte hreflang ist in drei Beziehungen leer, OS liefert de/en. Prüfen, ob die Referenz den fehlenden Rücklinkcode statt des A→B-Codes meint. OUT-093 verlangt den Code; keine leer=beliebig-Regel. Mapping oder Spec/Report nach K2-96 präzisieren."),
    "report_hreflang_inconsistent_language_region_return_links.csv": (
        "G02", ["OUT-094", "TECH-077"],
        "tests/parity/mappings/hreflang_inconsistent_returns.toml",
        "URL Target ist in der Referenz die Selbstannotation; bisheriger Schlüssel setzt sie mit OS Hreflang URL gleich. Die bisher ignorierte Rücklinkquelle prüfen und die A/B-Beziehung vollständig einschließlich Expected/Actual Language zuordnen. Kein URL-Mengen-DEV für einen Richtungsfehler."),
    "report_hreflang_non_canonical_return_links.csv": (
        "G02", ["OUT-095", "TECH-078"],
        "tests/parity/mappings/hreflang_noncanonical_returns.toml",
        "Ein Quellen-/Zielpaar erscheint vertauscht. TECH-078 wurde in N09 auf Alternate umgestellt; Report und Mapping anhand OUT-095 gemeinsam prüfen. Schlüssel nicht durch Ignore/Deduplizieren entschärfen."),
}

OVERVIEW = {
    "page titles | below 200 pixels": (
        "G21", ["ONPAGE-096", "UI-038"],
        "crates/os-parse/src/pixel/calibrated.rs",
        "Exakt graph/10.html bis graph/99.html kreuzen die Grenze: Referenz 202 px, OS 199 px (1,485 %). Populationseigenschaften stimmen nativ exakt. Daher 90 Zusatztreffer und Overview 23/113. Metrik am Grenzwert kalibrieren, keine URL-Sonderregel und Schwelle 200 unverändert. Beleg pixel-boundary.json."),
    "validation | <head> not first in <html> element": (
        "G17", ["TECH-283", "TECH-284"],
        "crates/os-parse/src/validation.rs",
        "Referenz 0, OS 1 auf m2-security. N09/K2-93 verlangt Missing Head zusätzlich als Head Not First. Betroffene URL bestimmen und mit m2-technical/validation-head-missing vergleichen; bei inkonsistenter Referenz adressgenauer REF-ART, sonst Fix nach K2-96."),
    "images | missing size attributes": (
        "G09", ["ONPAGE-179"],
        "crates/os-analyze/src/tabs/images.rs",
        "Alt-Site parity-onpage: Referenz 9, OS 10. Zusätzliche Bildinstanz/URL bestimmen und Existenz von width/height nach ONPAGE-179 prüfen; neuer Fix-Auftrag, kein pauschaler Alt-Site-DEV."),
    "links | non-descriptive anchor text in internal outlinks": (
        "G19", ["TECH-136", "TECH-144"],
        "crates/os-analyze/src/tabs/links.rs",
        "Alt-Site parity-technical: Referenz 0, OS 1. Eigener Filterexport isoliert links/self-only.html mit Selbstlink-Anchor This page; exakter Eintrag der TECH-144-Defaultliste. Referenzauslassung kann am Selbstlink oder an abweichender Liste liegen. K-07-Geltung für TECH-136 prüfen, dann Spec-/Filterfix nach K2-96 oder eng begründeten Default-DEV entscheiden. Kein pauschaler Alt-Site-DEV; Beleg links-detail.json."),
    "links | follow & nofollow internal inlinks to page": (
        "G19", ["TECH-139"],
        "crates/os-analyze/src/tabs/links.rs",
        "Alt-Site parity-technical: Referenz 3, OS 6. Die drei Zusatztreffer mit Follow/nofollow-Instanzen belegen. N10 nennt alte Referenz, das allein begründet kein DEV; danach Fix oder Referenzartefakt entscheiden."),
}


def classify(file, column=None, example=None):
    if file == "crawl_overview.csv":
        return OVERVIEW[example["key"]]
    if file == "internal_all.csv":
        if column == "Canonical Link Element 1":
            return ("G15", ["ONPAGE-011", "TECH-022"],
                    "crates/os-analyze/src/tabs/internal.rs",
                    "Internal zeigt noch #fragment, obwohl der Canonicals-Tab nach K2-93 fragmentfrei anzeigt. Gemeinsame Anzeigeableitung prüfen und an die entschiedene Regel angleichen.")
        if column == "Text Ratio":
            return ("G06", ["ONPAGE-013"],
                    "crates/os-parse/src/text.rs",
                    "Zwei fehlerhaft strukturierte HTML-Fixtures: bei 73 bzw. 85 Quellzeichen implizieren die nativen Ratios Referenz-Numeratoren 20/26 gegen OS 19/25, also je ein fehlendes Trennzeichen. DOM-Reparatur und Abgrenzung des in den Body verschobenen title-Texts prüfen; drei Nachkommastellen beibehalten.")
    return RULES[file]


def main():
    source = pathlib.Path(sys.argv[1])
    run = json.loads(source.read_text())
    before_path = HERE / "before.json.gz"
    before = json.loads(gzip.decompress(before_path.read_bytes())) if before_path.exists() else json.loads((HERE / "before.json").read_text())
    records = []
    exports = {}
    for site in run["sites"]:
        for export in site["exports"]:
            exports[site["site"], export["ref_file"]] = export
            if export["outcome"] != "compared":
                continue
            report = export["report"]
            issues = []
            for column in report["columns"]:
                if column["errors"]:
                    for example in column["examples"]:
                        issues.append(("cell", column["os_column"], example))
            for kind in ["missing_in_os", "extra_in_os"]:
                issues.extend((kind, None, {"key": key}) for key in report[kind])
            if report.get("row_order_identical") is False:
                raise RuntimeError("Unklassifizierte Listenreihenfolge")
            for kind, column, example in issues:
                group, specs, file, action = classify(export["ref_file"], column, example)
                records.append(dict(site=site["site"], export=export["ref_file"],
                                    kind=kind, column=column, example=example,
                                    group=group, specs=specs, classification="FIX-Auftrag",
                                    file=file, action=action,
                                    export_passed=report["passed"]))
    write("restbefunde.json", records)
    write("original-23.json", [dict(site=site["site"], file=x["ref_file"],
          before=x["report"]["failures"],
          after=exports[site["site"], x["ref_file"]].get("report", {}).get("failures"),
          passed=exports[site["site"], x["ref_file"]].get("report", {}).get("passed"))
          for site in before["sites"] for x in site["exports"]
          if x.get("report") and not x["report"]["passed"]])
    coverage = json.loads((HERE / "coverage-after.json").read_text())
    tabs = {}
    for spec in coverage["verified_ids"]:
        for check in coverage["checks"][spec]:
            tabs.setdefault(check["tab"], set()).add(spec)
    write("verified-je-tab.json", {tab: sorted(ids) for tab, ids in sorted(tabs.items())})
    populations = {}
    for site in ["parity-m2-list", "parity-m2-technical", "parity-response", "parity-onpage", "parity-technical"]:
        path = source.parent / site / "os/internal_all.csv"
        rows = list(csv.DictReader(path.open(encoding="utf-8-sig")))
        populations[site] = [{key: r.get(key, "") for key in ["Address", "Original Url", "Content", "Status Code", "Status"]}
                             for r in rows if not r.get("Content") or r.get("Content") == "foo"]
    write("other-populations.json", populations)
    compared = [x for x in exports.values() if x["outcome"] == "compared"]
    write("endzahlen.json", dict(sites=run["sites_present"],
          m2_sites=sum(s["present"] and s["site"].startswith("parity-m2-") for s in run["sites"]),
          compared=len(compared), passed=sum(x["report"]["passed"] for x in compared),
          failed=sum(not x["report"]["passed"] for x in compared),
          skipped=[dict(site=s["site"], **x) for s in run["sites"] for x in s["exports"] if x["outcome"] == "skipped"],
          rest_cells=sum(c["errors"] for x in compared for c in x["report"]["columns"]),
          rest_rows=sum(len(x["report"][k]) for x in compared for k in ["missing_in_os", "extra_in_os"]),
          classified_findings=len(records), verified=coverage["verified"], denominator=coverage["denominator"]))
    with gzip.open(HERE / "lauf-final.json.gz", "wb") as stream:
        stream.write(source.read_bytes())


if __name__ == "__main__":
    main()
