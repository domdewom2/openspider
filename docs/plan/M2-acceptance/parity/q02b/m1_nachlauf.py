"""38 historische M1/A3-Vergleiche mit nativen os-parity-diff-Aufrufen.

Aufruf: python3 <diese Datei> target/q02b-final/report.json
Nur os-parity liest die Referenzdateien. Keine eigene Vergleichslogik.
"""
import json
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]
BASELINE = "b4aebd7c1f9cb552ae74ff8f6c7431bd439c0fed"

run_path = pathlib.Path(sys.argv[1])
run = json.loads(run_path.read_text())
historical = json.loads((ROOT / "docs/plan/M2-reports/int-17/m1-k68.json").read_text())
mapping_dir = HERE / "m1-mappings"
mapping_dir.mkdir(exist_ok=True)
checks = []
for item in historical["checks"]:
    site, file = item["site"], item["file"]
    entry = next(x for s in run["sites"] if s["site"] == site
                 for x in s["exports"] if x["ref_file"] == file)
    slug = entry["mapping"]
    mapping = mapping_dir / (slug + ".toml")
    if not mapping.exists():
        content = subprocess.check_output([
            "git", "show", f"{BASELINE}:tests/parity/mappings/{slug}.toml"])
        mapping.write_bytes(content)
    os_dir = run_path.parent / site / "os"
    candidates = list(os_dir.rglob(file))
    if not candidates:
        # Namensregel des bestehenden Runners: Export besitzt ggf. Menüpräfix.
        import tomllib
        manifest = tomllib.loads((ROOT / f"tests/golden/parity/{site}/manifest.toml").read_text())
        export = next(e for e in manifest["export"] if e["ref_file"] == file)
        filename = export.get("os_file", {
            "all_inlinks.csv": "links_all_inlinks.csv",
            "all_error_(4xx_5xx_no_response)_inlinks.csv": "response_codes_all_errors_(4xx_5xx_no_response)_inlinks.csv",
            "redirect_chains.csv": "redirects_redirect_chains.csv",
        }.get(file, file))
        candidates = list(os_dir.rglob(filename))
    if len(candidates) != 1:
        raise RuntimeError(f"OS-Ausgabe nicht eindeutig: {site}/{file}: {candidates}")
    port = {"out-basic-site": 8900, "parity-onpage": 8902,
            "parity-technical": 8903, "parity-response": 8904,
            "parity-synthetic": 8905}[site]
    out = ROOT / "target/q02b-m1" / site / file.removesuffix(".csv")
    result = subprocess.run([
        str(ROOT / "target/debug/os-parity"), "diff",
        "--ref", f"tests/golden/parity/{site}/ref/{file}",
        "--os", str(candidates[0]), "--mapping", str(mapping),
        "--ref-base", f"http://127.0.0.1:{port}",
        "--os-base", f"http://127.0.0.1:{port}",
        "--deviations", "tests/parity/deviations.toml",
        "--thresholds", "tests/parity/thresholds.toml",
        "--site", site, "--out", str(out)], capture_output=True, text=True)
    if not (out / "report.json").exists():
        raise RuntimeError(result.stderr)
    report = json.loads((out / "report.json").read_text())
    checks.append(dict(site=site, file=file, exit_code=result.returncode, report=report))
summary = dict(baseline=BASELINE, total=len(checks),
               passed=sum(c["exit_code"] == 0 for c in checks), checks=checks)
(HERE / "m1-k68.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
print(f"M1: {summary['passed']}/{summary['total']}")
