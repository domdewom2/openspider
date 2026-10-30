#!/usr/bin/env python3
"""Execute an already built integration test, without compiling during measurement."""
import os
from pathlib import Path
import sys

name, *args = sys.argv[1:]
files = [p for p in Path('target/release/deps').glob(name + '-*') if p.is_file() and os.access(p, os.X_OK) and not p.suffix]
if not files:
    sys.exit(f'No prebuilt executable for {name}; run the documented cargo test --no-run build first')
exe = max(files, key=lambda p: p.stat().st_mtime)
os.execv(str(exe), [str(exe), *args])
