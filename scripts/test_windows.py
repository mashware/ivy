"""Portable rejection and payload tests for Windows packaging (not native runtime proof)."""
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import release
SPEC = importlib.util.spec_from_file_location('windows_package', SCRIPTS / 'package-windows.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class WindowsPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binaries = self.root / 'binaries'
        self.binaries.mkdir()
        data = bytearray(256)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 60, 128)
        data[128:132] = b'PE\0\0'
        struct.pack_into('<H', data, 132, 0x8664)
        for name in ('app.exe', 'panel-mcp.exe'):
            (self.binaries / name).write_bytes(data)
        (self.binaries / 'notices/fixture').mkdir(parents=True)
        (self.binaries / 'notices/fixture/LICENSE').write_bytes(b'original\r\n')
        self.payload = self.root / 'payload'

    def test_boost_fallback_preserves_original_text_and_attribution(self):
        source = self.root / 'boost-source'
        source.mkdir()
        (source / 'NOTICE').write_bytes(b'Original attribution\r\n')
        packages = [{'id': name, 'name': name, 'source': None}
                    for name in ('app', 'panel-control')]
        packages.append({'id': 'clipboard', 'name': 'clipboard-win', 'version': '5.4.1',
                         'source': 'registry+x', 'license': 'BSL-1.0',
                         'manifest_path': str(source / 'Cargo.toml'),
                         'repository': 'https://github.com/DoumanAsh/clipboard-win',
                         'authors': ['Upstream author']})
        metadata = self.root / 'metadata.json'
        metadata.write_text(json.dumps({'packages': packages,
            'workspace_members': ['app', 'panel-control'], 'resolve': {'nodes': [
                {'id': 'app', 'deps': [{'pkg': 'clipboard', 'dep_kinds': [{'kind': None}]}]},
                {'id': 'panel-control', 'deps': []}, {'id': 'clipboard', 'deps': []}]}}))
        destination = self.root / 'collected'
        release.notices(metadata, destination)
        notice = destination / 'clipboard-win-5.4.1'
        original = SCRIPTS.parent / 'resources/licenses/windows/BSL-1.0.txt'
        self.assertEqual((notice / 'SPDX-FALLBACK-BSL-1.0').read_bytes(), original.read_bytes())
        self.assertEqual((notice / 'NOTICE').read_bytes(), b'Original attribution\r\n')
        attribution = (notice / 'SPDX-FALLBACK-ATTRIBUTION.txt').read_text()
        self.assertIn('BSL-1.0 is selected', attribution)
        self.assertIn('Upstream author', attribution)
        self.assertIn('Version: 5.4.1', attribution)

    def test_stage_preserves_executables_and_notices(self):
        PACKAGE.stage(self.binaries, self.payload, '0.1.0-alpha.3')
        for before, after in (('app.exe', 'Ivy.exe'), ('panel-mcp.exe', 'panel-mcp.exe'),
                              ('notices/fixture/LICENSE', 'licenses/cargo/fixture/LICENSE')):
            self.assertEqual((self.binaries / before).read_bytes(), (self.payload / after).read_bytes())
        self.assertEqual((self.payload / 'Ivy.ico').read_bytes()[:4], b'\0\0\1\0')
        with self.assertRaisesRegex(ValueError, 'already exists'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0')

    def test_rejects_foreign_architecture_and_missing_pe(self):
        path = self.binaries / 'app.exe'
        for value in (b'ELF', b'MZ' + bytes(62)):
            path.write_bytes(value)
            with self.assertRaises(ValueError):
                PACKAGE.stage(self.binaries, self.payload, '0.1.0')
        data = bytearray(256)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 60, 128)
        data[128:132] = b'PE\0\0'
        struct.pack_into('<H', data, 132, 0xaa64)
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'not a Windows x64'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0')

    def test_rejects_missing_notices_and_invalid_version(self):
        with self.assertRaisesRegex(ValueError, 'unsupported release'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0+local')
        (self.binaries / 'notices/fixture/LICENSE').unlink()
        with self.assertRaisesRegex(ValueError, 'missing Cargo'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0')


if __name__ == '__main__':
    unittest.main()
