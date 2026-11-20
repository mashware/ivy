Closes #162

Windows pull requests and manually dispatched verification now run full workspace Clippy and tests. The Windows installer check starts the installed app with a clean profile, verifies input reaches its actual default shell, and requires normal shutdown and a saved layout before release.

- Preserve the existing Windows startup job identifier and require its named startup/path witnesses.
- Report every intentionally unsupported Unix test by name and reason. The 379-entry inventory is checked against compiled harnesses on both platforms; existing manual ignores are accounted for separately. Neutral cursor, restoration, state and parser tests remain enabled.
- Check installed startup through its PID-owned native window, with isolated APPDATA and no inherited XDG/SHELL/measurement overrides. Exercise crash, prevented shell startup, blocked layout saving and deadline controls before the normal run; retain installer payload, replacement, removal and user-data checks.
- Make Linux held-paste premises assert instead of silently returning, and keep Linux boundary assertions Linux-only.

## Validation

- Full Linux workspace suite at `5e5c5dd`: 2,690 passed, 0 failed, 16 existing ignored cases. The final subsequent change only narrows one test gate to Linux and formats it; its named Linux test passed separately, with unchanged Linux test behavior.
- Final formatting and full workspace Clippy with `-D warnings`: passed.
- Final compiler-backed Linux omission check: 379 entries across 28 harnesses, plus two existing portable manual ignores.
- Python packaging/reporter fixtures: 38 passed. Portable PowerShell parser, C# interop compilation, saved-tree and child-environment fixtures passed. Documentation check passed.
- MSVC all-target Clippy passed for term-core, panel-control, panel-contract and host-load. Full app cross-compilation was attempted but requires the unavailable Microsoft C archive tool for ring; it is not claimed as verified.
- Independent review covered the full change and corrective deltas; no unresolved findings.

## Not verified — needs a human

Native Windows execution is pending CI. Before merging:

1. Require `windows-startup-checks` green: full native workspace Clippy/tests, all three startup and six path witnesses, and exact compiled omission accounting.
2. Require the Windows installer job green and inspect `windows-launch-evidence`: PID-owned app window, real default-shell input, exit 0, no remaining descendants, and valid nonempty `%APPDATA%/ivy/layout.json`.
3. Confirm all four negative controls reject their intended failure and payload/replacement/uninstall/user-data retention checks pass. Investigate and fix any failure before retrying.

An unattended run prepared this draft; nobody confirmed its brief. Native Windows success is not inferred from portable fixtures or cross-compilation.
