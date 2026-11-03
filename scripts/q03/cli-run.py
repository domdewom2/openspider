#!/usr/bin/env python3
"""Isolated default CLI crawl, local synthetic site, compression and cleanup."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parents[2]
os.chdir(root)
out = root / 'target/loadtest'
out.mkdir(parents=True, exist_ok=True)
size = int(os.environ.get('Q03_PAGES', '5000000'))
with tempfile.TemporaryDirectory(prefix='q03-cli-', dir=out) as tmp:
    work = Path(tmp)
    env = dict(os.environ, OPENSPIDER_HOME=str(work / 'home'), OPENSPIDER_SECRET_STORE='memory')
    site = subprocess.Popen(['target/release/os-testsite', f'synthetic:n={size},links=30'],
                            stdout=subprocess.PIPE, text=True)
    try:
        url = site.stdout.readline().strip().split(' -> ')[1]
        command = [sys.executable, 'scripts/q03/measure.py', '--crawl-dir', str(work),
                   'cli-5m', '--', 'target/release/openspider', '--headless',
                   '--crawl', url, '--progress', 'json']
        result = subprocess.run(command, env=env)
        report = json.loads((out / 'cli-5m.json').read_text())
        candidates = list((work / 'home/crawls').glob('*.ospider'))
        file = candidates[0] if len(candidates) == 1 else work / 'missing.ospider'
        report['native_crawl_files'] = [str(p.relative_to(work)) for p in candidates]
        report['site_pages'] = size
        report['synthetic_site'] = f'synthetic:n={size},links=30'
        report['compression'] = []
        if result.returncode == 0 and not file.exists():
            report['artifact_error'] = 'Expected exactly one native crawl file'
            result = subprocess.CompletedProcess(command, 2)
        if result.returncode == 0 and file.exists():
            report['crawl_file_bytes'] = file.stat().st_size
            for level in (3, 19):
                dest = work / f'crawl-{level}.zst'
                start = time.monotonic()
                subprocess.run(['zstd', f'-{level}', '-T12', '-q', '-f', str(file), '-o', str(dest)], check=True)
                report['compression'].append(dict(level=level, threads=12, seconds=time.monotonic()-start,
                    bytes=dest.stat().st_size, factor=file.stat().st_size/dest.stat().st_size))
                dest.unlink()
        report['files_deleted_after_measurement'] = True
        (out / 'cli-5m.json').write_text(json.dumps(report, indent=2)+'\n')
    finally:
        site.terminate()
        site.wait()
    sys.exit(result.returncode)
