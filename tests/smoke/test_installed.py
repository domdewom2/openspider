# spec: OUT-263
import importlib.util
from pathlib import Path
import tempfile
import unittest

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
