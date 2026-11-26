"""Installed OpenCode live-hook desktop witness; disposable Windows CI only."""
import ctypes
from ctypes import wintypes
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import traceback

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / 'installed-proof'))
import responder

spec = importlib.util.spec_from_file_location('gui_check', HERE / 'gui-check.py')
gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)
if sys.platform == 'win32':
    G = ctypes.WinDLL('gdi32', use_last_error=True)
    G.GetPixel.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
    G.GetPixel.restype = wintypes.DWORD


def wait_for(predicate, seconds, stage, app):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if app.poll() is not None:
            raise RuntimeError(f'Ivy exited during {stage}: {app.returncode}')
        value = predicate()
        if value:
            return value
        time.sleep(.1)
    raise RuntimeError(f'timed out during {stage}')


def key(vk, down=True):
    event = gui.Input()
    event.type = 1
    event.u.keyboard = gui.KeyboardInput(vk, 0, 0 if down else 2, 0, 0)
    if gui.U.SendInput(1, ctypes.byref(event), ctypes.sizeof(event)) != 1:
        raise ctypes.WinError(ctypes.get_last_error())


def press(vk):
    key(vk)
    key(vk, False)


def text_only(value):
    events = []
    raw = value.encode('utf-16-le')
    for index in range(0, len(raw), 2):
        code = int.from_bytes(raw[index:index + 2], 'little')
        for flags in (4, 6):
            event = gui.Input()
            event.type = 1
            event.u.keyboard = gui.KeyboardInput(0, code, flags, 0, 0)
            events.append(event)
    data = (gui.Input * len(events))(*events)
    if gui.U.SendInput(len(events), data, ctypes.sizeof(gui.Input)) != len(events):
        raise ctypes.WinError(ctypes.get_last_error())


def require_foreground(hwnd):
    if gui.U.GetForegroundWindow() != hwnd:
        raise RuntimeError('Ivy is not the foreground window')


def mouse_at(hwnd, x, y, transitions):
    require_foreground(hwnd)
    point = wintypes.POINT(int(x), int(y))
    if not gui.U.ClientToScreen(hwnd, ctypes.byref(point)):
        raise ctypes.WinError(ctypes.get_last_error())
    left, top, width, height = (gui.U.GetSystemMetrics(metric) for metric in (76, 77, 78, 79))
    if width <= 0 or height <= 0:
        raise RuntimeError('virtual desktop dimensions must be positive')
    if not left <= point.x < left + width or not top <= point.y < top + height:
        raise ValueError('mouse target is outside the virtual desktop')
    # Aim at each pixel's centre in the 16-bit absolute coordinate space.
    absolute_x = ((point.x - left) * 2 + 1) * 65536 // (width * 2)
    absolute_y = ((point.y - top) * 2 + 1) * 65536 // (height * 2)
    move = gui.Input()
    move.type = 0
    move.u.mouse = gui.MouseInput(absolute_x, absolute_y, 0, 0xE001, 0, 0)
    events = [move]
    for flags, value in transitions:
        event = gui.Input()
        event.type = 0
        event.u.mouse = gui.MouseInput(0, 0, value, flags, 0, 0)
        events.append(event)
    # One SendInput array orders an uncoalesced virtual-desktop move before the action.
    packet = (gui.Input * len(events))(*events)
    sent = gui.U.SendInput(len(events), packet, ctypes.sizeof(gui.Input))
    if sent != len(events):
        raise RuntimeError(f'SendInput inserted {sent} of {len(events)} mouse events')


def click(hwnd, x, y):
    mouse_at(hwnd, x, y, ((2, 0), (4, 0)))
    time.sleep(.4)


def wheel(hwnd, x, y, lines):
    mouse_at(hwnd, x, y, ((0x0800, int(lines) * 120),))
    time.sleep(.4)


def launcher_action(hwnd, action, values):
    require_foreground(hwnd)
    kind = action['kind']
    if kind == 'click':
        click(hwnd, *action['at'])
    elif kind == 'scroll':
        wheel(hwnd, *action['at'], action['lines'])
    elif kind == 'text':
        key(0x11)
        press(0x41)
        key(0x11, False)
        text_only(values[action['value']])
        time.sleep(.3)
    else:
        raise ValueError(f'unknown launcher action: {kind}')


