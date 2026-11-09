#!/usr/bin/env bash
# Installed-binary smoke test under a virtual display, not a desktop integration test.
set -euo pipefail
[[ ${IVY_DISPOSABLE_CONTAINER:-} == yes && $(id -u) == 0 ]] || {
    echo 'Requires root in a disposable container and IVY_DISPOSABLE_CONTAINER=yes' >&2; exit 1;
}
if [[ -z ${DISPLAY:-} ]]; then exec xvfb-run -a "$0"; fi
tmp=$(mktemp -d)
app_pid=
cleanup() {
    status=$?
    if (( status != 0 )); then
        echo 'Installed runtime smoke failed; logs follow:' >&2
        for log in "$tmp"/*.log; do [[ ! -f $log ]] || { echo "--- $log" >&2; cat "$log" >&2; }; done
    fi
    if [[ -n $app_pid ]]; then kill "$app_pid" 2>/dev/null || :; wait "$app_pid" 2>/dev/null || :; fi
    rm -rf "$tmp"
    exit "$status"
}
trap cleanup EXIT
mkdir -m 700 "$tmp/config" "$tmp/runtime"
export XDG_CONFIG_HOME="$tmp/config" XDG_RUNTIME_DIR="$tmp/runtime" LIBGL_ALWAYS_SOFTWARE=1
export IVY_SMOKE_DIR="$tmp"
IVY_SMOKE_VERSION=${IVY_EXPECTED_VERSION:-}
if [[ -z $IVY_SMOKE_VERSION ]]; then
    IVY_SMOKE_VERSION=$(dpkg-query -W -f='${Version}' ivy-terminal | tr '~' '-')
fi
export IVY_SMOKE_VERSION
cat > "$tmp/probe.py" <<'PY'
import json, os, subprocess, sys

runtime = len(sys.argv) > 1
if runtime:
    assert os.environ.get('PANEL_ID') and os.environ.get('PANEL_DIR'), 'pane identity missing'
requests = [
    {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize'},
    {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
]
if runtime:
    requests += [
        {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call', 'params': {'name': 'panel_set',
         'arguments': {'source': 'package-smoke', 'document': {'lines': ['IVY_PACKAGE_MCP_OK']}}}},
        {'jsonrpc': '2.0', 'id': 4, 'method': 'tools/call', 'params': {'name': 'panel_get', 'arguments': {}}},
    ]
proc = subprocess.run(['/usr/bin/panel-mcp'], input=''.join(json.dumps(r) + '\n' for r in requests),
                      text=True, capture_output=True, timeout=15, check=True)
answers = [json.loads(line) for line in proc.stdout.splitlines()]
assert [a.get('id') for a in answers] == list(range(1, len(requests) + 1)), answers
assert all(a.get('jsonrpc') == '2.0' and 'error' not in a and not a['result'].get('isError') for a in answers), answers
assert answers[0]['result']['serverInfo']['version'] == os.environ['IVY_SMOKE_VERSION'], answers[0]
assert {t['name'] for t in answers[1]['result']['tools']} == {
    'panel_set', 'panel_get', 'panel_patch', 'panel_retire', 'split_open', 'split_read', 'split_close'}
helper_env = dict(os.environ)
if not runtime:
    helper_env.pop('PANEL_ID', None)
    helper_env.pop('PANEL_DIR', None)
helper = subprocess.run(['/usr/bin/ivy-session', 'attach', 'claude'], env=helper_env,
                        text=True, capture_output=True, timeout=5)
assert helper.returncode == 1 and len(helper.stdout.encode()) <= 16385, helper
assert json.loads(helper.stdout).get('error') == ('disabled' if runtime else 'outside_ivy'), helper.stdout
if runtime:
    assert 'IVY_PACKAGE_MCP_OK' in json.dumps(answers[3]['result']['content']), answers[3]
    with open(os.path.join(os.environ['IVY_SMOKE_DIR'], 'proof'), 'x') as proof:
        proof.write('pane MCP round trip passed\n')
PY
python3 "$tmp/probe.py" >"$tmp/bridge.log" 2>&1
/usr/bin/ivy >"$tmp/ivy.log" 2>&1 &
app_pid=$!
window=
for ((i=0; i<150; i++)); do
    window=$(xdotool search --onlyvisible --name '^Ivy$' 2>/dev/null | head -n 1 || :)
    [[ -z $window ]] || break
    kill -0 "$app_pid" 2>/dev/null || { echo 'Ivy exited before opening a window' >&2; exit 1; }
    sleep 0.1
done
[[ -n $window ]] || { echo 'Ivy window did not appear within 15 seconds' >&2; exit 1; }
xdotool windowfocus --sync "$window"
# mktemp creates a shell-safe path; the probe inherits the pane's own identity.
xdotool type --clearmodifiers --delay 1 "python3 $tmp/probe.py runtime >$tmp/probe.log 2>&1"
xdotool key Return
for ((i=0; i<200; i++)); do
    [[ ! -f $tmp/proof ]] || break
    kill -0 "$app_pid" 2>/dev/null || { echo 'Ivy exited during the pane smoke' >&2; exit 1; }
    sleep 0.1
done
[[ -f $tmp/proof ]] || { echo 'Pane MCP proof did not arrive within 20 seconds' >&2; exit 1; }
echo 'Installed bridge and Ivy pane MCP smoke passed (headless Xvfb).'
