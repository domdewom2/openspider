"""Repeat the 38 historical M1 comparisons with current candidate exports.

Only os-parity reads reference CSV contents. Historical mappings, thresholds and
accepted deviations are extracted into a temporary directory and never changed.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--run', type=Path, required=True)
parser.add_argument('--write', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
baseline = 'b4aebd7c1f9cb552ae74ff8f6c7431bd439c0fed'
summary = json.loads((args.run / 'report.json').read_text())
checks = json.loads((root / 'docs/plan/M2-acceptance/parity/b3a-n06.json').read_text())['checks']


def historical(path):
    return subprocess.check_output(['git', 'show', f'{baseline}:{path}'], cwd=root, text=True)


def slug(text):
    return re.sub('[^a-z0-9]+', '_', text.lower()).strip('_')


results = []
with tempfile.TemporaryDirectory(prefix='n08-k68-', dir=root / 'tests/parity') as work:
    work = Path(work)
    for name in ['deviations', 'thresholds']:
        (work / f'{name}.toml').write_text(historical(f'tests/parity/{name}.toml'))
    for check in checks:
        site, filename = check['site'], check['file']
        current = next(s for s in summary['sites'] if s['site'] == site)
        export = next(e for e in current['exports'] if e['ref_file'] == filename)
        manifest = tomllib.loads(historical(f'tests/golden/parity/{site}/manifest.toml'))
        entry = next(e for e in manifest['export'] if e['ref_file'] == filename)
        mapping = work / f"{entry['mapping']}.toml"
        mapping.write_text(historical(f'tests/parity/mappings/{mapping.name}'))
        parts = export['os'].split(':')[1:]
        names = [entry['os_file']] if 'os_file' in entry else [slug(' '.join(parts)) + '.csv', slug(parts[-1]) + '.csv']
        candidates = [p for p in (args.run / site / 'os').rglob('*.csv')
                      if p.name.replace('-', '_') in [n.replace('-', '_') for n in names]]
        if len(candidates) != 1:
            raise RuntimeError((site, filename, candidates))
        base = f"http://127.0.0.1:{manifest['port']}"
        report_dir = work / 'report'
        command = [str(root / 'target/debug/os-parity'), 'diff',
                   '--ref', str(root / 'tests/golden/parity' / site / 'ref' / filename),
                   '--os', str(candidates[0]), '--mapping', str(mapping),
                   '--ref-base', base, '--os-base', base, '--site', site,
                   '--deviations', str(work / 'deviations.toml'),
                   '--thresholds', str(work / 'thresholds.toml'), '--out', str(report_dir)]
        result = subprocess.run(command, capture_output=True, text=True, cwd=root)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr)
        report = json.loads((report_dir / 'report.json').read_text())
        words = next((c for c in report['columns'] if c['os_column'] == 'Word Count'), None)
        results.append(dict(site=site, file=filename, passed=result.returncode == 0,
                            ref_rows=report['ref_rows'], os_rows=report['os_rows'],
                            url_set_identical=report['url_set_identical'],
                            cell_errors=sum(c['errors'] for c in report['columns']),
                            word_count=words))
output = dict(baseline=baseline, candidate_commit=subprocess.check_output(
    ['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    run=str(args.run), passed=sum(c['passed'] for c in results), total=len(results), checks=results)
args.write.write_text(json.dumps(output, indent=2) + '\n')
print(f"M1: {output['passed']}/{output['total']}; Word Count errors: "
      f"{sum(c['word_count']['errors'] for c in results if c['word_count'])}")
raise SystemExit(0 if output['passed'] == output['total'] else 1)
