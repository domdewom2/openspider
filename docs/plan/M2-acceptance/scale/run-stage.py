#!/usr/bin/env python3
"""Run one local CLI scale stage. Usage: python3 -I run-stage.py N.

Run from the repository root after the release build; temporary crawl files are removed
only after measurement succeeds or fails. Requires macOS ps and caffeinate externally.
"""
import datetime
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
SCALE = Path(__file__).resolve().parent


def main():
    n = int(sys.argv[1])
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=ROOT))
    binaries = Path(metadata['target_directory']) / 'release'
    date = datetime.date.today().isoformat()
    dest = SCALE / os.environ.get('OS_SCALE_DEST', date)
    dest.mkdir(exist_ok=True)
    uptime = subprocess.check_output(['uptime'], text=True).strip()
    load = float(uptime.split('load averages:')[-1].strip().split()[0].replace(',', '.'))
    if load >= 2:
        raise RuntimeError(f'load >= 2: {uptime}')
    spec = importlib.util.spec_from_file_location('dbstat', SCALE / 'dbstat.py')
    dbstat = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(dbstat)
    with tempfile.TemporaryDirectory(prefix='openspider-scale-') as tmp:
        tmp = Path(tmp)
        site = subprocess.Popen([str(binaries/'os-testsite'), f'synthetic:n={n},links=30'], stdout=subprocess.PIPE, text=True)
        try:
            url = site.stdout.readline().strip().split(' -> ')[1]
            env = os.environ.copy()
            env.update(OPENSPIDER_HOME=str(tmp/'home'), OPENSPIDER_SECRET_STORE='memory', OS_SCALE_DIAG='1')
            with (tmp/'stderr').open('w') as log:
                start = time.monotonic()
                proc = subprocess.Popen([str(binaries/'openspider'), '--crawl', url, '--headless', '--output-folder', str(tmp/'out'), '--json-progress'], stdout=subprocess.DEVNULL, stderr=log, env=env)
                samples = []
                while proc.poll() is None:
                    if time.monotonic() - start > 5400:
                        proc.kill()
                        proc.wait()
                        raise RuntimeError('CLI exceeded 5400 seconds')
                    files = list((tmp/'home'/'crawls').glob('*.ospider'))
                    db = files[0] if files else None
                    def size(path):
                        try: return path.stat().st_size
                        except FileNotFoundError: return 0
                    ps = subprocess.run(['ps', '-o', 'rss=', '-p', str(proc.pid)], capture_output=True, text=True)
                    rss = int(ps.stdout.strip() or 0) * 1024
                    samples.append({'elapsed_s': time.monotonic()-start, 'db_bytes': size(db) if db else 0, 'wal_bytes': size(Path(str(db)+'-wal')) if db else 0, 'rss_bytes': rss})
                    time.sleep(5)
                elapsed = time.monotonic()-start
            lines = (tmp/'stderr').read_text().splitlines()
            if proc.returncode:
                raise RuntimeError(f'CLI exit {proc.returncode}: {lines[-20:]}')
            progress = []
            diag = {}
            for line in lines:
                if line.startswith('OS_SCALE_DIAG '):
                    parts = line.split(); diag[parts[1]] = {k:int(v) for k,v in (p.split('=',1) for p in parts[2:])}
                elif line.startswith('{'):
                    try: p=json.loads(line)
                    except ValueError: continue
                    if p.get('phase') in ('crawl','analysis') and 'crawled' in p: progress.append(p)
            final = max((p.get('crawled',0) for p in progress), default=0)
            if final != n: raise RuntimeError(f'expected {n} URLs, got {final}')
            files = list((tmp/'home'/'crawls').glob('*.ospider'))
            if len(files) != 1: raise RuntimeError(f'expected one crawl file, got {files}')
            storage = dbstat.measure(str(files[0]))
            export_dir = os.environ.get('OS_SCALE_EXPORT_DIR')
            if export_dir:
                subprocess.run([str(binaries/'openspider'), '--load-crawl', str(files[0]),
                                '--headless', '--output-folder', export_dir,
                                '--export-tabs', 'Internal:All', '--bulk-export', 'Links:All Inlinks'],
                               env=env, check=True, timeout=5400)
            points = [(0,0)] + [(p['crawled'],p['elapsed_s']) for p in progress if p.get('crawled',0)>0]
            def at(count):
                for (a,ta),(b,tb) in zip(points,points[1:]):
                    if a <= count <= b and b>a: return ta+(tb-ta)*(count-a)/(b-a)
                return None
            windows=[]
            for begin in range(0,n,100000):
                end=min(begin+100000,n); a=at(begin); b=at(end)
                windows.append({'from':begin,'to':end,'seconds':b-a if a is not None and b is not None else None,'urls_per_s':(end-begin)/(b-a) if a is not None and b is not None and b>a else None})
            result={'n':n,'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'uptime':uptime,'cli_elapsed_s_sampled':elapsed,'progress':progress,'windows':windows,'samples':samples,'diag':diag,'storage':storage,'measurement_notes':['Progress elapsed_s is integer seconds; window boundaries are linearly interpolated.','Sampling every 5 s can miss short peaks; process completion time can be overestimated by up to 5 s.','Parse is included in engine analyze_ns; worker sums overlap and are not wall-clock shares.']}
            (dest/f'stage-{n}.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
            top=sorted(storage['objects'],key=lambda o:o['bytes'],reverse=True)[:10]
            md=[f'# Stufe {n}', '',f'- Load: {uptime}',f'- URLs: {final}; Datenbank: {storage["file_bytes"]} Bytes ({storage["file_bytes"]/final:.1f} Bytes/URL)',f'- WAL-Spitze (5-s-Abtastung): {max(s["wal_bytes"] for s in samples)} Bytes; RSS-Spitze: {max(s["rss_bytes"] for s in samples)} Bytes',f'- Freiliste: {storage["freelist_count"]} Seiten', '', '## Durchsatzfenster', '', '| URLs | URLs/s |','|---|---:|']
            md += [f'| {w["from"]}–{w["to"]} | {w["urls_per_s"] if w["urls_per_s"] is not None else "unbekannt"} |' for w in windows]
            md += ['', '## Größte Objekte', '', '| Objekt | Bytes | Bytes/URL | ungenutzte Bytes |','|---|---:|---:|---:|']
            md += [f'| {o["name"]} | {o["bytes"]} | {o["bytes"]/final:.1f} | {o["unused"]} |' for o in top]
            (dest/f'stage-{n}.md').write_text('\n'.join(md)+'\n')
            print(f'{n}: {storage["file_bytes"]} bytes, windows={windows}, results={dest}',flush=True)
        finally:
            site.send_signal(signal.SIGINT)
            site.wait(timeout=30)


if __name__ == '__main__':
    main()
