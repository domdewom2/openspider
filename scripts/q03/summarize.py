#!/usr/bin/env python3
"""Collect Q03 measurement blocks, preserving missing evidence as incomplete."""
import json
from pathlib import Path
import platform
import subprocess

out = Path('target/loadtest')
blocks = {}
diagnostics = {}
for path in sorted(out.glob('perf*.json')):
    destination = diagnostics if '-errors' in path.stem or '-invalid-' in path.stem else blocks
    destination[path.stem] = json.loads(path.read_text())
for path in sorted(out.glob('cli-*-invalid-*.json')):
    diagnostics[path.stem] = json.loads(path.read_text())
cli = out / 'cli-5m.json'
if cli.exists():
    value = json.loads(cli.read_text())
    value.pop('samples', None)
    blocks['cli-5m'] = value
report = dict(platform=platform.platform(), commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
              blocks=blocks, diagnostics=diagnostics, ui_measured=False,
              reference_hardware=platform.system() == 'Darwin',
              note='Headless evidence only. PERF-004 and UI responsiveness in PERF-007 require separate app measurement.')
(out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
