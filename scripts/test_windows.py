"""Portable rejection and payload tests for Windows packaging (not native runtime proof)."""
import hashlib
import ctypes
from unittest.mock import patch
import importlib.util
import json
import shutil
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

GUI_SPEC = importlib.util.spec_from_file_location(
    'windows_gui_check', SCRIPTS.parent / 'packaging/windows/gui-check.py')
GUI = importlib.util.module_from_spec(GUI_SPEC)
GUI_SPEC.loader.exec_module(GUI)


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


class WindowsLaunchWitnessTests(unittest.TestCase):
    def test_selects_only_the_named_non_dialog_main_window(self):
        windows = [{'handle': 1, 'title': 'Default IME', 'class': 'IME'},
                   {'handle': 2, 'title': 'Ivy', 'class': '#32770'},
                   {'handle': 3, 'title': 'Ivy', 'class': 'winit'}]
        self.assertEqual(GUI.main_window(windows), 3)
        self.assertIsNone(GUI.main_window(windows[:2]))

    def test_reads_exact_readiness_marker_for_all_default_windows_shell_encodings(self):
        with tempfile.TemporaryDirectory() as root:
            marker = Path(root, 'marker')
            self.assertFalse(GUI.marker_matches(marker, 'token'))
            for encoding in ('utf-8', 'utf-8-sig', 'utf-16'):
                marker.write_bytes('token\r\n'.encode(encoding))
                self.assertTrue(GUI.marker_matches(marker, 'token'))
                self.assertFalse(GUI.marker_matches(marker, 'other'))
            marker.write_bytes(b'\xff')
            self.assertFalse(GUI.marker_matches(marker, 'token'))

    def test_sendinput_layout_and_unicode_enter_match_the_existing_native_witness(self):
        self.assertEqual(ctypes.sizeof(GUI.KeyboardInput), 24)
        self.assertEqual(ctypes.sizeof(GUI.MouseInput), 32)
        self.assertEqual(GUI.Input.u.offset, 8)
        self.assertEqual(ctypes.sizeof(GUI.Input), 40)
        captured = []
        class Native:
            def SendInput(self, count, events, size):
                self_size = ctypes.sizeof(GUI.Input)
                if size != self_size:
                    raise AssertionError('incorrect native input size')
                captured.extend((e.type, e.u.keyboard.key, e.u.keyboard.scan,
                                 e.u.keyboard.flags) for e in events)
                return count
        with patch.object(GUI, 'U', Native(), create=True):
            GUI.type_command('A\U0001f600')
        self.assertEqual(captured, [(1, 0, 65, 4), (1, 0, 65, 6),
                                    (1, 0, 0xd83d, 4), (1, 0, 0xd83d, 6),
                                    (1, 0, 0xde00, 4), (1, 0, 0xde00, 6),
                                    (1, 13, 0, 0), (1, 13, 0, 2)])


