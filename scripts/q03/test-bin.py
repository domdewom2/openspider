#!/usr/bin/env python3
"""Execute a prebuilt package/test target; reject ambiguity and zero matching tests."""
import os
from pathlib import Path
import re
import subprocess
import sys

name, *args = sys.argv[1:]
package, name = name.split('/', 1) if '/' in name else (None, name)
files = []
packages = set()
for path in Path('target/release/deps').glob(name + '-*'):
    if not (path.is_file() and os.access(path, os.X_OK) and not path.suffix):
        continue
    dep = path.with_suffix('.d')
    owners = set(re.findall(r'(?:^|[\s/])crates/([^/]+)/tests/' + re.escape(name) + r'\.rs', dep.read_text())) if dep.exists() else set()
    if package is not None and package not in owners:
        continue
    files.append(path)
    packages.update(owners)
if not files:
    sys.exit(f'No prebuilt executable for {package or "*"}/{name}; run cargo test --no-run first')
if package is None and len(packages) > 1:
    sys.exit(f'Ambiguous target {name}: {sorted(packages)}; specify package/test')
exe = max(files, key=lambda path: path.stat().st_mtime)
listing = subprocess.check_output([str(exe), *args, '--list'], text=True)
if not re.search(r': (test|benchmark)$', listing, re.MULTILINE):
    sys.exit(f'No matching tests in {exe} for {args}; refusing a formal pass')
os.execv(str(exe), [str(exe), *args])
