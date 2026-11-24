"""Native installed-image and detached-launch witnesses, only on disposable Windows CI."""
import ctypes
import json
from ctypes import wintypes
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time

if sys.platform == 'win32':
    K = ctypes.WinDLL('kernel32', use_last_error=True)
    U = ctypes.WinDLL('user32', use_last_error=True)
    K.GetConsoleWindow.restype = wintypes.HWND
    K.AttachConsole.argtypes = [wintypes.DWORD]
    K.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    K.LoadLibraryExW.restype = wintypes.HMODULE
    K.FreeLibrary.argtypes = [wintypes.HMODULE]
    K.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
    K.FindResourceW.restype = wintypes.HANDLE
    K.LoadResource.argtypes = [wintypes.HMODULE, wintypes.HANDLE]
    K.LoadResource.restype = wintypes.HANDLE
    K.LockResource.argtypes = [wintypes.HANDLE]
    K.LockResource.restype = ctypes.c_void_p
    K.SizeofResource.argtypes = [wintypes.HMODULE, wintypes.HANDLE]
    U.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    U.IsWindowVisible.argtypes = [wintypes.HWND]
    U.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    U.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM, wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)]
    U.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    U.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    U.GetForegroundWindow.restype = wintypes.HWND
    U.SetForegroundWindow.argtypes = [wintypes.HWND]
    U.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]


def resource(module, kind, name):
    handle = K.FindResourceW(module, ctypes.c_void_p(name), ctypes.c_void_p(kind))
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    size = K.SizeofResource(module, handle)
    data = K.LockResource(K.LoadResource(module, handle))
    if not data or not size:
        raise RuntimeError('empty resource')
    return ctypes.string_at(data, size)


def validate_icon(group, ico, image_resource):
    """Compare compiled group metadata and image bytes with the installed ICO."""
    if len(group) < 6 or len(ico) < 6:
        raise AssertionError('truncated icon header')
    reserved, kind, count = struct.unpack_from('<HHH', group)
    if (reserved, kind, count) != struct.unpack_from('<HHH', ico) or kind != 1 or not count:
        raise AssertionError('icon group header differs from installed icon')
    if len(group) != 6 + 14 * count or len(ico) < 6 + 16 * count:
        raise AssertionError('truncated or invalid icon directory')
    for i in range(count):
        a = struct.unpack_from('<BBBBHHIH', group, 6 + 14 * i)
        b = struct.unpack_from('<BBBBHHII', ico, 6 + 16 * i)
        image = ico[b[7]:b[7] + b[6]]
        if b[7] < 6 + 16 * count or len(image) != b[6]:
            raise AssertionError(f'icon image {i} is truncated or overlaps its directory')
        metadata = list(b[:7])
        # RC derives one color plane from PNG data, even when the ICO directory says zero.
        # LLVM 21 reproduces this for every PNG in Ivy.ico; all image bytes stay unchanged.
        if image.startswith(b'\x89PNG\r\n\x1a\n') and b[4] == 0 and a[4] == 1:
            metadata[4] = 1
        if a[:7] != tuple(metadata):
            raise AssertionError(f'icon group metadata {i} differs: {a[:7]} != {tuple(metadata)}')
        if image_resource(a[7]) != image:
            raise AssertionError(f'icon image bytes {i} differ from installed icon')


def check_icon(executable, icon):
    module = K.LoadLibraryExW(str(executable), None, 0x22)  # data file, image resource
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        validate_icon(resource(module, 14, 1), icon.read_bytes(),
                      lambda name: resource(module, 3, name))
    finally:
        K.FreeLibrary(module)


def has_console(child):
    if K.GetConsoleWindow():
        raise RuntimeError('inspector already has a console')
    if child.poll() is not None:
        raise RuntimeError('inspected process already exited')
    if K.AttachConsole(child.pid):
        K.FreeConsole()
        return True
    error = ctypes.get_last_error()
    if child.poll() is not None:
        raise RuntimeError('inspected process exited during inspection')
    if error != 6:
        raise ctypes.WinError(error)
    return False


def windows_for(pid):
    found = []
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        owner = wintypes.DWORD()
        U.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and U.IsWindowVisible(hwnd):
            title, kind = ctypes.create_unicode_buffer(512), ctypes.create_unicode_buffer(256)
            U.GetWindowTextW(hwnd, title, len(title))
            U.GetClassNameW(hwnd, kind, len(kind))
            found.append({'handle': hwnd, 'pid': pid, 'title': title.value, 'class': kind.value})
        return True
    U.EnumWindows(callback(visit), 0)
    return found


