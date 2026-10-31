#!/usr/bin/env python3
"""OUT-263: exercise the CLI discovered inside an actual installation (stdlib only)."""
import argparse
import csv
import datetime as dt
import io
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import uuid
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]


def run(args, timeout=300, **kwargs):
    result = subprocess.run([str(x) for x in args], timeout=timeout,
                            text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f'{args}: exit {result.returncode}\n{result.stdout}\n{result.stderr}')
    return result


def discover(folder):
    # Inspect the installed payload; never substitute a build-tree binary.
    candidates = [p for p in Path(folder).resolve().rglob('*') if p.is_file()
                  and re.fullmatch(r'openspider(?:-(?:aarch64|x86_64|universal)-apple-darwin)?(?:\.exe)?', p.name)]
    if len(candidates) != 1:
        raise AssertionError(f'Expected one installed CLI, found {candidates}')
    if platform.system() == 'Windows':
        binary = candidates[0].read_bytes()
        pe = int.from_bytes(binary[60:64], 'little')
        assert binary[pe:pe + 4] == b'PE\0\0'
        assert int.from_bytes(binary[pe + 4:pe + 6], 'little') == 0x8664, 'Expected x64 CLI (emulation on ARM)'
    run([candidates[0], '--version'])
    return candidates[0]


def normalize(path, base):
    """Spec 06 §0: replace fixture base, times and crawl IDs, preserving all other bytes/ordering."""
    with path.open(encoding='utf-8-sig', newline='') as source:
        rows = list(csv.reader(source))
    assert len(rows) > 1, f'Empty export: {path}'
    header = rows[0]
    assert 'Address' in header and 'Status Code' in header
    for row in rows[1:]:
        assert len(row) == len(header)
        for i, value in enumerate(row):
            value = value.replace(base, '{BASE}')
            if value:
                if header[i] in ('Response Time', 'Response Time (ms)'):
                    value = '<MS>'
                elif header[i] in ('Crawl Timestamp', 'Last-Modified', 'Last Modified'):
                    value = '<TS>'
                elif header[i] in ('Crawl ID', 'Crawl Id'):
                    value = '<ID>'
            row[i] = value
    out = io.StringIO(newline='')
    csv.writer(out, lineterminator='\n').writerows(rows)
    return out.getvalue().encode('utf-8')


def check_sitemap(folder, base):
    files = list(folder.rglob('*.xml'))
    assert files, 'No XML sitemap generated'
    locations = []
    for path in files:
        tree = ET.parse(path)
        locations += [n.text for n in tree.iter('{http://www.sitemaps.org/schemas/sitemap/0.9}loc')]
    assert base + '/' in locations and base + '/about.html' in locations, locations
    assert all(v.startswith(base + '/') for v in locations), locations


def powershell(script):
    return run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', script])


def psquote(value):
    return "'" + str(value).replace("'", "''") + "'"


def scheduler_diagnostics(system, domain, label, evidence):
    """Capture scheduler state before cleanup without hiding the original failure."""
    commands = []
    if system == 'Darwin':
        commands.append(('launchctl-print', ['launchctl', 'print', f'{domain}/{label}']))
    elif system == 'Windows':
        commands.append(('scheduled-task-info', ['powershell.exe', '-NoProfile', '-NonInteractive',
            '-Command', f'''$ErrorActionPreference='Stop'
$t=Get-ScheduledTask -TaskName {psquote(label)}
$i=$t | Get-ScheduledTaskInfo
[pscustomobject]@{{State=$t.State; LastRunTime=$i.LastRunTime; LastTaskResult=$i.LastTaskResult}} | Format-List
''']))
        commands.append(('scheduled-task-definition.xml', ['powershell.exe', '-NoProfile', '-NonInteractive',
            '-Command', f'Export-ScheduledTask -TaskName {psquote(label)}']))
    for name, command in commands:
        try:
            result = subprocess.run(command, text=True, capture_output=True, timeout=30)
            # Export-ScheduledTask declares UTF-16 in its XML header.
            encoding = 'utf-16' if name.endswith('.xml') else 'utf-8'
            (evidence / name).write_text(result.stdout, encoding=encoding)
            (evidence / (name + '.stderr')).write_text(
                f'exit={result.returncode}\n{result.stderr}', encoding='utf-8')
        except Exception as error:
            (evidence / (name + '.error')).write_text(str(error), encoding='utf-8')
    for name in ('task.stdout', 'task.stderr'):
        path = evidence / name
        if not path.exists():
            path.write_text('No task output captured (task may not have started).\n', encoding='utf-8')