class WindowsIconTests(unittest.TestCase):
    def setUp(self):
        self.ico = (SCRIPTS.parent / 'packaging/windows/Ivy.ico').read_bytes()
        # Captured RT_GROUP_ICON from LLVM 21.1.8 llvm-rc /no-preprocess compiling
        # `1 ICON "Ivy.ico"`: planes 0 -> 1 for all four PNGs; RT_ICON bytes are exact.
        self.group = bytes.fromhex(
            '0000010004001010000001002000c4020000010020200000010020009c0700000200'
            '3030000001002000260d000003000000000001002000dd4d00000400')
        self.images = {}
        for i in range(4):
            entry = struct.unpack_from('<BBBBHHII', self.ico, 6 + 16 * i)
            self.images[i + 1] = self.ico[entry[7]:entry[7] + entry[6]]

    def test_accepts_real_compiler_png_normalization_and_exact_images(self):
        GUI.validate_icon(self.group, self.ico, self.images.__getitem__)
        unnormalized = bytearray(self.group)
        for i in range(4):
            struct.pack_into('<H', unnormalized, 6 + 14 * i + 4, 0)
        GUI.validate_icon(unnormalized, self.ico, self.images.__getitem__)

    def test_rejects_changes_to_every_group_metadata_field(self):
        for offset in (0, 1, 2, 3, 4, 6, 8):
            with self.subTest(offset=offset):
                changed = bytearray(self.group)
                changed[6 + offset] ^= 2
                with self.assertRaisesRegex(AssertionError, 'group metadata 0 differs'):
                    GUI.validate_icon(changed, self.ico, self.images.__getitem__)

    def test_rejects_changed_or_truncated_image_bytes(self):
        for name in self.images:
            original = self.images[name]
            for value in (original[:-1], original[:-1] + bytes([original[-1] ^ 1])):
                with self.subTest(name=name, length=len(value)):
                    changed = dict(self.images)
                    changed[name] = value
                    with self.assertRaisesRegex(AssertionError, 'image bytes .* differ'):
                        GUI.validate_icon(self.group, self.ico, changed.__getitem__)
        with self.assertRaisesRegex(AssertionError, 'truncated'):
            GUI.validate_icon(self.group, self.ico[:-1], self.images.__getitem__)

    def test_rejects_planes_normalization_for_non_png(self):
        changed = bytearray(self.ico)
        changed[70] = 0
        with self.assertRaisesRegex(AssertionError, 'group metadata 0 differs'):
            GUI.validate_icon(self.group, changed, self.images.__getitem__)

    def test_rejects_invalid_or_truncated_directories(self):
        for group, ico in ((self.group[:5], self.ico), (self.group, self.ico[:5]),
                           (self.group[:-1], self.ico), (self.group, self.ico[:69]),
                           (b'\x01' + self.group[1:], self.ico)):
            with self.subTest(group_length=len(group), ico_length=len(ico)):
                with self.assertRaises(AssertionError):
                    GUI.validate_icon(group, ico, self.images.__getitem__)


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
        for name, subsystem in (('app.exe', 2), ('panel-mcp.exe', 3),
                                ('ivy-session.exe', 3), ('ivy-codex.exe', 3)):
            (self.binaries / name).write_bytes(pe(subsystem=subsystem,
                                                 marker=name.encode('ascii')))
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
                              ('ivy-session.exe', 'ivy-session.exe'),
                              ('ivy-codex.exe', 'ivy-codex.exe'),
                              ('ivy-codex.exe', 'ivy-codex-shell.exe'),
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
                            ('ivy-session.exe', pe(subsystem=2)),
                            ('ivy-codex.exe', pe(subsystem=2)),
                            ('app.exe', pe()[:200]),
                            ('app.exe', pe()[:152] + b'\x0b\x01' + pe()[154:])):
            with self.subTest(name=name, value=value[152:154]):
                path = self.binaries / name
                original = path.read_bytes()
                try:
                    path.write_bytes(value)
                    with self.assertRaises(ValueError):
                        PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
                    self.assertFalse(self.payload.exists())
                finally:
                    path.write_bytes(original)
                    if self.payload.exists():
                        shutil.rmtree(self.payload)

    def test_stage_preserves_the_complete_recovery_adapter_tree(self):
        PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
        source = SCRIPTS.parent / 'integrations/session-recovery'
        installed = self.payload / 'session-recovery'
        expected = {p.relative_to(source): p.read_bytes()
                    for p in source.rglob('*') if p.is_file()}
        self.assertIn(Path('README.md'), expected)
        actual = {p.relative_to(installed): p.read_bytes()
                  for p in installed.rglob('*') if p.is_file()}
        self.assertEqual(actual, expected)

    def test_rejects_each_missing_or_invalid_session_helper_before_staging(self):
        for name in ('ivy-session.exe', 'ivy-codex.exe'):
            with self.subTest(name=name):
                path = self.binaries / name
                original = path.read_bytes()
                path.unlink()
                with self.assertRaises(FileNotFoundError):
                    PACKAGE.stage(self.binaries, self.payload, '0.1.0', self.conpty, self.pin)
                self.assertFalse(self.payload.exists())
                for invalid in (b'ELF', pe(0xaa64)):
                    path.write_bytes(invalid)
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