def screenshot(hwnd, evidence, name):
    gui.failure_screenshot(hwnd, evidence / (name + '.png'))


def record_launcher_action(hwnd, evidence, index, action, prefix='launcher'):
    screen = wintypes.POINT()
    if not gui.U.GetCursorPos(ctypes.byref(screen)):
        raise ctypes.WinError(ctypes.get_last_error())
    client = wintypes.POINT(screen.x, screen.y)
    if not gui.U.ScreenToClient(hwnd, ctypes.byref(client)):
        raise ctypes.WinError(ctypes.get_last_error())
    record = {'index': index, 'action': action['kind'], 'target': action['target'],
              'requested_client': action.get('at'), 'cursor_client': [client.x, client.y],
              'cursor_screen': [screen.x, screen.y], 'timestamp': time.time()}
    with (evidence / f'{prefix}-actions.jsonl').open('a', encoding='utf-8') as output:
        output.write(json.dumps(record) + '\n')
    screenshot(hwnd, evidence, f"{prefix}-action-{index:02d}-{action['kind']}")
    if capture := action.get('capture'):
        screenshot(hwnd, evidence, capture)


def selected_pixels(hwnd, origin, columns, rows):
    require_foreground(hwnd)
    dc = gui.U.GetDC(0)
    if not dc:
        raise RuntimeError('GetDC(0) failed during selector observation')
    matched = set()
    try:
        for y in rows:
            colors = []
            for x in columns:
                color = G.GetPixel(dc, origin[0] + x, origin[1] + y)
                if color == 0xFFFFFFFF:
                    raise RuntimeError(f'GetPixel failed at client ({x}, {y})')
                colors.append(color)
            # COLORREF is 0x00bbggrr; run12's selected chip is RGB(26, 55, 46).
            if all(color == 0x002E371A for color in colors):
                matched.add(y)
    finally:
        if not gui.U.ReleaseDC(0, dc):
            raise RuntimeError('ReleaseDC failed during selector observation')
    return matched


