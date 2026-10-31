# spec: OUT-263
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import subprocess

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('installed', ROOT / 'scripts/smoke/installed.py')
installed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installed)


class ExportContract(unittest.TestCase):
    # spec: OUT-263
    def test_normalization_preserves_content_and_row_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'export.csv'
            path.write_bytes(b'Address,Status Code,Response Time,Crawl Timestamp,Crawl ID,Hash\r\n'
                             b'http://127.0.0.1:123/a,200,0.123,2026-01-01,id-1,abc\r\n'
                             b'http://127.0.0.1:123/b,404,,,,def\r\n')
            result = installed.normalize(path, 'http://127.0.0.1:123')
            self.assertEqual(result, b'Address,Status Code,Response Time,Crawl Timestamp,Crawl ID,Hash\n'
                             b'{BASE}/a,200,<MS>,<TS>,<ID>,abc\n{BASE}/b,404,,,,def\n')
            path.write_bytes(path.read_bytes().replace(b',404,', b',200,'))
            self.assertNotEqual(result, installed.normalize(path, 'http://127.0.0.1:123'))

    # spec: OUT-263
    def test_empty_export_and_incomplete_install_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            path = folder / 'empty.csv'
            path.write_text('Address,Status Code\n')
            with self.assertRaises(AssertionError):
                installed.normalize(path, 'http://localhost:1')
            with self.assertRaises(AssertionError):
                installed.discover(folder)


class SchedulerContract(unittest.TestCase):
    # spec: OUT-263
    def test_macos_timeout_retains_diagnostics_and_uses_user_fallback(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / 'profile'
            evidence = Path(tmp) / 'evidence'
            evidence.mkdir()
            env = {k: 'test' for k in ('OPENSPIDER_HOME', 'OPENSPIDER_SECRET_STORE',
                                      'OPENSPIDER_DISABLE_UPDATE_CHECK')}

            def command(args, **kwargs):
                if args[:3] == ['launchctl', 'bootstrap', 'gui/123']:
                    raise RuntimeError('gui unavailable')
                if args[:2] == ['launchctl', 'bootout']:
                    raise RuntimeError('cleanup failed')
                return subprocess.CompletedProcess(args, 0, '', '')

            with patch.object(installed.platform, 'system', return_value='Darwin'), \
                 patch.object(installed.os, 'getuid', return_value=123, create=True), \
                 patch.object(installed, 'run', side_effect=command) as run, \
                 patch.object(installed.time, 'monotonic', side_effect=[0, 241]), \
                 patch.object(installed.subprocess, 'run', return_value=
                              subprocess.CompletedProcess([], 0, 'launch state', 'diagnostic stderr')) as inspect:
                with self.assertRaisesRegex(RuntimeError, 'did not finish before the deadline'):
                    installed.scheduled(Path('/app/openspider'), home, 'http://localhost', evidence, env, False)
            self.assertIn('gui unavailable', (evidence / 'bootstrap-fallback.log').read_text())
            self.assertTrue(any(c.args[0][:3] == ['launchctl', 'bootstrap', 'user/123'] for c in run.call_args_list))
            self.assertTrue(inspect.call_args.args[0][2].startswith('user/123/de.openspider.task.'))
            self.assertEqual((evidence / 'launchctl-print').read_text(), 'launch state')
            self.assertIn('diagnostic stderr', (evidence / 'launchctl-print.stderr').read_text())
            self.assertIn('cleanup failed', (evidence / 'scheduler-cleanup.error').read_text())
            self.assertFalse(list((home / 'schedules').glob('*.ostask')))

    # spec: OUT-263
    def test_windows_timeout_captures_state_xml_and_task_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / 'profile'
            evidence = Path(tmp) / 'evidence'
            evidence.mkdir()
            (evidence / 'task.stderr').write_text('task failure')
            env = {k: 'test' for k in ('OPENSPIDER_HOME', 'OPENSPIDER_SECRET_STORE',
                                      'OPENSPIDER_DISABLE_UPDATE_CHECK')}
            with patch.object(installed.platform, 'system', return_value='Windows'), \
                 patch.object(installed, 'powershell') as powershell, \
                 patch.object(installed.time, 'monotonic', side_effect=[0, 241]), \
                 patch.object(installed.subprocess, 'run', side_effect=[
                     subprocess.CompletedProcess([], 0, 'State LastRunTime LastTaskResult', ''),
                     subprocess.CompletedProcess([], 0, '<?xml version="1.0" encoding="UTF-16"?><Task/>', '')]) as inspect:
                with self.assertRaisesRegex(RuntimeError, 'last record=None; diagnostics:'):
                    installed.scheduled(Path('app/openspider.exe'), home, 'http://localhost', evidence, env, False)
            registration = powershell.call_args_list[0].args[0]
            self.assertIn('New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive', registration)
            self.assertIn('-Principal $p', registration)
            self.assertIn('Unregister-ScheduledTask', powershell.call_args_list[-1].args[0])
            self.assertIn('Get-ScheduledTaskInfo', inspect.call_args_list[0].args[0][-1])
            self.assertIn('Export-ScheduledTask', inspect.call_args_list[1].args[0][-1])
            import xml.etree.ElementTree as ET
            self.assertEqual(ET.parse(evidence / 'scheduled-task-definition.xml').getroot().tag, 'Task')
            self.assertEqual((evidence / 'task.stderr').read_text(), 'task failure')
            wrapper = (evidence / 'scheduled.ps1').read_text()
            self.assertIn('1> ', wrapper)
            self.assertIn('2> ', wrapper)
            self.assertTrue((evidence / 'task.stdout').exists())
            self.assertFalse(list((home / 'schedules').glob('*.ostask')))


class CrossPlatformContract(unittest.TestCase):
    # spec: OUT-263
    def test_comparison_requires_all_native_results_and_catches_regression(self):
        import json
        compare_spec = importlib.util.spec_from_file_location('compare', ROOT / 'scripts/smoke/compare.py')
        compare = importlib.util.module_from_spec(compare_spec)
        compare_spec.loader.exec_module(compare)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for runner, kinds in [('macos-15', ['dmg']), ('macos-15-intel', ['dmg']),
                                  ('windows-latest', ['msi', 'nsis']), ('windows-11-arm', ['msi', 'nsis'])]:
                for kind in kinds:
                    out = root / runner / kind
                    out.mkdir(parents=True)
                    for mode in ['text', 'javascript']:
                        (out / (mode + '.csv')).write_bytes(b'Address,Status Code\n{BASE}/,200\n')
                    (out / 'result.json').write_text(json.dumps(dict(dry_run=False, processor='fixture',
                        cpus=4, median_urls_per_second=20 if runner == 'windows-11-arm' else 100)))
            self.assertIn('20.0%', compare.compare(root))
            self.assertIn('Hardware prüfen', compare.compare(root))
            changed = root / 'windows-11-arm/nsis/javascript.csv'
            changed.write_bytes(changed.read_bytes().replace(b'200', b'500'))
            with self.assertRaises(AssertionError):
                compare.compare(root)
            changed.unlink()
            with self.assertRaises(FileNotFoundError):
                compare.compare(root)


if __name__ == '__main__':
    unittest.main()
