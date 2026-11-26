"""Portable rejection and payload tests for Windows packaging (not native runtime proof)."""
import hashlib
import ctypes
from contextlib import redirect_stdout
import io
from unittest.mock import patch
import importlib.util
import json
import shutil
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
import zipfile
from types import SimpleNamespace

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

LIVE_SPEC = importlib.util.spec_from_file_location(
    'windows_live_hooks', SCRIPTS / 'windows-live-hooks.py')
LIVE = importlib.util.module_from_spec(LIVE_SPEC)
LIVE_SPEC.loader.exec_module(LIVE)
MUTATION_SPEC = importlib.util.spec_from_file_location(
    'windows_live_hook_mutations', SCRIPTS / 'windows-live-hook-mutations.py')
MUTATIONS = importlib.util.module_from_spec(MUTATION_SPEC)
MUTATION_SPEC.loader.exec_module(MUTATIONS)

LAUNCHER_SPEC = importlib.util.spec_from_file_location(
    'windows_installed_launcher', SCRIPTS.parent / 'packaging/windows/live-hooks-gui.py')
LAUNCHER = importlib.util.module_from_spec(LAUNCHER_SPEC)
LAUNCHER_SPEC.loader.exec_module(LAUNCHER)


class WindowsInstalledLauncherTests(unittest.TestCase):
    def test_ordered_mouse_packet_updates_cached_cursor_before_click(self):
        desktop = {76: -1920, 77: -1080, 78: 3840, 79: 2160}
        window = (-1600, -700)
        for expected in [(-1920, -1080), (-1919, -1079), (-1297, -259), (0, 0), (1919, 1079)]:
            with self.subTest(screen=expected):
                cached = (800, 400)
                down = None
                clicks = []
                packets = []

                def to_screen(_hwnd, pointer):
                    point = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.wintypes.POINT)).contents
                    point.x += window[0]
                    point.y += window[1]
                    return 1

                def send(count, pointer, size):
                    nonlocal cached, down
                    self.assertEqual(size, ctypes.sizeof(LAUNCHER.gui.Input))
                    events = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.gui.Input))
                    flags = []
                    for index in range(count):
                        self.assertEqual(events[index].type, 0)
                        mouse = events[index].u.mouse
                        flags.append(mouse.flags)
                        if mouse.flags & 1:
                            cached = (desktop[76] + mouse.x * desktop[78] // 65536,
                                      desktop[77] + mouse.y * desktop[79] // 65536)
                        if mouse.flags & 2:
                            down = cached
                        if mouse.flags & 4 and down == cached:
                            clicks.append(cached)
                    packets.append(flags)
                    return count

                api = SimpleNamespace(ClientToScreen=to_screen, GetSystemMetrics=desktop.__getitem__,
                                      SendInput=send)
                with patch.object(LAUNCHER.gui, 'U', api, create=True), \
                        patch.object(LAUNCHER.time, 'sleep') as delay:
                    LAUNCHER.click(1, expected[0] - window[0], expected[1] - window[1])
                self.assertEqual(clicks, [expected],
                                 'ordered mouse packets must click the exact requested pixel')
                self.assertEqual(packets, [[0xE001, 2, 4]],
                                 'move, no-coalesce, virtual-desktop and absolute flags precede one click')
                delay.assert_called_once_with(.4)

    def test_signed_wheel_input_reaches_both_scroll_anchors(self):
        offset = 0
        cached = None
        observed = []
        packets = []

        def send(count, pointer, _size):
            nonlocal offset, cached
            events = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.gui.Input))
            flags = []
            for index in range(count):
                mouse = events[index].u.mouse
                flags.append(mouse.flags)
                if events[index].type == 0 and mouse.flags & 1:
                    cached = (mouse.x * 1920 // 65536, mouse.y * 1080 // 65536)
                if events[index].type == 0 and mouse.flags & 0x0800:
                    lines = ctypes.c_int32(mouse.data).value / 120
                    offset = min(800, max(0, offset - lines * 40))
                    observed.append((cached, lines))
            packets.append(flags)
            return count

        api = SimpleNamespace(ClientToScreen=lambda *_: 1, SendInput=send,
                              GetSystemMetrics={76: 0, 77: 0, 78: 1920, 79: 1080}.__getitem__)
        with patch.object(LAUNCHER.gui, 'U', api, create=True), \
                patch.object(LAUNCHER.time, 'sleep'):
            LAUNCHER.wheel(1, 480, 400, -100)
            self.assertEqual(offset, 800, 'negative native wheel input must expose the bottom')
            LAUNCHER.wheel(1, 480, 400, 100)
            self.assertEqual(offset, 0, 'positive native wheel input must expose the top')
        self.assertEqual(observed, [((480, 400), -100), ((480, 400), 100)],
                         'wheel actions must use the requested cached cursor and signed delta')
        self.assertEqual(packets, [[0xE001, 0x0800], [0xE001, 0x0800]])

    def test_invalid_virtual_desktop_or_target_never_sends_input(self):
        for width, height, point, reason in [
                (0, 1080, (480, 400), 'positive'),
                (1920, 0, (480, 400), 'positive'),
                (-1, 1080, (480, 400), 'positive'),
                (1920, 1080, (-1, 400), 'outside'),
                (1920, 1080, (1920, 400), 'outside'),
                (1920, 1080, (480, 1080), 'outside')]:
            with self.subTest(width=width, height=height, point=point):
                def send(*_args):
                    self.fail('invalid desktop geometry must not insert any input')
                api = SimpleNamespace(ClientToScreen=lambda *_: 1, SendInput=send,
                                      GetSystemMetrics={76: 0, 77: 0, 78: width, 79: height}.__getitem__)
                with patch.object(LAUNCHER.gui, 'U', api, create=True), \
                        self.assertRaisesRegex((RuntimeError, ValueError), reason):
                    LAUNCHER.click(1, *point)

    def test_partial_mouse_packet_is_rejected_before_the_action_delay(self):
        api = SimpleNamespace(ClientToScreen=lambda *_: 1, SendInput=lambda count, *_: count - 1,
                              GetSystemMetrics={76: 0, 77: 0, 78: 1920, 79: 1080}.__getitem__)
        for action in [lambda: LAUNCHER.click(1, 303, 441), lambda: LAUNCHER.wheel(1, 480, 400, -100)]:
            with patch.object(LAUNCHER.gui, 'U', api, create=True), \
                    patch.object(LAUNCHER.time, 'sleep') as delay, \
                    self.assertRaisesRegex(RuntimeError, 'SendInput inserted'):
                action()
            delay.assert_not_called()

    def test_each_launcher_action_records_coordinates_and_keeps_named_captures(self):
        def position(pointer):
            point = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.wintypes.POINT)).contents
            point.x, point.y = -1297, -259
            return 1

        def to_client(_hwnd, pointer):
            point = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.wintypes.POINT)).contents
            point.x += 1600
            point.y += 700
            return 1

        def capture(_hwnd, path):
            path.write_bytes(b'portable screenshot placeholder')

        api = SimpleNamespace(GetCursorPos=position, ScreenToClient=to_client)
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(LAUNCHER.gui, 'U', api, create=True), \
                patch.object(LAUNCHER.gui, 'failure_screenshot', capture), \
                patch.object(LAUNCHER.time, 'time', return_value=12.5):
            evidence = Path(temporary)
            action = {'kind': 'click', 'at': [303, 441], 'target': 'Open a new agent console',
                      'capture': '02-launcher'}
            LAUNCHER.record_launcher_action(1, evidence, 1, action)
            LAUNCHER.record_launcher_action(1, evidence, 2,
                                            {'kind': 'text', 'target': 'OpenCode executable',
                                             'value': 'executable'})
            records = [json.loads(line) for line in (evidence / 'launcher-actions.jsonl').read_text().splitlines()]
            self.assertEqual(records[0], {'index': 1, 'action': 'click', 'target': action['target'],
                                         'requested_client': [303, 441], 'cursor_client': [303, 441],
                                         'cursor_screen': [-1297, -259], 'timestamp': 12.5},
                             'launcher action trace must preserve actual client and screen coordinates')
            self.assertIsNone(records[1]['requested_client'])
            self.assertEqual({path.name for path in evidence.glob('*.png')},
                             {'launcher-action-01-click.png', 'launcher-action-02-text.png',
                              '02-launcher.png'},
                             'each action must retain a screenshot and the existing named captures')

    def test_text_action_replaces_existing_input_with_literal_utf16(self):
        with tempfile.TemporaryDirectory() as temporary:
            value = str(Path(temporary, "client $() 'quoted' 🧪.exe"))
        field = bytearray('previous value'.encode('utf-16-le'))
        control = False
        selected = False

        def send(count, pointer, size):
            nonlocal control, selected
            self.assertEqual(size, ctypes.sizeof(LAUNCHER.gui.Input))
            events = ctypes.cast(pointer, ctypes.POINTER(LAUNCHER.gui.Input))
            for index in range(count):
                keyboard = events[index].u.keyboard
                if keyboard.key == 0x11:
                    control = keyboard.flags != 2
                elif keyboard.key == 0x41 and keyboard.flags == 0 and control:
                    selected = True
                elif keyboard.flags == 4:
                    if selected:
                        field.clear()
                        selected = False
                    field.extend(int(keyboard.scan).to_bytes(2, 'little'))
            return count

        with patch.object(LAUNCHER.gui, 'U', SimpleNamespace(SendInput=send), create=True), \
                patch.object(LAUNCHER.time, 'sleep'):
            LAUNCHER.launcher_action(1, {'kind': 'text', 'value': 'executable'},
                                     {'executable': value})
        self.assertEqual(field.decode('utf-16-le'), value,
                         'launcher text must replace the old field without evaluating its contents')

    def witness(self, records, *, require_bridge=True):
        with tempfile.TemporaryDirectory() as temporary:
            executable = Path(temporary, 'opencode.exe')
            normalized = []
            for record in records:
                entry = {key: value for key, value in record.items() if key != 'client'}
                if record.get('client'):
                    entry['ExecutablePath'] = str(executable)
                normalized.append(entry)
            result = SimpleNamespace(stdout=json.dumps(normalized))
            with patch.object(LAUNCHER.subprocess, 'run', return_value=result):
                return LAUNCHER.process_tree(1, executable, Path(temporary), 'processes',
                                             require_bridge=require_bridge)

    def test_pre_prompt_process_gate_rejects_missing_and_foreign_clients(self):
        for records in [[], [
            {'ProcessId': 2, 'ParentProcessId': 99, 'client': True},
            {'ProcessId': 99, 'ParentProcessId': 0},
        ]]:
            with self.subTest(records=records), self.assertRaisesRegex(
                    RuntimeError, 'expected one OpenCode descendant of Ivy, found 0'):
                self.witness(records, require_bridge=False)

    def test_bridge_is_optional_only_for_the_pre_prompt_gate(self):
        records = [
            {'ProcessId': 1, 'ParentProcessId': 0},
            {'ProcessId': 2, 'ParentProcessId': 1, 'client': True},
        ]
        self.assertEqual(self.witness([record.copy() for record in records], require_bridge=False), 2)
        with self.assertRaisesRegex(RuntimeError, 'expected one live MCP bridge'):
            self.witness([record.copy() for record in records])
        records.append({'ProcessId': 3, 'ParentProcessId': 2, 'ExecutablePath': 'panel-mcp.exe'})
        self.assertEqual(self.witness(records), 2)