def main_window(windows):
    return next((w['handle'] for w in windows
                 if 'Ivy' in w['title'] and w['class'] != '#32770'), None)


# The same SendInput structures and Unicode key sequence as test-launch.ps1.
# Fixed-width Windows fields keep this layout testable on hosts whose C long is 64 bits.
class KeyboardInput(ctypes.Structure):
    _fields_ = [('key', ctypes.c_uint16), ('scan', ctypes.c_uint16),
                ('flags', ctypes.c_uint32), ('time', ctypes.c_uint32), ('extra', ctypes.c_size_t)]


class MouseInput(ctypes.Structure):
    _fields_ = [('x', ctypes.c_int32), ('y', ctypes.c_int32), ('data', ctypes.c_uint32),
                ('flags', ctypes.c_uint32), ('time', ctypes.c_uint32), ('extra', ctypes.c_size_t)]


class InputUnion(ctypes.Union):
    _fields_ = [('keyboard', KeyboardInput), ('mouse', MouseInput)]


class Input(ctypes.Structure):
    _fields_ = [('type', ctypes.c_uint32), ('u', InputUnion)]


if sys.platform == 'win32':
    U.SendInput.argtypes = [wintypes.UINT, ctypes.POINTER(Input), ctypes.c_int]
    U.SendInput.restype = wintypes.UINT


def type_command(text):
    events = []
    def key(value, scan, flags):
        event = Input()
        event.type = 1
        event.u.keyboard = KeyboardInput(value, scan, flags, 0, 0)
        events.append(event)
    # SendInput receives UTF-16 code units, including both halves of non-BMP path characters.
    units = text.encode('utf-16-le')
    for offset in range(0, len(units), 2):
        unit = int.from_bytes(units[offset:offset + 2], 'little')
        key(0, unit, 4)
        key(0, unit, 6)
    key(13, 0, 0)
    key(13, 0, 2)
    inputs = (Input * len(events))(*events)
    if U.SendInput(len(events), inputs, ctypes.sizeof(Input)) != len(events):
        raise ctypes.WinError(ctypes.get_last_error())


def marker_matches(path, token):
    try:
        data = path.read_bytes()
    except (FileNotFoundError, PermissionError):
        return False
    # Windows PowerShell writes UTF-16; PowerShell 7 and cmd write this ASCII token as UTF-8.
    encoding = 'utf-16' if data.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig'
    try:
        return data.decode(encoding).strip() == token
    except UnicodeError:
        return False


def failure_screenshot(hwnd, target):
    rect = wintypes.RECT()
    if not U.GetWindowRect(hwnd, ctypes.byref(rect)):
        raise ctypes.WinError(ctypes.get_last_error())
    width, height = rect.right - rect.left, rect.bottom - rect.top
    if width <= 0 or height <= 0:
        raise RuntimeError('window has no screenshot bounds')
    # Reuse the native System.Drawing capture used by test-launch.ps1.
    escaped_target = str(target).replace("'", "''")
    script = ("Add-Type -AssemblyName System.Drawing; "
              f"$b=[Drawing.Bitmap]::new({width},{height}); $g=[Drawing.Graphics]::FromImage($b); "
              f"try {{ $g.CopyFromScreen({rect.left},{rect.top},0,0,$b.Size); "
              f"$b.Save('{escaped_target}',[Drawing.Imaging.ImageFormat]::Png) }} "
              "finally { $g.Dispose(); $b.Dispose() }")
    subprocess.run(['powershell.exe', '-NoProfile', '-Command', script], check=True, timeout=10)


def stop(child):
    if child.poll() is None:
        child.kill()
    child.wait(timeout=10)


