import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import jsonschema
import yaml
from validate import validate


class ManifestSchemaTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='p01-winget-')
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.assets = root / 'assets'
        self.assets.mkdir()
        self.output = root / 'manifests'
        repo = Path(__file__).resolve().parents[2]
        fixture = json.loads((repo / 'packaging/homebrew/fixtures.json').read_text())
        for name, data in fixture.items():
            (self.assets / name).write_text(data)
        subprocess.run([
            'node', str(repo / 'packaging/homebrew/generate.mjs'),
            '--directory', str(self.assets), '--output', str(self.output),
            '--mode', 'winget', '--version', '1.0.0', '--repository', 'example/openspider',
            '--package-id', 'OpenSpider.CITest',
        ], check=True, capture_output=True)

    # // spec: OUT-266 [M3]
    def test_generated_fixture_validates_all_three_manifests(self):
        with contextlib.redirect_stdout(io.StringIO()) as result:
            validate(self.output)
        self.assertIn('ok: winget schema', result.getvalue())

    def test_invalid_installer_hash_is_rejected(self):
        path = self.output / 'OpenSpider.CITest.installer.yaml'
        doc = yaml.safe_load(path.read_text())
        doc['Installers'][0]['InstallerSha256'] = 'not-a-hash'
        path.write_text(yaml.safe_dump(doc))
        with self.assertRaises(jsonschema.ValidationError):
            validate(self.output)

    def test_mixed_package_versions_are_rejected(self):
        path = self.output / 'OpenSpider.CITest.yaml'
        doc = yaml.safe_load(path.read_text())
        doc['PackageVersion'] = '2.0.0'
        path.write_text(yaml.safe_dump(doc))
        with self.assertRaisesRegex(AssertionError, 'Mixed package'):
            validate(self.output)


if __name__ == '__main__':
    unittest.main()
