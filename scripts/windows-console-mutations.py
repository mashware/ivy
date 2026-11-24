"""Prove the Windows console tests fail when their individual creation flag is removed."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CASES = [
    ('crates/app/src/git_process.rs', 'app', 'git_process::windows_tests::background_runner_does_not_allocate_a_console'),
    ('crates/panel-control/src/codex_relay.rs', 'panel-control', 'codex_relay::windows_console_tests::capability_command_does_not_allocate_a_console'),
]
for filename, package, case in CASES:
    path = ROOT / filename
    original = path.read_bytes()
    needle = b'command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW'
    if original.count(needle) != 1:
        raise RuntimeError(f'Expected exactly one creation flag in {filename}')
    try:
        path.write_bytes(original.replace(needle, b'command.creation_flags(0); // mutation'))
        run = subprocess.run(['cargo', 'test', '--locked', '-p', package, '--lib', '--', case,
                              '--exact', '--nocapture'], cwd=ROOT, capture_output=True, text=True, timeout=300)
        output = run.stdout + run.stderr
        print(output)
        if run.returncode == 0 or f'test {case} ... FAILED' not in output:
            raise RuntimeError(f'{case}: mutation did not produce the expected test failure')
    finally:
        path.write_bytes(original)
print('Both console mutations were detected.')