def claude_selector_row(hwnd, origin):
    matched = selected_pixels(hwnd, origin, (225, 280), range(196, 504))
    spans = []
    first = None
    for y in range(196, 505):
        if y in matched:
            if first is None:
                first = y
        elif first is not None:
            if y - first >= 12:
                spans.append((first + y - 1) // 2)
            first = None
    if len(spans) > 1:
        raise RuntimeError('ambiguous selected Claude chip rows')
    return spans[0] if spans else None


def select_observed_opencode(hwnd, evidence):
    require_foreground(hwnd)
    origin = wintypes.POINT()
    if not gui.U.ClientToScreen(hwnd, ctypes.byref(origin)):
        raise ctypes.WinError(ctypes.get_last_error())
    origin = (origin.x, origin.y)
    row = claude_selector_row(hwnd, origin)
    for step in range(25):
        if row is not None:
            break
        lines = -100 if step == 0 else 1
        wheel(hwnd, 480, 400, lines)
        record_launcher_action(hwnd, evidence, step,
                               {'kind': 'scroll', 'at': [480, 400], 'lines': lines,
                                'target': 'observe selected Claude chip'}, prefix='selector')
        row = claude_selector_row(hwnd, origin)
    if row is None:
        raise RuntimeError('selected Claude chip was not visibly found within 25 wheel actions')
    click(hwnd, 461, row)
    record_launcher_action(hwnd, evidence, 25,
                           {'kind': 'click', 'at': [461, row], 'target': 'OpenCode'}, prefix='selector')
    band = range(max(196, row - 4), min(504, row + 5))
    started = time.monotonic()
    deadline = started + 5
    observations = []
    while True:
        opencode = selected_pixels(hwnd, origin, (429, 492), band)
        claude = selected_pixels(hwnd, origin, (225, 280), band)
        now = time.monotonic()
        observations.append({'elapsed': now - started, 'opencode_selected_rows': sorted(opencode),
                             'claude_selected_rows': sorted(claude)})
        (evidence / 'selector-confirmation.json').write_text(json.dumps(
            {'clicked_client': [461, row], 'observations': observations}), encoding='utf-8')
        if opencode and not claude and now <= deadline:
            break
        if now >= deadline:
            raise RuntimeError('pixel observation did not confirm OpenCode selected and Claude unselected within 5 seconds')
    screenshot(hwnd, evidence, 'selector-selected')
    (evidence / 'selector.json').write_text(json.dumps(
        {'client_origin': origin, 'clicked_client': [461, row],
         'opencode_selected_rows': sorted(opencode), 'claude_selected_rows': sorted(claude)}),
        encoding='utf-8')


def run_launcher_recipe(hwnd, recipe, values, evidence):
    selector_verified = False
    for index, action in enumerate(recipe['actions'], 1):
        try:
            if action['target'] == 'OpenCode' and action['kind'] == 'scroll':
                select_observed_opencode(hwnd, evidence)
                selector_verified = True
                continue
            if action['target'] == 'OpenCode' and action['kind'] == 'click':
                if not selector_verified:
                    raise RuntimeError('OpenCode selector was not observed before its recipe click')
                continue
            launcher_action(hwnd, action, values)
            record_launcher_action(hwnd, evidence, index, action)
        except Exception as error:
            raise RuntimeError(f"launcher: {action['target']}: {error}") from error
    if not selector_verified:
        raise RuntimeError('launcher recipe did not verify the OpenCode selector')


def process_tree(app_pid, opencode, evidence, name, *, require_bridge=True):
    script = 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,ExecutablePath | ConvertTo-Json -Compress'
    raw = subprocess.run(['powershell.exe', '-NoProfile', '-Command', script], check=True,
                         capture_output=True, text=True, timeout=20,
                         creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0)).stdout
    processes = json.loads(raw)
    by_pid = {entry['ProcessId']: entry for entry in processes}
    def descendant(pid, ancestor):
        seen = set()
        while pid in by_pid and pid not in seen:
            if pid == ancestor:
                return True
            seen.add(pid)
            pid = by_pid[pid]['ParentProcessId']
        return False
    roots = [entry for entry in processes if entry.get('ExecutablePath') and
             os.path.normcase(entry['ExecutablePath']) == os.path.normcase(str(opencode)) and
             descendant(entry['ProcessId'], app_pid)]
    if len(roots) != 1:
        raise RuntimeError(f'expected one OpenCode descendant of Ivy, found {len(roots)}')
    root = roots[0]['ProcessId']
    descendants = [entry for entry in processes if descendant(entry['ProcessId'], root)]
    bridges = [entry for entry in descendants if (entry.get('ExecutablePath') or '').lower().endswith('panel-mcp.exe')]
    (evidence / (name + '.json')).write_text(json.dumps({'ivy_pid': app_pid, 'root': roots[0], 'descendants': descendants}, indent=2), encoding='utf-8')
    if require_bridge and len(bridges) != 1:
        raise RuntimeError(f'expected one live MCP bridge under OpenCode, found {len(bridges)}')
    return root