def host(executable, evidence):
    if K.GetConsoleWindow():
        raise AssertionError('detached host has a console')
    with tempfile.TemporaryDirectory(prefix='ivy-console-control-') as root:
        ready = Path(root, 'ready')
        control = subprocess.Popen([sys.executable, '-c',
            'import pathlib,sys,time; pathlib.Path(sys.argv[1]).touch(); time.sleep(30)',
            str(ready)], close_fds=True)
        try:
            # CreateProcess returns before child initialization; inspect after its own ready signal.
            deadline = time.monotonic() + 10
            while not ready.exists():
                if control.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError('console positive control did not initialize')
                time.sleep(0.05)
            if not has_console(control):
                raise AssertionError('positive control did not allocate a console')
        finally:
            stop(control)
    with tempfile.TemporaryDirectory(prefix='ivy-gui-') as root:
        env = os.environ.copy()
        for key in list(env):
            if key.startswith(('MEASURE_', 'IVY_MEASURE', 'IVY_BENCH')):
                del env[key]
        env['XDG_CONFIG_HOME'] = str(Path(root, 'config').resolve())
        env['XDG_STATE_HOME'] = str(Path(root, 'state').resolve())
        app = subprocess.Popen([str(executable)], env=env, close_fds=True, creationflags=0)
        stage, hwnd = 'main window', None
        def record():
            windows = windows_for(app.pid)
            with (evidence / 'stages.jsonl').open('a', encoding='utf-8') as log:
                log.write(json.dumps({'stage': stage, 'pid': app.pid,
                                      'exit': app.poll(), 'windows': windows}) + '\n')
            return windows
        try:
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError(f'Ivy exited before showing a window: {app.returncode}')
                hwnd = main_window(record())
                if hwnd:
                    break
                time.sleep(0.1)
            if not hwnd:
                raise RuntimeError('Ivy did not show a window')
            stage = 'console input readiness'
            record()
            U.SetForegroundWindow(hwnd)
            while U.GetForegroundWindow() != hwnd:
                if app.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError('could not focus Ivy main window before readiness deadline')
                time.sleep(0.05)
            marker, token = Path(root, 'input-ready.txt'), os.urandom(16).hex()
            type_command(f'echo {token} > "{marker}"')
            while not marker_matches(marker, token):
                if app.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError('Ivy console did not execute the input readiness witness')
                time.sleep(0.05)
            stage = 'responsive main window and no console'
            record()
            result = ctypes.c_size_t()
            if not U.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 5000, ctypes.byref(result)):
                raise RuntimeError('Ivy window did not respond')
            if has_console(app):
                raise AssertionError('installed Ivy allocated a console')
            stage = 'normal close'
            record()
            if not U.PostMessageW(hwnd, 0x10, 0, 0):
                raise ctypes.WinError(ctypes.get_last_error())
            if app.wait(timeout=15) != 0:
                raise RuntimeError(f'Ivy did not close cleanly: {app.returncode}')
            stage = 'closed cleanly'
            record()
        except BaseException:
            # Diagnostics are best effort and must preserve the original failed assertion.
            try:
                record()
                if hwnd:
                    failure_screenshot(hwnd, evidence / 'failure.png')
            except Exception as diagnostic:
                print(f'Failure diagnostics unavailable: {diagnostic}', flush=True)
            raise
        finally:
            stop(app)


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('IVY_DISPOSABLE_RUNNER') != 'yes':
        raise RuntimeError('requires disposable Windows CI')
    executable = Path(sys.argv[1]).resolve(strict=True)
    if len(sys.argv) > 2 and sys.argv[2] == '--host':
        host(executable, Path(sys.argv[3]))
        return
    check_icon(executable, executable.with_name('Ivy.ico'))
    evidence_root = Path(os.environ['GITHUB_WORKSPACE']) / 'windows-launch-evidence'
    evidence_root.mkdir(exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix='detached-', dir=evidence_root))
    print(f'Detached launch evidence: {evidence}', flush=True)
    log = evidence / 'host.log'
    with log.open('w+') as output:
        child = subprocess.Popen([sys.executable, __file__, str(executable), '--host', str(evidence)],
                                 creationflags=0x8, stdin=subprocess.DEVNULL,
                                 stdout=output, stderr=subprocess.STDOUT, close_fds=True)
        try:
            # Host worst-case budgets: control readiness/cleanup 10+10, app readiness 45,
            # response 5, normal close 15, failure screenshot 10, app cleanup 10 = 105 s.
            # Allow another 20 s for scheduling so the parent cannot cut off failure evidence.
            code = child.wait(timeout=125)
        finally:
            stop(child)
    if code:
        raise RuntimeError('detached launch failed:\n' + log.read_text(errors='replace'))
    print('Installed icon and detached GUI launch passed; console positive control passed.')


if __name__ == '__main__':
    main()
