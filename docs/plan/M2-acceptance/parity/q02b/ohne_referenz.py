"""B3f: explizite K2-14-Liste mit nativen coverage-Testbelegen, keine verified-Gutschrift."""
import json
import pathlib
import tomllib

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]
coverage = json.loads((ROOT / "target/q02b-spec-coverage.json").read_text())
legacy_path = ROOT / "target/q02b-spec-m1.json"
legacy = json.loads(legacy_path.read_text()) if legacy_path.exists() else {"specs": {}, "extra": {}}
excluded_tabs = {
    "JavaScript", "Structured Data", "PageSpeed", "Mobile", "Accessibility",
    "Custom Search", "Custom Extraction", "Custom JavaScript", "Analytics",
    "Search Console", "Link Metrics", "Sitemaps", "Change Detection",
}
excluded_ids = {"ONPAGE-053", "ONPAGE-057", "ONPAGE-155", "ONPAGE-156", "ONPAGE-178", "ONPAGE-180"}
reasons = {}
for milestone in ["M1", "M2"]:
    catalog = tomllib.loads((ROOT / f"docs/plan/{milestone}-filters.toml").read_text())
    for tab in catalog["tab"]:
        for row in tab["filters"]:
            spec = row["spec"]
            if tab["id"] in excluded_tabs or spec in excluded_ids:
                reasons.setdefault(spec, set()).add(f"Filter {tab['id']} / {row['id']} (M2.md 6.1, K2-14)")

# Vollständige Fachpakete ohne Referenzexport; keine extra_spec_ids, da diese
# auch gemischte IDs mit referenzfähigen Teilfunktionen enthalten.
packages = {
    "M2-R01", "M2-R02", "M2-E05", "M2-D09", "M2-D14", "M2-D15", "M2-D19",
    "M2-D20", "M2-D21", "M2-D22", "M2-D23", "M2-I01", "M2-I02",
    "M2-I03", "M2-I04", "M2-I05", "M2-C01", "M2-C02", "M2-T01",
}
for package in json.loads((ROOT / "docs/plan/M2-packages.json").read_text()):
    if package["id"] in packages:
        for spec in package["spec_ids"]:
            reasons.setdefault(spec, set()).add(f"{package['id']}: {package['title']}")

# Speicherung ist eine separate K2-14-Lücke, nicht das ganze E01-Paket.
for spec in ["CRAWL-061", "CRAWL-062", "CRAWL-063"]:
    reasons.setdefault(spec, set()).add("Header-/Cookie-/Quelltext-Speicherung, M2.md 6.1 / K2-14")

ids = []
for spec, why in sorted(reasons.items()):
    status = (coverage["specs"].get(spec) or coverage["extra"].get(spec)
              or legacy["specs"].get(spec) or legacy["extra"].get(spec) or {})
    tests = [test for test in status.get("tests", []) if test["result"] == "ok"]
    ids.append(dict(spec=spec, reason=sorted(why), verified=False,
                    status_from_coverage=status.get("status", "open"),
                    passed_fixture_tests=tests,
                    failed_tests=[test for test in status.get("tests", []) if test["result"] == "failed"],
                    evidence_complete=bool(tests) and status.get("status") in {"implemented", "verified"}))
result = dict(basis="M2.md 6.1 / K2-14; Testzuordnung durch os-parity coverage (M2, für dort nicht aufgeführte M1-IDs ergänzend M1) aus tatsächlich abgeschlossenen Tests des Workspace-Nachlaufs K2-96; der Prozessstatus und alle fehlgeschlagenen oder nicht abgeschlossenen Targets stehen separat im Workspace-Bericht",
              scope="Filterkataloge und ausdrücklich ausgeschlossene Fachfunktionen; keine Aussage über die vollständige B2/UI-Abnahme",
              count=len(ids), with_passed_fixture_test=sum(bool(i["passed_fixture_tests"]) for i in ids),
              complete_fixture_status=sum(i["evidence_complete"] for i in ids),
              open_fixture_status=[i["spec"] for i in ids if not i["evidence_complete"]], ids=ids,
              mixed_ids_note="Rendering-Teilfunktionen von ONPAGE-012/172, CRAWL-022 u.a. bleiben getrennt: kein pauschaler Referenzausschluss der ganzen ID. ONPAGE-016 betrifft ausschließlich JS-Link-Zählspalten und gehört vollständig zur K2-14-Liste.")
(HERE / "ohne-referenz.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
print(f"B3f: {result['complete_fixture_status']}/{result['count']} vollständige Fixture-Statusbelege; {result['with_passed_fixture_test']} IDs mit mindestens einem erfolgreichen Test")