@unittest.skipUnless(shutil.which('rustc'), 'Rust compiler unavailable for real libtest fixture')
class WindowsLiveHookDriverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='ivy-libtest-output-')
        cls.addClassCleanup(cls.temp.cleanup)
        source = Path(cls.temp.name, 'witness.rs')
        source.write_text('''
use std::io::Write;
fn diagnostic(text: &str) {
    let text = text.to_owned();
    std::thread::spawn(move || {
        writeln!(std::io::stderr().lock(), "{text}").unwrap();
    }).join().unwrap();
}
#[test]
fn noisy_witness() {
    println!("fixture stdout witness");
    diagnostic("fixture raw diagnostic witness");
}
#[test]
fn failing_witness() {
    diagnostic("intended fixture assertion");
    panic!("fixture assertion tripped");
}
''', encoding='utf-8')
        cls.executable = Path(cls.temp.name, 'witness.exe' if sys.platform == 'win32' else 'witness')
        subprocess.run([shutil.which('rustc'), '--edition=2024', '--test', str(source),
                        '-o', str(cls.executable)], check=True, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=60)
        cls.real_run = staticmethod(subprocess.run)

    def cargo_transport(self, command, **options):
        self.assertEqual(command[0], 'cargo')
        if '--' in command:
            arguments = command[command.index('--') + 1:]
        else:
            self.assertEqual(command[1], 'build')
            arguments = ['missing_witness', '--exact']
        # Keep the driver's real capture kwargs: changing only the Cargo prefix
        # makes raw-stderr merging a regression this fixture can actually see.
        return self.real_run([str(self.executable), *arguments], **options)

    def invoke_fixture(self, name):
        evidence = Path(self.temp.name, 'mutation-evidence')
        evidence.mkdir(exist_ok=True)
        with patch.object(subprocess, 'run', self.cargo_transport), \
                patch.object(MUTATIONS, 'EVIDENCE', evidence), redirect_stdout(io.StringIO()):
            return MUTATIONS.invoke(name, MUTATIONS.test_command('fixture', name), None)

    def test_native_and_vendor_commands_preserve_exact_named_result_with_real_diagnostics(self):
        evidence = Path(self.temp.name, 'native-evidence')
        with patch.object(subprocess, 'run', self.cargo_transport), \
                patch.object(LIVE, 'EVIDENCE', evidence), \
                patch.object(LIVE, 'sys', SimpleNamespace(platform='win32')), \
                patch.object(LIVE, 'APP_TESTS', ('noisy_witness',)), \
                patch.object(LIVE, 'TERM_TESTS', ()), \
                patch.object(LIVE, 'VENDOR_TESTS', ('noisy_witness',)), \
                redirect_stdout(io.StringIO()):
            LIVE.main()
        for label in ('app-1', 'vendor-1'):
            log = (evidence / f'{label}.log').read_text(encoding='utf-8')
            stdout, stderr = log.split(f'=== {label}: stderr ===\n')
            self.assertIn(f'=== {label}: stdout ===\n', stdout)
            self.assertIn('fixture stdout witness', stdout)
            self.assertNotIn('fixture raw diagnostic witness', stdout)
            self.assertIn('fixture raw diagnostic witness', stderr)

    def test_mutation_commands_accept_real_noisy_pass_and_named_assertion_only(self):
        passed = self.invoke_fixture('noisy_witness')
        self.assertTrue(MUTATIONS.passed_once('noisy_witness', *passed))
        LIVE.require_pass('noisy_witness', *passed)
        self.assertIn('fixture stdout witness', passed[1])
        self.assertNotIn('fixture raw diagnostic witness', passed[1])
        self.assertIn('fixture raw diagnostic witness', passed[2])
        failed = self.invoke_fixture('failing_witness')
        marker = 'intended fixture assertion'
        self.assertNotIn(marker, failed[1])
        self.assertIn(marker, failed[2])
        self.assertTrue(MUTATIONS.detected('failing_witness', marker, *failed))
        zero = self.invoke_fixture('missing_witness')
        for name, result in [
                ('noisy_witness', zero), ('wrong_witness', passed), ('failing_witness', failed),
                ('noisy_witness', (None, passed[1], passed[2])),
                ('noisy_witness', (0, '', passed[1]))]:
            with self.subTest(name=name, code=result[0]):
                self.assertFalse(MUTATIONS.passed_once(name, *result))
                with self.assertRaises(SystemExit):
                    LIVE.require_pass(name, *result)
        for name, result in [
                ('failing_witness', zero), ('wrong_witness', failed), ('failing_witness', passed),
                ('failing_witness', (None, failed[1], failed[2])),
                ('failing_witness', (101, '', failed[1] + failed[2])),
                ('failing_witness', (1, failed[1], 'error: could not compile fixture\n' + failed[2]))]:
            with self.subTest(name=name, code=result[0]):
                self.assertFalse(MUTATIONS.detected(name, marker, *result))


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
