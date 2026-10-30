#!/usr/bin/env python3
"""One process measurement, also used by the Mac reference run (Python 3)."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('name')
    parser.add_argument('--out', default='target/loadtest')
    parser.add_argument('--crawl-dir', help='collect CLI progress and crawl file sizes')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    cmd = args.command
    if cmd and cmd[0] == '--':
        cmd = cmd[1:]
    if not cmd:
        parser.error('command required after --')
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / (args.name + '.json')).write_text(json.dumps(dict(command=cmd, exit_code=None, completed=False, temp_directory=os.environ.get('TMPDIR'))) + '\n')
    start = time.monotonic()
    samples = []
    peak = 0
    stop = threading.Event()
    with (out / (args.name + '.log')).open('w') as log:
        child = subprocess.Popen(cmd, stdout=log, stderr=subprocess.PIPE, text=True)

        def read():
            for line in child.stderr:
                log.write(line)
                log.flush()
                if args.crawl_dir:
                    try:
                        value = json.loads(line)
                        if value.get('phase') in ('crawl', 'analysis') and 'crawled' in value:
                            value['wall_s'] = time.monotonic() - start
                            files = list(Path(args.crawl_dir).rglob('*.ospider*'))
                            value['file_bytes_including_wal'] = sum(p.stat().st_size for p in files if p.is_file())
                            value['crawl_main_bytes'] = max((p.stat().st_size for p in files if p.suffix == '.ospider' and 'export' not in p.relative_to(args.crawl_dir).parts), default=0)
                            samples.append(value)
                    except (ValueError, OSError):
                        pass

        def memory():
            nonlocal peak
            while not stop.wait(0.2):
                try:
                    for line in Path(f'/proc/{child.pid}/status').read_text().splitlines():
                        if line.startswith('VmHWM:'):
                            peak = max(peak, int(line.split()[1]) * 1024)
                except OSError:
                    pass

        reader = threading.Thread(target=read)
        monitor = threading.Thread(target=memory)
        reader.start()
        monitor.start()
        _, status, usage = os.wait4(child.pid, 0)
        child.returncode = os.waitstatus_to_exitcode(status)
        stop.set()
        reader.join()
        monitor.join()
    seconds = time.monotonic() - start
    # macOS reports bytes, Linux KiB. wait4 is exact at process exit.
    peak = max(peak, usage.ru_maxrss * (1 if sys.platform == 'darwin' else 1024))
    report = dict(command=cmd, exit_code=child.returncode, completed=True, temp_directory=os.environ.get("TMPDIR"), seconds=seconds,
                  peak_rss_bytes=peak, samples=samples)
    if samples:
        growing = [(0.0, 0)]
        stalls = []
        for sample in samples:
            if sample['crawled'] > growing[-1][1]:
                gap = sample['wall_s'] - growing[-1][0]
                if gap > 30:
                    stalls.append(dict(from_urls=growing[-1][1], to_urls=sample['crawled'], seconds=gap))
                growing.append((sample['wall_s'], sample['crawled']))
        windows = []
        last_t, last_n = 0.0, 0
        for boundary in range(100_000, growing[-1][1] + 1, 100_000):
            for index in range(1, len(growing)):
                t, n = growing[index]
                pt, pn = growing[index - 1]
                if pn < boundary <= n:
                    at = pt + (t - pt) * (boundary - pn) / (n - pn)
                    windows.append(dict(to_urls=boundary, seconds=at-last_t, urls_per_s=(boundary-last_n)/(at-last_t)))
                    last_t, last_n = at, boundary
                    break
        millions = []
        prev_n, prev_bytes = 0, 0
        for boundary in range(1_000_000, growing[-1][1] + 1, 1_000_000):
            sample = next(s for s in samples if s['crawled'] >= boundary)
            n, size = sample['crawled'], sample['crawl_main_bytes']
            millions.append(dict(boundary=boundary, sample_urls=n, main_bytes=size,
                                 cumulative_bytes_per_url=size/n,
                                 section_bytes_per_url=(size-prev_bytes)/(n-prev_n)))
            prev_n, prev_bytes = n, size
        report.update(million_sections=millions, crawled=growing[-1][1], crawl_seconds=growing[-1][0],
                      urls_per_s=growing[-1][1]/max(growing[-1][0], 1e-9),
                      max_progress_gap_s=max((b[0]-a[0] for a,b in zip(growing,growing[1:])), default=0),
                      stalls_over_30s=stalls, windows_100k=windows,
                      post_crawl_tail_s=seconds-growing[-1][0], final_progress=samples[-1])
    (out / (args.name + '.json')).write_text(json.dumps(report, indent=2) + '\n')
    print(f'{args.name}: exit={child.returncode}, {seconds:.2f}s, peak={peak} bytes', flush=True)
    return child.returncode


if __name__ == '__main__':
    sys.exit(main())
