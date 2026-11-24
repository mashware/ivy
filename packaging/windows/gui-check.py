"""Native installed-image and detached-launch witnesses, only on disposable Windows CI."""
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time

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


def resource(module, kind, name):
    handle = K.FindResourceW(module, ctypes.c_void_p(name), ctypes.c_void_p(kind))
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    size = K.SizeofResource(module, handle)
    data = K.LockResource(K.LoadResource(module, handle))
    if not data or not size:
        raise RuntimeError('empty resource')
    return ctypes.string_at(data, size)


def check_icon(executable, icon):
    module = K.LoadLibraryExW(str(executable), None, 0x22)  # data file, image resource
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        group = resource(module, 14, 1)
        ico = icon.read_bytes()
        reserved, kind, count = struct.unpack_from('<HHH', group)
        if (reserved, kind, count) != struct.unpack_from('<HHH', ico) or kind != 1 or not count:
            raise AssertionError('icon group header differs from installed icon')
        for i in range(count):
            a = struct.unpack_from('<BBBBHHIH', group, 6 + 14 * i)
            b = struct.unpack_from('<BBBBHHII', ico, 6 + 16 * i)
            if a[:7] != b[:7] or resource(module, 3, a[7]) != ico[b[7]:b[7] + b[6]]:
                raise AssertionError(f'icon image {i} differs from installed icon')
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


def window_for(pid):
    found = []
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        owner = wintypes.DWORD()
        U.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and U.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True
    U.EnumWindows(callback(visit), 0)
    return found[0] if found else None


def stop(child):
    if child.poll() is None:
        child.kill()
    child.wait(timeout=10)


def host(executable):
    if K.GetConsoleWindow():
        raise AssertionError('detached host has a console')
    control = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], close_fds=True)
    try:
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
        try:
            deadline = time.monotonic() + 45
            hwnd = None
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError(f'Ivy exited before showing a window: {app.returncode}')
                hwnd = window_for(app.pid)
                if hwnd:
                    break
                time.sleep(0.1)
            if not hwnd:
                raise RuntimeError('Ivy did not show a window')
            result = ctypes.c_size_t()
            if not U.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 5000, ctypes.byref(result)):
                raise RuntimeError('Ivy window did not respond')
            if has_console(app):
                raise AssertionError('installed Ivy allocated a console')
            if not U.PostMessageW(hwnd, 0x10, 0, 0):
                raise ctypes.WinError(ctypes.get_last_error())
            if app.wait(timeout=15) != 0:
                raise RuntimeError(f'Ivy did not close cleanly: {app.returncode}')
        finally:
            stop(app)


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('IVY_DISPOSABLE_RUNNER') != 'yes':
        raise RuntimeError('requires disposable Windows CI')
    executable = Path(sys.argv[1]).resolve(strict=True)
    if len(sys.argv) > 2 and sys.argv[2] == '--host':
        host(executable)
        return
    check_icon(executable, executable.with_name('Ivy.ico'))
    with tempfile.TemporaryDirectory(prefix='ivy-gui-host-') as root:
        log = Path(root, 'host.log')
        with log.open('w+') as output:
            child = subprocess.Popen([sys.executable, __file__, str(executable), '--host'],
                                     creationflags=0x8, stdin=subprocess.DEVNULL,
                                     stdout=output, stderr=subprocess.STDOUT, close_fds=True)
            try:
                code = child.wait(timeout=90)
            finally:
                stop(child)
        if code:
            raise RuntimeError('detached launch failed:\n' + log.read_text(errors='replace'))
    print('Installed icon and detached GUI launch passed; console positive control passed.')


if __name__ == '__main__':
    main()
