"""M3-R01: exercise real plist validation against generated bundle fixtures."""
import os
import pathlib
import plistlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECK = ROOT / 'scripts/release/check-macos-bundle.sh'


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.app = pathlib.Path(self.temp.name) / 'OpenSpider.app'
        for directory in ('MacOS', 'Resources'):
            (self.app / 'Contents' / directory).mkdir(parents=True)
        for name in ('OpenSpider', 'openspider'):
            (self.app / 'Contents/MacOS' / name).touch()
        (self.app / 'Contents/Resources/icon.icns').touch()
        self.info = dict(
            CFBundleIdentifier='org.openspider.desktop',
            CFBundleExecutable='OpenSpider', CFBundleIconFile='icon.icns',
            LSMinimumSystemVersion='13.0',
            CFBundleDocumentTypes=[dict(CFBundleTypeExtensions=[ext], CFBundleTypeRole='Editor')
                                   for ext in ('ospider', 'osconfig', 'osauth', 'ospreset', 'ostask')])

    def check(self):
        with (self.app / 'Contents/Info.plist').open('wb') as f:
            plistlib.dump(self.info, f)
        return subprocess.run(['bash', str(CHECK), '--plist-only', str(self.app)],
                              capture_output=True, text=True)

    # // spec: OUT-272 [M3]
    def test_file_associations_and_icon(self):
        self.assertEqual(self.check().returncode, 0)
        for ext in ('ospider', 'osconfig', 'osauth', 'ospreset', 'ostask'):
            with self.subTest(ext=ext):
                original = self.info['CFBundleDocumentTypes']
                self.info['CFBundleDocumentTypes'] = [t for t in original
                                                     if ext not in t['CFBundleTypeExtensions']]
                self.assertNotEqual(self.check().returncode, 0)
                self.info['CFBundleDocumentTypes'] = original
        (self.app / 'Contents/Resources/icon.icns').unlink()
        self.assertNotEqual(self.check().returncode, 0)

    # // spec: OUT-260
    # // spec: OUT-261
    def test_minimum_platform_and_cli(self):
        self.assertEqual(self.check().returncode, 0)
        self.info['LSMinimumSystemVersion'] = '12.0'
        self.assertNotEqual(self.check().returncode, 0)
        self.info['LSMinimumSystemVersion'] = '13.0'
        (self.app / 'Contents/MacOS/openspider').unlink()
        self.assertNotEqual(self.check().returncode, 0)

    # // spec: OUT-265
    # // spec: OUT-261 [P1]
    def test_every_macho_slice_requires_signature_runtime_timestamp_and_empty_entitlements(self):
        self.assertEqual(self.check().returncode, 0)
        (self.app / 'Contents/Resources/helper.bin').touch()
        tools = pathlib.Path(self.temp.name) / 'tools'
        tools.mkdir()
        commands = {
            'uname': 'echo Darwin',
            'file': 'case "$*" in *MacOS*|*helper.bin*) echo Mach-O;; *) echo data;; esac',
            'lipo': 'echo "x86_64 arm64"',
            'codesign': r"""
case "$*" in
  *--entitlements*)
    if [ "${BROKEN:-}" = entitlements ]; then
      echo '<plist version="1.0"><dict><key>com.apple.security.cs.allow-jit</key><true/></dict></plist>'
    else
      echo '<plist version="1.0"><dict/></plist>'
    fi ;;
  *--verify*)
    # Reject only the embedded helper, after App/CLI checks have passed.
    case "$*" in *helper.bin*) [ "${BROKEN:-}" != unsigned ];; *) true;; esac ;;
  *)
    [ "${BROKEN:-}" != adhoc ] || echo 'Signature=adhoc'
    [ "${BROKEN:-}" = runtime ] || echo 'CodeDirectory flags=0x10000(runtime)'
    [ "${BROKEN:-}" = timestamp ] || echo 'Timestamp=Oct 30 2026'
    true ;;
esac
""",
        }
        for name, code in commands.items():
            path = tools / name
            path.write_text('#!/bin/sh\n' + code + '\n')
            path.chmod(0o755)
        env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                   MACOS_EXPECTED_ARCH='universal')
        result = subprocess.run(['bash', str(CHECK), str(self.app)], env=env,
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for broken in ('unsigned', 'adhoc', 'runtime', 'timestamp', 'entitlements'):
            with self.subTest(broken=broken):
                result = subprocess.run(['bash', str(CHECK), str(self.app)],
                                        env=dict(env, BROKEN=broken), capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0, result.stdout)
        (tools / 'lipo').write_text('#!/bin/sh\necho arm64\n')
        result = subprocess.run(['bash', str(CHECK), str(self.app)], env=env,
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0, 'Universal must reject a missing Intel slice')


if __name__ == '__main__':
    unittest.main()