def scheduled(cli, home, base, evidence, env, dry_run):
    task_id = 'r03-' + uuid.uuid4().hex[:12]
    label = 'de.openspider.task.' + task_id
    at = (dt.datetime.now().astimezone() + dt.timedelta(minutes=2)).replace(second=0, microsecond=0)
    schedules = home / 'schedules'
    schedules.mkdir(parents=True)
    output = evidence / 'scheduled-export'
    task = schedules / (task_id + '.ostask')
    # JSON strings are also valid TOML basic strings for these paths/URLs.
    q = lambda x: json.dumps(str(x))
    task.write_text(f'''version = 1
[task]
id = {q(task_id)}
name = "Installed smoke"
[task.schedule]
kind = "once"
at = {q(at.isoformat())}
[task.crawl.source]
kind = "spider"
url = {q(base + '/')}
[task.crawl.overrides]
threads = "1"
"robots.retries" = "0"
[task.output]
mode = "overwrite"
[[task.exports]]
kind = "tab"
name = "Internal:All"
dir = {q(output)}
''', encoding='utf-8')
    system = platform.system()
    plist = evidence / (label + '.plist')
    domain = f'gui/{os.getuid()}' if system == 'Darwin' else None
    registered = False
    try:
        if dry_run:
            assert system == 'Linux', 'Dry-run is reserved for Linux'
            run([cli, '--run-task', task_id], env=env)
        elif system == 'Darwin':
            payload = dict(Label=label, ProgramArguments=[str(cli), '--run-task', task_id],
                           EnvironmentVariables={k: env[k] for k in
                             ('OPENSPIDER_HOME', 'OPENSPIDER_SECRET_STORE', 'OPENSPIDER_DISABLE_UPDATE_CHECK')},
                           StartCalendarInterval=dict(Month=at.month, Day=at.day, Hour=at.hour, Minute=at.minute),
                           StandardOutPath=str(evidence / 'task.stdout'),
                           StandardErrorPath=str(evidence / 'task.stderr'))
            plist.write_bytes(plistlib.dumps(payload))
            # Set before invoking so cleanup also covers a partially successful registration.
            registered = True
            try:
                run(['launchctl', 'bootstrap', domain, plist])
            except RuntimeError as error:
                fallback = f'user/{os.getuid()}'
                (evidence / 'bootstrap-fallback.log').write_text(
                    f'{error}\nRetry bootstrap in {fallback}\n', encoding='utf-8')
                domain = fallback
                run(['launchctl', 'bootstrap', domain, plist])
            run(['launchctl', 'print', f'{domain}/{label}'])
        elif system == 'Windows':
            # Task Scheduler does not inherit the caller's OPENSPIDER_HOME.
            # A small wrapper supplies the isolated profile and invokes the installed CLI.
            wrapper = evidence / 'scheduled.ps1'
            wrapper.write_text('\n'.join(f'$env:{k} = {psquote(env[k])}' for k in
                ('OPENSPIDER_HOME', 'OPENSPIDER_SECRET_STORE', 'OPENSPIDER_DISABLE_UPDATE_CHECK')) +
                f'\n& {psquote(cli)} --run-task {psquote(task_id)} '
                f'1> {psquote(evidence / "task.stdout")} 2> {psquote(evidence / "task.stderr")}\n'
                'exit $LASTEXITCODE\n', encoding='utf-8')
            args = f'-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{wrapper}"'
            registered = True
            powershell(f'''$ErrorActionPreference='Stop'
$a=New-ScheduledTaskAction -Execute 'powershell.exe' -Argument {psquote(args)}
$t=New-ScheduledTaskTrigger -Once -At ([datetime]{psquote(at.isoformat())})
$s=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$p=New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive
Register-ScheduledTask -TaskName {psquote(label)} -Action $a -Trigger $t -Settings $s -Principal $p | Out-Null
''')
        else:
            raise AssertionError('Native scheduler requires macOS or Windows; use --dry-run on Linux')
        deadline = time.monotonic() + (30 if dry_run else 240)
        record = None
        while time.monotonic() < deadline:
            records = list((schedules / 'history' / task_id).glob('*.json'))
            if records:
                record = json.loads(records[0].read_text(encoding='utf-8'))
                if record['result'] != 'running':
                    break
            time.sleep(1)
        if not record or record['result'] == 'running':
            raise RuntimeError(f'Scheduled task {label} did not finish before the deadline; '
                               f'expected start {at.isoformat()}, last record={record!r}; '
                               f'diagnostics: {evidence}')
        if record['result'] not in ('completed', 'completed_with_warnings'):
            raise RuntimeError(f'Scheduled task {label} failed: {record!r}; diagnostics: {evidence}')
        assert record.get('exit_code') == 0, record
        exports = [Path(p) for p in record.get('exports', [])]
        assert exports and all(p.is_relative_to(output) and p.is_file() for p in exports), record
        assert any(p.name == 'internal_all.csv' for p in exports), record
        if record.get('log_path'):
            shutil.copyfile(record['log_path'], evidence / 'scheduled-task.log')
        started = dt.datetime.fromisoformat(record['started_at'].replace('Z', '+00:00'))
        delta = (started - at).total_seconds()
        if not dry_run:
            assert abs(delta) <= 60, f'Scheduler deviation {delta}s'
        (evidence / 'schedule.json').write_text(json.dumps(dict(dry_run=dry_run,
            expected=at.isoformat(), deviation_seconds=delta, record=record), indent=2), encoding='utf-8')
    except Exception:
        scheduler_diagnostics(system, domain, label, evidence)
        raise
    finally:
        primary_error = sys.exc_info()[0] is not None
        try:
            if registered and system == 'Darwin':
                run(['launchctl', 'bootout', f'{domain}/{label}'])
                result = subprocess.run(['launchctl', 'print', f'{domain}/{label}'], capture_output=True)
                assert result.returncode != 0, 'LaunchAgent still registered'
            elif registered and system == 'Windows':
                powershell(f'''$ErrorActionPreference='Stop'
Unregister-ScheduledTask -TaskName {psquote(label)} -Confirm:$false
if (Get-ScheduledTask -TaskName {psquote(label)} -ErrorAction SilentlyContinue) {{ throw 'Task remains' }}
''')
        except Exception as error:
            (evidence / 'scheduler-cleanup.error').write_text(str(error), encoding='utf-8')
            if not primary_error:
                raise
        finally:
            task.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--install-dir', type=Path, required=True)
    parser.add_argument('--testsite', type=Path, required=True)
    parser.add_argument('--chromium', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--dry-run', action='store_true')
    options = parser.parse_args()
    cli = discover(options.install_dir)
    evidence = options.output.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    chromium = options.chromium.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='openspider-smoke-') as tmp:
        home = Path(tmp)
        env = dict(os.environ, OPENSPIDER_HOME=str(home), OPENSPIDER_SECRET_STORE='memory',
                   OPENSPIDER_DISABLE_UPDATE_CHECK='1', OPENSPIDER_CHROMIUM=str(chromium))
        log = (evidence / 'testsite.log').open('w', encoding='utf-8')
        fixture = home / 'out-basic-site'
        shutil.copytree(ROOT / 'tests/fixtures/out-basic-site', fixture)
        with (fixture / 'scenario.toml').open('a', encoding='utf-8') as routes:
            routes.write('\n[[route]]\npath = "/__smoke_js_probe"\nbody = ' +
                         json.dumps((ROOT / 'tests/smoke/js-proof.html').read_text()) + '\n')
        # Existing parity port: stable response bytes, including canonical URLs/Hash.
        server = subprocess.Popen([str(options.testsite.resolve()), str(fixture), '8900'], cwd=ROOT,
                                  stdout=log, stderr=subprocess.STDOUT)
        try:
            base = None
            for _ in range(150):
                text = (evidence / 'testsite.log').read_text(encoding='utf-8')
                match = re.search(r'-> (http://127\.0\.0\.1:\d+)', text)
                if match:
                    base = match[1]
                    break
                assert server.poll() is None, text
                time.sleep(.1)
            assert base, 'Fixture server did not start'
            measurements = []
            for mode in ('text', 'javascript'):
                out = evidence / mode
                started = time.monotonic()
                result = run([cli, '--crawl', base + '/', '--set', 'threads=1', '--set', 'robots.retries=0',
                    '--set', 'extraction.http_headers=true', '--set', f'rendering.mode={mode}',
                    '--set', 'rendering.ajax_timeout_s=0.1', '--export-tabs', 'Internal:All',
                    '--create-sitemap', '--output-folder', out], env=env, timeout=600)
                (evidence / (mode + '.log')).write_text(result.stdout + result.stderr, encoding='utf-8')
                normalized = normalize(out / 'internal_all.csv', base)
                (evidence / (mode + '.csv')).write_bytes(normalized)
                actual_rows = list(csv.DictReader(io.StringIO(normalized.decode('utf-8'))))
                with (ROOT / 'tests/golden/out-basic-site/internal_all.csv').open(encoding='utf-8-sig', newline='') as golden:
                    expected_rows = list(csv.DictReader(golden))
                addresses = lambda rows: sorted((r['Address'], r['Status Code']) for r in rows)
                assert addresses(actual_rows) == addresses(expected_rows), 'Fixture URL/status coverage differs'
                assert b'Rendering fehlgeschlagen' not in normalized
                check_sitemap(out, base)
                print(f'{mode}: passed in {time.monotonic() - started:.2f}s', flush=True)
            # Repeat text crawls to reduce startup/noise effects; identical workload on both Windows runners.
            for index in range(3):
                out = evidence / f'benchmark-{index}'
                started = time.monotonic()
                run([cli, '--crawl', base + '/', '--set', 'threads=1', '--set', 'robots.retries=0',
                     '--export-tabs', 'Internal:All', '--output-folder', out], env=env, timeout=600)
                elapsed = time.monotonic() - started
                count = len(list(csv.reader(io.StringIO((out / 'internal_all.csv').read_text(encoding='utf-8-sig'))))) - 1
                assert count > 0
                measurements.append(dict(seconds=elapsed, urls=count, urls_per_second=count / elapsed))
            probe = evidence / 'js-proof'
            run([cli, '--crawl', base + '/__smoke_js_probe', '--max-urls', '1',
                 '--set', 'rendering.mode=javascript', '--set', 'rendering.ajax_timeout_s=0.1',
                 '--export-tabs', 'Internal:All', '--output-folder', probe], env=env, timeout=600)
            with (probe / 'internal_all.csv').open(encoding='utf-8-sig', newline='') as source:
                rows = list(csv.DictReader(source))
            assert len(rows) == 1 and rows[0]['Title 1'] == 'R03 rendered', rows
            scheduled(cli, home, base, evidence, env, options.dry_run)
            report = dict(platform=platform.platform(), machine=platform.machine(), cli=str(cli),
                          version=run([cli, '--version']).stdout.strip(),
                          processor=platform.processor(), cpus=os.cpu_count(), dry_run=options.dry_run,
                          PROCESSOR_ARCHITEW6432=os.environ.get('PROCESSOR_ARCHITEW6432'),
                          RUNNER_ARCH=os.environ.get('RUNNER_ARCH'),
                          browser_path=str(chromium),
                          browser_version=(powershell(f'(Get-Item -LiteralPath {psquote(chromium)}).VersionInfo.ProductVersion')
                              if platform.system() == 'Windows' else run([chromium, '--version'])).stdout.strip(),
                          measurements=measurements,
                          median_urls_per_second=statistics.median(m['urls_per_second'] for m in measurements))
            (evidence / 'result.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
            print(json.dumps(report, indent=2))
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            log.close()


if __name__ == '__main__':
    main()
