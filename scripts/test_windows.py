"""Portable rejection and payload tests for Windows packaging (not native runtime proof)."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zipfile

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import release
SPEC = importlib.util.spec_from_file_location('windows_package', SCRIPTS / 'package-windows.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)
import conpty


def pe(machine=0x8664, marker=b'', subsystem=3):
    data = bytearray(512)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 128)
    data[128:132] = b'PE\0\0'
    struct.pack_into('<H', data, 132, machine)
    struct.pack_into('<H', data, 148, 240)
    struct.pack_into('<H', data, 152, 0x20b)
    struct.pack_into('<H', data, 220, subsystem)
    return bytes(data) + marker


# The package paths written out, not read from the pin: a wrong member in conpty.json must fail here.
DLL_MEMBER = 'runtimes/win-x64/native/conpty.dll'
HOST_MEMBER = 'build/native/runtimes/x64/OpenConsole.exe'


def pinned(dll, host):
    """A pin over the given bytes, as conpty.json spells it."""
    return {'conpty.dll': {'member': DLL_MEMBER, 'sha256': hashlib.sha256(dll).hexdigest()},
            'OpenConsole.exe': {'member': HOST_MEMBER, 'sha256': hashlib.sha256(host).hexdigest()}}


class ConptyPairTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.dll, self.host = pe(marker=b'dll'), pe(marker=b'host')
        self.out = self.root / 'pair'

    def package(self, members):
        path = self.root / 'conpty.nupkg'
        with zipfile.ZipFile(path, 'w') as archive:
            for name, data in members.items():
                archive.writestr(name, data)
        return path

    def test_extracts_the_x64_pair_flat(self):
        path = self.package({DLL_MEMBER: self.dll, HOST_MEMBER: self.host,
                             'runtimes/win-arm64/native/conpty.dll': pe(0xaa64)})
        conpty.extract(path, self.out, pinned(self.dll, self.host))
        self.assertEqual(sorted(p.name for p in self.out.iterdir()), ['OpenConsole.exe', 'conpty.dll'])
        self.assertEqual((self.out / 'conpty.dll').read_bytes(), self.dll)
        self.assertEqual((self.out / 'OpenConsole.exe').read_bytes(), self.host)

    def test_refuses_a_file_that_is_not_the_pinned_one(self):
        for name, members in (('conpty.dll', {DLL_MEMBER: pe(marker=b'other'), HOST_MEMBER: self.host}),
                              ('OpenConsole.exe', {DLL_MEMBER: self.dll, HOST_MEMBER: pe(marker=b'other')})):
            with self.subTest(name=name):
                path = self.package(members)
                with self.assertRaisesRegex(ValueError, f'{name} checksum .* differs from the pin'):
                    conpty.extract(path, self.out, pinned(self.dll, self.host))
                self.assertFalse(self.out.exists())

    def test_refuses_a_member_too_large_to_be_the_host_before_inflating_it(self):
        path = self.package({DLL_MEMBER: self.dll, HOST_MEMBER: b'\0' * (conpty.MEMBER_CEILING + 1)})
        with self.assertRaisesRegex(ValueError, 'claims .* bytes'):
            conpty.extract(path, self.out, pinned(self.dll, self.host))
        self.assertFalse(self.out.exists())

    def test_refuses_a_missing_or_foreign_member_and_writes_nothing(self):
        path = self.package({DLL_MEMBER: self.dll})
        with self.assertRaisesRegex(ValueError, 'lacks build/native/runtimes/x64/OpenConsole.exe'):
            conpty.extract(path, self.out, pinned(self.dll, self.host))
        self.assertFalse(self.out.exists())
        foreign = pe(0xaa64)
        path = self.package({DLL_MEMBER: foreign, HOST_MEMBER: self.host})
        with self.assertRaisesRegex(ValueError, 'not a Windows x64'):
            conpty.extract(path, self.out, pinned(foreign, self.host))
        self.assertFalse(self.out.exists())

    def test_the_pin_names_both_files_by_their_package_path_and_sha256(self):
        wanted = conpty.pin()
        self.assertEqual(wanted['package'], 'Microsoft.Windows.Console.ConPTY')
        self.assertRegex(wanted['version'], r'^\d+\.\d+\.\d+$')
        self.assertEqual({n: f['member'] for n, f in wanted['files'].items()},
                         {'conpty.dll': DLL_MEMBER, 'OpenConsole.exe': HOST_MEMBER})
        for entry in wanted['files'].values():
            self.assertRegex(entry['sha256'], r'^[0-9a-f]{64}$')


class WindowsPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binaries = self.root / 'binaries'
        self.binaries.mkdir()
        (self.binaries / 'app.exe').write_bytes(pe(subsystem=2))
        (self.binaries / 'panel-mcp.exe').write_bytes(pe(subsystem=3))
        (self.binaries / 'notices/fixture').mkdir(parents=True)
        (self.binaries / 'notices/fixture/LICENSE').write_bytes(b'original\r\n')
        self.payload = self.root / 'payload'
        self.conpty = self.root / 'conpty'
        self.conpty.mkdir()
        (self.conpty / 'conpty.dll').write_bytes(pe(marker=b'dll'))
        (self.conpty / 'OpenConsole.exe').write_bytes(pe(marker=b'host'))
        self.pin = pinned(pe(marker=b'dll'), pe(marker=b'host'))

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
        PACKAGE.stage(self.binaries, self.payload, '0.1.0-alpha.3', self.conpty, self.pin)
        for before, after in (('app.exe', 'Ivy.exe'), ('panel-mcp.exe', 'panel-mcp.exe'),
                              ('notices/fixture/LICENSE', 'licenses/cargo/fixture/LICENSE')):
            self.assertEqual((self.binaries / before).read_bytes(), (self.payload / after).read_bytes())
        self.assertEqual((self.payload / 'Ivy.ico').read_bytes()[:4], b'\0\0\1\0')
        for name in ('conpty.dll', 'OpenConsole.exe'):
            self.assertEqual((self.conpty / name).read_bytes(), (self.payload / name).read_bytes())
        self.assertIn(b'Microsoft Corporation', (self.payload / 'licenses/conpty/LICENSE').read_bytes())
        with self.assertRaisesRegex(ValueError, 'already exists'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)

    def test_rejects_wrong_or_malformed_subsystems(self):
        for name, value in (('app.exe', pe(subsystem=3)),
                            ('panel-mcp.exe', pe(subsystem=2)),
                            ('app.exe', pe()[:200]),
                            ('app.exe', pe()[:152] + b'\x0b\x01' + pe()[154:])):
            with self.subTest(name=name, value=value[152:154]):
                path = self.binaries / name
                original = path.read_bytes()
                path.write_bytes(value)
                with self.assertRaises(ValueError):
                    PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
                self.assertFalse(self.payload.exists())
                path.write_bytes(original)

    def test_rejects_foreign_architecture_and_missing_pe(self):
        path = self.binaries / 'app.exe'
        for value in (b'ELF', b'MZ' + bytes(62)):
            path.write_bytes(value)
            with self.assertRaises(ValueError):
                PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
        data = bytearray(256)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 60, 128)
        data[128:132] = b'PE\0\0'
        struct.pack_into('<H', data, 132, 0xaa64)
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'not a Windows x64'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)

    def test_rejects_a_missing_or_stale_conpty_pair(self):
        for name in ('conpty.dll', 'OpenConsole.exe'):
            with self.subTest(name=name):
                good = (self.conpty / name).read_bytes()
                (self.conpty / name).unlink()
                with self.assertRaises(FileNotFoundError):
                    PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
                (self.conpty / name).write_bytes(pe(marker=b'stale'))
                with self.assertRaisesRegex(ValueError, f'{name} checksum .* differs from the pin'):
                    PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
                self.assertFalse(self.payload.exists())
                (self.conpty / name).write_bytes(good)

    def test_stage_reads_the_pin_in_the_repository_when_given_none(self):
        """The path the release takes: no pin passed, so the one in packaging/windows is read."""
        pin = self.root / 'conpty.json'
        pin.write_text(json.dumps({'package': 'x', 'version': '1.0.0', 'files': self.pin}))
        real, conpty.PIN = conpty.PIN, pin
        self.addCleanup(setattr, conpty, 'PIN', real)
        PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty)
        for name in ('conpty.dll', 'OpenConsole.exe'):
            self.assertEqual((self.payload / name).read_bytes(), (self.conpty / name).read_bytes())
        pin.write_text(json.dumps({'package': 'x', 'version': '1.0.0', 'sha256': '0' * 64}))
        with self.assertRaisesRegex(ValueError, 'does not pin each file'):
            PACKAGE.stage(self.binaries, self.root / 'other', '0.1.0', self.conpty)

    def test_rejects_missing_notices_and_invalid_version(self):
        with self.assertRaisesRegex(ValueError, 'unsupported release'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0+local', self.conpty, self.pin)
        (self.binaries / 'notices/fixture/LICENSE').unlink()
        with self.assertRaisesRegex(ValueError, 'missing Cargo'):
            PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)


if __name__ == '__main__':
    unittest.main()
