#!/usr/bin/env python3
"""Recheck the 38 green Q02 M1 comparisons with the current diff engine.

B3a keeps its M1 scope. The separate full M2 run also checks M2 counters on
legacy sites; those additional product findings must not be called M1 regressions.
No reference file is opened or changed by this helper: os-parity diff reads it.
"""
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[4]
BASE = 'b4aebd7c1f9cb552ae74ff8f6c7431bd439c0fed'
BEFORE = ROOT / 'target/parity-n06-before'
OUT = Path(__file__).resolve().parent
summary = json.loads((BEFORE / 'report.json').read_text())


def original(path):
    return subprocess.check_output(['git', 'show', f'{BASE}:{path}'], cwd=ROOT, text=True)


def slug(text):
    return re.sub('[^a-z0-9]+', '_', text.lower()).strip('_')


checks = []
with tempfile.TemporaryDirectory(dir=ROOT / 'tests/parity', prefix='n06-b3a-') as temp:
    temp = Path(temp)
    deviations = temp / 'deviations.toml'
    deviations.write_text(original('tests/parity/deviations.toml'))
    thresholds = temp / 'thresholds.toml'
    thresholds.write_text(original('tests/parity/thresholds.toml'))
    for site in summary['sites']:
        if site['site'].startswith('parity-m2-'):
            continue
        import tomllib
        manifest = tomllib.loads(original(f"tests/golden/parity/{site['site']}/manifest.toml"))
        entries = {e['ref_file']: e for e in manifest['export']}
        for export in site['exports']:
            if export['outcome'] != 'compared' or not export['report']['passed']:
                continue
            entry = entries[export['ref_file']]
            mapping = temp / (export['mapping'] + '.toml')
            mapping.write_text(original('tests/parity/mappings/' + mapping.name))
            parts = export['os'].split(':')[1:]
            candidates = [entry['os_file']] if 'os_file' in entry else [
                slug(' '.join(parts)) + '.csv', slug(parts[-1]) + '.csv']
            paths = [p for p in (BEFORE / site['site'] / 'os').rglob('*.csv')
                     if p.name.replace('-', '_') in [n.replace('-', '_') for n in candidates]]
            if len(paths) != 1:
                raise RuntimeError((site['site'], export['ref_file'], paths))
            base = f"http://127.0.0.1:{manifest['port']}"
            command = [str(ROOT / 'target/debug/os-parity'), 'diff',
                       '--ref', str(ROOT / 'tests/golden/parity' / site['site'] / 'ref' / export['ref_file']),
                       '--os', str(paths[0]), '--mapping', str(mapping),
                       '--ref-base', base, '--os-base', base, '--site', site['site'],
                       '--deviations', str(deviations), '--thresholds', str(thresholds)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            checks.append(dict(site=site['site'], file=export['ref_file'], passed=result.returncode == 0))
            if result.returncode:
                print(result.stdout, result.stderr)
result = dict(baseline=BASE, definition='Q02 M1 scope, current os-parity diff; full M2 scope is reported separately',
              passed=sum(c['passed'] for c in checks), total=len(checks), checks=checks)
(OUT / 'b3a-n06.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
print(f"B3a: {result['passed']}/{result['total']}")
assert result['passed'] == result['total'] == 38
