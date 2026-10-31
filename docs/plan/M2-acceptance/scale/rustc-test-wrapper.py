#!/usr/bin/env python3
"""Keep local workspace test artifacts within disk budget; retain backtrace fixtures."""
import os
from pathlib import Path
import subprocess
import sys

args = sys.argv[1:]
result = subprocess.run(args)
if result.returncode:
    sys.exit(result.returncode)

def value(flag):
    for i, arg in enumerate(args):
        if arg == flag and i + 1 < len(args):
            return args[i + 1]
        if arg.startswith(flag + '='):
            return arg.split('=', 1)[1]
    return None

if '--test' in args:
    crate = value('--crate-name')
    folder = value('--out-dir')
    extra = ''
    for i, arg in enumerate(args):
        candidate = args[i + 1] if arg == '-C' and i + 1 < len(args) else arg[2:] if arg.startswith('-C') else ''
        if candidate.startswith('extra-filename='):
            extra = candidate.split('=', 1)[1]
    if crate and folder and crate not in ('logging_cli', 'logging_rotation'):
        binary = Path(folder) / (crate + extra)
        if binary.is_file() and os.access(binary, os.X_OK):
            subprocess.run(['strip', str(binary)], check=True)
