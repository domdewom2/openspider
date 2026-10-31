#!/usr/bin/env python3
"""N06: Kompatibilitätsaufruf; sämtliche verified-Regeln liegen in os-parity coverage."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[4]
subprocess.run([str(root / 'target/debug/os-parity'), 'coverage', '--root', str(root),
                '--parity-report', str(Path(sys.argv[1]).resolve()),
                '--write', str(Path(__file__).resolve().parent / 'verified.json')], check=True)