def main():
    if sys.platform != 'win32' or os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('IVY_DISPOSABLE_RUNNER') != 'yes':
        raise RuntimeError('requires disposable Windows CI')
    ivy, opencode, evidence = (Path(value).resolve() for value in sys.argv[1:4])
    evidence.mkdir(parents=True, exist_ok=True)
    recipe = json.loads((HERE / 'installed-proof' / 'launcher-mouse.json').read_text(encoding='utf-8'))
    gui.U.SetProcessDPIAware()
    gui.U.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT]
    gui.U.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    gui.U.GetSystemMetrics.argtypes = [ctypes.c_int]
    gui.U.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
    gui.U.ScreenToClient.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    gui.U.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    gui.U.GetDC.argtypes = [wintypes.HWND]
    gui.U.GetDC.restype = wintypes.HDC
    gui.U.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
    app = server = None
    hwnd = None
    stage = 'prepare'
    with tempfile.TemporaryDirectory(prefix='ivy-installed-proof-') as temporary:
        root = Path(temporary)
        work = root / 'empty-cwd'
        work.mkdir()
        proof = root / 'proof'
        proof.mkdir()
        config = root / 'config'
        plugin_dir = config / 'plugins'
        plugin_dir.mkdir(parents=True)
        shutil.copyfile(HERE / 'installed-proof' / 'hook.js', plugin_dir / 'ivy-proof.js')
        nonce = secrets.token_hex(12)
        server = responder.serve(evidence / 'model-transcript.jsonl', nonce, proof / 'tool-name')
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        cfg = {'model': 'ivy-proof/scripted', 'small_model': 'ivy-proof/scripted',
               'enabled_providers': ['ivy-proof'], 'autoupdate': False, 'share': 'disabled', 'snapshot': False,
               'permission': {'*': 'allow'},
               'provider': {'ivy-proof': {'npm': '@ai-sdk/openai-compatible', 'env': [],
                           'options': {'baseURL': f'http://127.0.0.1:{server.server_port}/v1'},
                           'models': {'scripted': {'name': 'CI scripted responder', 'tool_call': True,
                                                  'limit': {'context': 32768, 'output': 4096}}}}},
               'mcp': {'ivy': {'type': 'local', 'command': [str(ivy.with_name('panel-mcp.exe'))], 'enabled': True}}}
        config_file = config / 'opencode.json'
        config_file.write_text(json.dumps(cfg, indent=2), encoding='utf-8')
        tui = root / 'tui.json'
        tui.write_text('{}', encoding='utf-8')
        env = os.environ.copy()
        for name in list(env):
            if any(part in name.upper() for part in ('API_KEY', 'TOKEN', 'SECRET', 'PASSWORD')) or name.upper() in ('HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'IVY_SESSION_HELPER') or name.startswith(('OPENCODE_', 'MEASURE_', 'IVY_MEASURE', 'IVY_BENCH')):
                del env[name]
        for name in ('APPDATA', 'LOCALAPPDATA', 'USERPROFILE', 'HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'OPENCODE_TEST_HOME'):
            value = root / name.lower()
            value.mkdir(exist_ok=True)
            env[name] = str(value)
        env.update(OPENCODE_CONFIG=str(config_file), OPENCODE_CONFIG_DIR=str(config), OPENCODE_TUI_CONFIG=str(tui),
                   OPENCODE_DISABLE_AUTOUPDATE='1', OPENCODE_DISABLE_MODELS_FETCH='1', OPENCODE_DISABLE_PROJECT_CONFIG='1', IVY_PROOF_DIR=str(proof),
                   IVY_PROOF_NONCE=nonce, IVY_PROOF_HELPER=str(ivy.with_name('ivy-session.exe')))
        # Pinned OpenCode's npm.install skips reification when node_modules exists and its
        # package-lock declares the requested plugin; this hook itself has no dependencies.
        for directory in (config, Path(env['XDG_CONFIG_HOME']) / 'opencode'):
            (directory / 'node_modules').mkdir(parents=True, exist_ok=True)
            (directory / 'package-lock.json').write_text(json.dumps({'packages': {'': {'dependencies': {'@opencode-ai/plugin': '1.18.31'}}}}), encoding='utf-8')
        (evidence / 'config.json').write_text(json.dumps(cfg, indent=2), encoding='utf-8')
        (evidence / 'limitations.txt').write_text('External traffic was not blocked or independently verified. Screenshots require human review.\n', encoding='utf-8')
        log = (evidence / 'ivy.log').open('w', encoding='utf-8')
        try:
            stage = 'client version'
            version = subprocess.run([str(opencode), '--version'], cwd=str(work), env=env,
                                     capture_output=True, text=True, timeout=15, check=True).stdout.strip()
            (evidence / 'version.txt').write_text(version, encoding='utf-8')
            if version != '1.18.31':
                raise RuntimeError(f'unexpected OpenCode version: {version!r}')
            app = subprocess.Popen([str(ivy)], cwd=str(work), env=env, stdout=log, stderr=subprocess.STDOUT)
            stage = 'window'
            hwnd = wait_for(lambda: gui.main_window(gui.windows_for(app.pid)), 45, stage, app)
            if not gui.U.SetWindowPos(hwnd, None, 0, 0, 960, 640, 0x14):
                raise ctypes.WinError(ctypes.get_last_error())
            gui.U.SetForegroundWindow(hwnd)
            wait_for(lambda: gui.U.GetForegroundWindow() == hwnd, 10, 'focus', app)
            rect = wintypes.RECT()
            if not gui.U.GetClientRect(hwnd, ctypes.byref(rect)):
                raise ctypes.WinError(ctypes.get_last_error())
            width, height = rect.right, rect.bottom
            (evidence / 'window.json').write_text(json.dumps({'pid': app.pid, 'width': width, 'height': height}), encoding='utf-8')
            if [width, height] != recipe['viewport']:
                raise RuntimeError('unexpected window geometry for the native launcher recipe')
            stage = 'original shell'
            marker = root / 'original-ready'
            gui.type_command(f'echo {nonce} > "{marker}"')
            wait_for(lambda: gui.marker_matches(marker, nonce), 45, stage, app)
            screenshot(hwnd, evidence, '01-original')
            stage = 'connect dialog'
            click(hwnd, width - 159, 20)
            screenshot(hwnd, evidence, '02-connect')
            stage = 'launcher recipe'
            run_launcher_recipe(hwnd, recipe, {'executable': str(opencode), 'directory': str(work)}, evidence)
            stage = 'open fresh console'
            time.sleep(12)
            launched_pid = process_tree(app.pid, opencode, evidence, 'processes-launched', require_bridge=False)
            screenshot(hwnd, evidence, '04-client')
            gui.U.SetForegroundWindow(hwnd)
            wait_for(lambda: gui.U.GetForegroundWindow() == hwnd, 10, 'proof input focus', app)
            stage = 'proof prompt'
            text_only(responder.PROMPT + nonce)
            press(13)
            wait_for(lambda: (proof / 'gate-ready.json').exists() or (proof / 'hook-error.json').exists(), 120, stage, app)
            if (proof / 'hook-error.json').exists():
                raise RuntimeError('MCP publication did not meet strict success predicate')
            shutil.copyfile(proof / 'gate-ready.json', evidence / 'gate-ready.json')
            root_pid = process_tree(app.pid, opencode, evidence, 'processes-before')
            if root_pid != launched_pid or json.loads((proof / 'gate-ready.json').read_text(encoding='utf-8')).get('pid') != root_pid:
                raise RuntimeError('hook did not run in the selected OpenCode root')
            screenshot(hwnd, evidence, '05-active-wait')
            # The initial tab remains at the left of the newly created tab.
            key(0x11); key(0x10); press(0x25); key(0x10, False); key(0x11, False)
            time.sleep(1)
            screenshot(hwnd, evidence, '06-inactive-wait')
            still_running = root / 'original-still-running'
            gui.type_command(f'echo {nonce} > "{still_running}"')
            wait_for(lambda: gui.marker_matches(still_running, nonce), 15, 'original console preservation', app)
            key(0x11); key(0x10); press(0x27); key(0x10, False); key(0x11, False)
            time.sleep(1)
            (proof / 'release').write_text('release', encoding='utf-8')
            stage = 'hook acknowledgement'
            wait_for(lambda: (proof / 'helper.json').exists(), 30, stage, app)
            result = json.loads((proof / 'helper.json').read_text(encoding='utf-8'))
            shutil.copyfile(proof / 'helper.json', evidence / 'helper.json')
            if result.get('code') != 0 or json.loads(result.get('stdout', '{}')).get('ok') is not True:
                raise RuntimeError('helper did not acknowledge publication')
            stage = 'panel readback'
            wait_for(lambda: (proof / 'readback.json').exists(), 45, stage, app)
            readback = json.loads((proof / 'readback.json').read_text(encoding='utf-8'))
            shutil.copyfile(proof / 'readback.json', evidence / 'readback.json')
            content = readback.get('result', {}).get('content', [])
            if len(content) != 1 or content[0].get('type') != 'text':
                raise RuntimeError('unexpected panel_get response')
            document = json.loads(content[0]['text'])
            if document.get('attention') is not None or 'Installed OpenCode hook witness' not in json.dumps(document):
                raise RuntimeError('panel readback does not contain cleared hook document')
            process_tree(app.pid, opencode, evidence, 'processes-after')
            screenshot(hwnd, evidence, '07-after-hook')
            (evidence / 'result.json').write_text(json.dumps({'status': 'images-require-human-review', 'ivy_pid': app.pid}), encoding='utf-8')
        except BaseException:
            (evidence / 'failure.txt').write_text(stage + '\n' + traceback.format_exc(), encoding='utf-8')
            if hwnd:
                try:
                    screenshot(hwnd, evidence, 'failure')
                except Exception:
                    pass
            for file in proof.glob('*.json'):
                shutil.copyfile(file, evidence / file.name)
            raise
        finally:
            if app is not None:
                if app.poll() is None:
                    subprocess.run(['taskkill.exe', '/PID', str(app.pid), '/T', '/F'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15, check=False)
                gui.stop(app)
            log.close()
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == '__main__':
    main()
