//! **A shell script that puts the terminal into a mode must not print its ready marker when the
//! mode command failed**, and this is what holds it.
//!
//! It sits beside the three guards that read source rather than behaviour — [`shape`](../shape.rs)
//! for the size of a module, [`prose`](../prose.rs) and [`glossary`](../glossary.rs) for the
//! language — and it exists for the same reason they do: the rule it holds was paid for once and
//! lived nowhere but in somebody's judgement.
//!
//! # What was paid for
//!
//! Two scripts of the test bench chained their commands with `;`:
//!
//! ```text
//! stty -echo -icanon; printf '\033[?1000h…'; echo ready; cat -v
//! ```
//!
//! `;` runs the next command whatever the last one did, so the «ready» marker arrived even with
//! `stty` missing. Fifteen tests hang off those two scripts and **fourteen of them stayed green**
//! against a terminal in canonical mode with echo on — measured: 744 passed, 1 failed. They stayed
//! green because the kernel forges the very evidence they look for: with `ICANON` on the program
//! receives nothing, and the line discipline's echo writes the escape `^[`, character for character
//! what the `cat -v` inside would have printed. #158, and the same root already cost three false
//! greens in #50.
//!
//! # It reads `term-core` too, and that is the whole point
//!
//! #50 fixed this exact shape in `term-core` and left the comment explaining it — and nobody carried
//! it across to `app`, which is why #158 happened. A guard that watched one crate would repeat the
//! mistake it was written to stop, so [`MODULES`] carries both.
//!
//! # The invariant, and why it is not «no `;` in a shell literal»
//!
//! The property is narrow and it has to stay narrow, because these crates legitimately use `;`:
//! `while :; do stty size; sleep 0.05; done` is loop syntax, and `printf 'JUST WRITTEN\n'; sleep 30`
//! is a script whose failure shows up red on its own. A guard that banned the character would fire
//! on four scripts that were measured safe, and a guard that fires on safe code gets deleted.
//!
//! So a script is judged **only when both halves of the defect are present**: it *sets* a terminal
//! mode, and it later *prints* a text some test waits for. Between those two commands every
//! separator has to be `&&`. Everything else is left alone — including a `stty` that only **asks**
//! the terminal something, which is what the two loops above do and why they pass.
//!
//! # What a green here does not mean
//!
//! It does not mean the mode took. A command that exits zero is not a mode that took, and the answer
//! to that is not a guard over the source: it is
//! `confirm_the_console_is_in_the_mode_it_asked_for`, which writes a byte above `0x7f` and waits for
//! the meta notation only `cat -v` produces. Both halves were proven load-bearing by mutation —
//! `stty` replaced by `true` is the case `&&` cannot see. This guard holds the half that lives in
//! the text.

mod common;

use common::{list_against_disk, rust_sources};
use std::collections::BTreeSet;
use std::path::Path;

/// Every module the scripts can live in, pulled in **at compile time**.
///
/// `shape.rs` and `prose.rs` do the same and for the same reason: a directory walk that supplied the
/// text would turn a renamed or deleted file into a target that is silently skipped, and a guard with
/// no file to look at reads exactly like a clean one. So the list names its targets, and losing one
/// breaks the build; the walk in [`the_list_still_names_every_module`] only finds the file nobody
/// named.
///
/// **All of them and not just the two files the scripts live in today**: the bench's pieces have
/// already moved out of `app/src/lib.rs` once — the injected gestures to `src/injected.rs`, reading
/// the settings to `src/settings.rs` — and a script that moved with them would leave the guard
/// reporting on nothing.
///
/// **And that is exactly what happened next, in silence.** The delivery that took the tiling and the
/// tree out of `app/src/lib.rs` paid the four sibling lists — each of them fails when a file goes
/// missing — and did not pay this one, because this one had no way to complain. It has one now:
/// [`the_list_still_names_every_module`] checks it against the tree and the reviewers no longer have
/// to.
const MODULES: &[(&str, &str)] = &[
    (
        "app/src/ai_sessions/tests/codex_probe.rs",
        include_str!("../src/ai_sessions/tests/codex_probe.rs"),
    ),
    (
        "app/src/ai_sessions/programs/child.rs",
        include_str!("../src/ai_sessions/programs/child.rs"),
    ),
    (
        "app/src/ai_sessions/programs/child/tests.rs",
        include_str!("../src/ai_sessions/programs/child/tests.rs"),
    ),
    ("app/src/clipboard.rs", include_str!("../src/clipboard.rs")),
    (
        "app/src/pointer/clipboard.rs",
        include_str!("../src/pointer/clipboard.rs"),
    ),
    (
        "app/src/tests/context_clipboard.rs",
        include_str!("../src/tests/context_clipboard.rs"),
    ),
    (
        "app/src/tests/agent_signals/native_transport.rs",
        include_str!("../src/tests/agent_signals/native_transport.rs"),
    ),
    (
        "app/src/chrome/connection/tests.rs",
        include_str!("../src/chrome/connection/tests.rs"),
    ),
    (
        "app/src/control/applying.rs",
        include_str!("../src/control/applying.rs"),
    ),
    (
        "app/src/agent_setup.rs",
        include_str!("../src/agent_setup.rs"),
    ),
    (
        "app/src/agent_signals.rs",
        include_str!("../src/agent_signals.rs"),
    ),
    (
        "app/src/agent_signals/logos.rs",
        include_str!("../src/agent_signals/logos.rs"),
    ),
    (
        "app/src/tests/agent_signals.rs",
        include_str!("../src/tests/agent_signals.rs"),
    ),
    (
        "app/src/control/channel.rs",
        include_str!("../src/control/channel.rs"),
    ),
    (
        "panel-control/src/bin/panel-mcp.rs",
        include_str!("../../panel-control/src/bin/panel-mcp.rs"),
    ),
    (
        "panel-control/src/client.rs",
        include_str!("../../panel-control/src/client.rs"),
    ),
    (
        "panel-control/src/endpoint.rs",
        include_str!("../../panel-control/src/endpoint.rs"),
    ),
    (
        "panel-control/src/lib.rs",
        include_str!("../../panel-control/src/lib.rs"),
    ),
    (
        "panel-control/src/listener.rs",
        include_str!("../../panel-control/src/listener.rs"),
    ),
    (
        "panel-control/src/wire.rs",
        include_str!("../../panel-control/src/wire.rs"),
    ),
    (
        "app/src/ai_sessions/mod.rs",
        include_str!("../src/ai_sessions/mod.rs"),
    ),
    (
        "app/src/ai_sessions/programs.rs",
        include_str!("../src/ai_sessions/programs.rs"),
    ),
    (
        "app/src/ai_sessions/protocol.rs",
        include_str!("../src/ai_sessions/protocol.rs"),
    ),
    (
        "app/src/ai_sessions/tests.rs",
        include_str!("../src/ai_sessions/tests.rs"),
    ),
    (
        "app/src/ai_sessions/verify.rs",
        include_str!("../src/ai_sessions/verify.rs"),
    ),
    (
        "app/src/ai_sessions/worker.rs",
        include_str!("../src/ai_sessions/worker.rs"),
    ),
    (
        "app/src/chrome/connection/recovery.rs",
        include_str!("../src/chrome/connection/recovery.rs"),
    ),
    ("app/src/resume.rs", include_str!("../src/resume.rs")),
    (
        "panel-control/src/bin/ivy-session.rs",
        include_str!("../../panel-control/src/bin/ivy-session.rs"),
    ),
    (
        "panel-control/src/session.rs",
        include_str!("../../panel-control/src/session.rs"),
    ),
    (
        "panel-control/src/session/helper.rs",
        include_str!("../../panel-control/src/session/helper.rs"),
    ),
    (
        "panel-control/src/session/peer.rs",
        include_str!("../../panel-control/src/session/peer.rs"),
    ),
    (
        "panel-control/src/session/tests.rs",
        include_str!("../../panel-control/src/session/tests.rs"),
    ),
    (
        "panel-control/tests/session_helper.rs",
        include_str!("../../panel-control/tests/session_helper.rs"),
    ),
    (
        "term-core/src/foreground.rs",
        include_str!("../../term-core/src/foreground.rs"),
    ),
    (
        "term-core/src/foreground/arguments.rs",
        include_str!("../../term-core/src/foreground/arguments.rs"),
    ),
    (
        "term-core/src/foreground/linux.rs",
        include_str!("../../term-core/src/foreground/linux.rs"),
    ),
    (
        "term-core/src/foreground/tests.rs",
        include_str!("../../term-core/src/foreground/tests.rs"),
    ),
    (
        "app/src/chrome/typography.rs",
        include_str!("../src/chrome/typography.rs"),
    ),
    (
        "app/src/header/title_text.rs",
        include_str!("../src/header/title_text.rs"),
    ),
    (
        "app/src/tests/title_typography.rs",
        include_str!("../src/tests/title_typography.rs"),
    ),
    (
        "app/src/git_process.rs",
        include_str!("../src/git_process.rs"),
    ),
    ("app/src/project.rs", include_str!("../src/project.rs")),
    (
        "app/src/project/tests.rs",
        include_str!("../src/project/tests.rs"),
    ),
    (
        "app/src/project_view.rs",
        include_str!("../src/project_view.rs"),
    ),
    (
        "app/src/project_view/tests.rs",
        include_str!("../src/project_view/tests.rs"),
    ),
    (
        "app/src/chrome/menu.rs",
        include_str!("../src/chrome/menu.rs"),
    ),
    (
        "app/src/chrome/updates.rs",
        include_str!("../src/chrome/updates.rs"),
    ),
    (
        "app/src/chrome/updates/feed.rs",
        include_str!("../src/chrome/updates/feed.rs"),
    ),
    (
        "app/src/chrome/updates/service.rs",
        include_str!("../src/chrome/updates/service.rs"),
    ),
    (
        "app/src/viewer/origin.rs",
        include_str!("../src/viewer/origin.rs"),
    ),
    (
        "app/src/viewer/delivery.rs",
        include_str!("../src/viewer/delivery.rs"),
    ),
    (
        "app/src/viewer/diff.rs",
        include_str!("../src/viewer/diff.rs"),
    ),
    (
        "app/src/viewer/review.rs",
        include_str!("../src/viewer/review.rs"),
    ),
    (
        "app/src/viewer/review/message.rs",
        include_str!("../src/viewer/review/message.rs"),
    ),
    (
        "app/src/links/local.rs",
        include_str!("../src/links/local.rs"),
    ),
    (
        "term-core/src/screen/metadata.rs",
        include_str!("../../term-core/src/screen/metadata.rs"),
    ),
    (
        "term-core/src/view/local.rs",
        include_str!("../../term-core/src/view/local.rs"),
    ),
    (
        "app/src/viewer/interaction_tests.rs",
        include_str!("../src/viewer/interaction_tests.rs"),
    ),
    ("app/src/viewer.rs", include_str!("../src/viewer.rs")),
    (
        "app/src/viewer/load.rs",
        include_str!("../src/viewer/load.rs"),
    ),
    (
        "app/src/viewer/markdown.rs",
        include_str!("../src/viewer/markdown.rs"),
    ),
    (
        "app/src/viewer/tests.rs",
        include_str!("../src/viewer/tests.rs"),
    ),
    (
        "app/src/viewer/view.rs",
        include_str!("../src/viewer/view.rs"),
    ),
    (
        "app/src/console_input.rs",
        include_str!("../src/console_input.rs"),
    ),
    ("app/src/summary.rs", include_str!("../src/summary.rs")),
    (
        "app/src/summary/input.rs",
        include_str!("../src/summary/input.rs"),
    ),
    (
        "app/src/summary/model.rs",
        include_str!("../src/summary/model.rs"),
    ),
    (
        "app/src/summary/view.rs",
        include_str!("../src/summary/view.rs"),
    ),
    (
        "app/src/summary/probe.rs",
        include_str!("../src/summary/probe.rs"),
    ),
    (
        "app/src/panel/summary.rs",
        include_str!("../src/panel/summary.rs"),
    ),
    (
        "app/src/measure/overview.rs",
        include_str!("../src/measure/overview.rs"),
    ),
    (
        "app/src/tests/overview.rs",
        include_str!("../src/tests/overview.rs"),
    ),
    (
        "app/src/settings/editor/view/footer.rs",
        include_str!("../src/settings/editor/view/footer.rs"),
    ),
    (
        "app/src/settings/editor/view/typography.rs",
        include_str!("../src/settings/editor/view/typography.rs"),
    ),
    (
        "app/src/settings/editor/view/tests/usability.rs",
        include_str!("../src/settings/editor/view/tests/usability.rs"),
    ),
    (
        "app/src/settings/editor/view/tests/layout.rs",
        include_str!("../src/settings/editor/view/tests/layout.rs"),
    ),
    (
        "app/src/tab_bar/interaction.rs",
        include_str!("../src/tab_bar/interaction.rs"),
    ),
    (
        "app/src/tests/integrated_chrome.rs",
        include_str!("../src/tests/integrated_chrome.rs"),
    ),
    (
        "app/src/tests/pane_rename.rs",
        include_str!("../src/tests/pane_rename.rs"),
    ),
    (
        "app/src/chrome/branding.rs",
        include_str!("../src/chrome/branding.rs"),
    ),
    (
        "app/src/chrome/connection.rs",
        include_str!("../src/chrome/connection.rs"),
    ),
    ("app/src/chrome.rs", include_str!("../src/chrome.rs")),
    (
        "app/src/header/integrated.rs",
        include_str!("../src/header/integrated.rs"),
    ),
    ("app/src/pane_view.rs", include_str!("../src/pane_view.rs")),
    (
        "app/src/visibility.rs",
        include_str!("../src/visibility.rs"),
    ),
    (
        "app/src/settings/editor/probe.rs",
        include_str!("../src/settings/editor/probe.rs"),
    ),
    (
        "app/src/settings/editor/view/tests.rs",
        include_str!("../src/settings/editor/view/tests.rs"),
    ),
    (
        "app/src/settings/editor/view/colors.rs",
        include_str!("../src/settings/editor/view/colors.rs"),
    ),
    (
        "app/src/settings/editor/view/appearance.rs",
        include_str!("../src/settings/editor/view/appearance.rs"),
    ),
    (
        "app/src/tests/settings_chrome.rs",
        include_str!("../src/tests/settings_chrome.rs"),
    ),
    (
        "app/src/tab_bar/chrome.rs",
        include_str!("../src/tab_bar/chrome.rs"),
    ),
    (
        "app/src/panel/cache.rs",
        include_str!("../src/panel/cache.rs"),
    ),
    (
        "app/src/settings/editor/runtime.rs",
        include_str!("../src/settings/editor/runtime.rs"),
    ),
    (
        "app/src/fonts/selection.rs",
        include_str!("../src/fonts/selection.rs"),
    ),
    (
        "app/src/settings/editor/script.rs",
        include_str!("../src/settings/editor/script.rs"),
    ),
    (
        "app/src/tests/settings_editor.rs",
        include_str!("../src/tests/settings_editor.rs"),
    ),
    (
        "app/src/fonts/startup.rs",
        include_str!("../src/fonts/startup.rs"),
    ),
    (
        "app/src/settings/editor.rs",
        include_str!("../src/settings/editor.rs"),
    ),
    (
        "app/src/settings/editor/view.rs",
        include_str!("../src/settings/editor/view.rs"),
    ),
    (
        "app/src/settings/editor/timing.rs",
        include_str!("../src/settings/editor/timing.rs"),
    ),
    (
        "app/src/settings/document.rs",
        include_str!("../src/settings/document.rs"),
    ),
    (
        "app/src/settings/store.rs",
        include_str!("../src/settings/store.rs"),
    ),
    ("app/src/lib.rs", include_str!("../src/lib.rs")),
    ("app/src/startup.rs", include_str!("../src/startup.rs")),
    (
        "app/src/restoration.rs",
        include_str!("../src/restoration.rs"),
    ),
    (
        "app/src/session/ownership.rs",
        include_str!("../src/session/ownership.rs"),
    ),
    ("app/src/alarms.rs", include_str!("../src/alarms.rs")),
    ("app/src/control.rs", include_str!("../src/control.rs")),
    (
        "app/src/control/retiring.rs",
        include_str!("../src/control/retiring.rs"),
    ),
    (
        "app/src/panel/driving.rs",
        include_str!("../src/panel/driving.rs"),
    ),
    (
        "app/src/panel/pane.rs",
        include_str!("../src/panel/pane.rs"),
    ),
    ("app/src/header.rs", include_str!("../src/header.rs")),
    (
        "app/src/pointer/header.rs",
        include_str!("../src/pointer/header.rs"),
    ),
    (
        "app/src/header/actions.rs",
        include_str!("../src/header/actions.rs"),
    ),
    (
        "app/src/header/controls.rs",
        include_str!("../src/header/controls.rs"),
    ),
    (
        "app/src/header/menu.rs",
        include_str!("../src/header/menu.rs"),
    ),
    (
        "app/src/header/splitting.rs",
        include_str!("../src/header/splitting.rs"),
    ),
    (
        "app/src/tests/header_controls.rs",
        include_str!("../src/tests/header_controls.rs"),
    ),
    ("app/src/injected.rs", include_str!("../src/injected.rs")),
    ("app/src/panel.rs", include_str!("../src/panel.rs")),
    (
        "app/src/panel/document.rs",
        include_str!("../src/panel/document.rs"),
    ),
    (
        "app/src/panel/buttons.rs",
        include_str!("../src/panel/buttons.rs"),
    ),
    ("app/src/measure.rs", include_str!("../src/measure.rs")),
    (
        "app/src/measure/header.rs",
        include_str!("../src/measure/header.rs"),
    ),
    ("app/src/session.rs", include_str!("../src/session.rs")),
    ("app/src/fonts.rs", include_str!("../src/fonts.rs")),
    (
        "app/src/fonts/atlas.rs",
        include_str!("../src/fonts/atlas.rs"),
    ),
    ("app/src/settings.rs", include_str!("../src/settings.rs")),
    ("app/src/keyboard.rs", include_str!("../src/keyboard.rs")),
    ("app/src/painter.rs", include_str!("../src/painter.rs")),
    (
        "app/src/painter/notices.rs",
        include_str!("../src/painter/notices.rs"),
    ),
    (
        "app/src/painter/blocks.rs",
        include_str!("../src/painter/blocks.rs"),
    ),
    (
        "app/src/painter/emoji.rs",
        include_str!("../src/painter/emoji.rs"),
    ),
    (
        "app/src/pty_output.rs",
        include_str!("../src/pty_output.rs"),
    ),
    ("app/src/watcher.rs", include_str!("../src/watcher.rs")),
    ("app/src/theme.rs", include_str!("../src/theme.rs")),
    (
        "app/src/theme/catalog.rs",
        include_str!("../src/theme/catalog.rs"),
    ),
    ("app/src/mouse.rs", include_str!("../src/mouse.rs")),
    ("app/src/opening.rs", include_str!("../src/opening.rs")),
    ("app/src/pointer.rs", include_str!("../src/pointer.rs")),
    ("app/src/report.rs", include_str!("../src/report.rs")),
    ("app/src/ligatures.rs", include_str!("../src/ligatures.rs")),
    (
        "app/src/fonts/ligatures.rs",
        include_str!("../src/fonts/ligatures.rs"),
    ),
    (
        "app/src/painter/ligatures.rs",
        include_str!("../src/painter/ligatures.rs"),
    ),
    (
        "app/src/painter/ligatures/tests.rs",
        include_str!("../src/painter/ligatures/tests.rs"),
    ),
    (
        "app/src/painter/pane.rs",
        include_str!("../src/painter/pane.rs"),
    ),
    (
        "app/src/ligature_tests.rs",
        include_str!("../src/ligature_tests.rs"),
    ),
    ("app/src/search.rs", include_str!("../src/search.rs")),
    ("app/src/tab_bar.rs", include_str!("../src/tab_bar.rs")),
    ("app/src/tiling.rs", include_str!("../src/tiling.rs")),
    ("app/src/tree.rs", include_str!("../src/tree.rs")),
    ("app/src/main.rs", include_str!("../src/main.rs")),
    ("app/src/closing.rs", include_str!("../src/closing.rs")),
    (
        "app/src/tests/markdown.rs",
        include_str!("../src/tests/markdown.rs"),
    ),
    ("app/src/links.rs", include_str!("../src/links.rs")),
    (
        "app/src/links/console.rs",
        include_str!("../src/links/console.rs"),
    ),
    (
        "app/src/injected/links.rs",
        include_str!("../src/injected/links.rs"),
    ),
    (
        "app/src/injected/search.rs",
        include_str!("../src/injected/search.rs"),
    ),
    (
        "app/src/panel/color.rs",
        include_str!("../src/panel/color.rs"),
    ),
    (
        "app/src/panel/links.rs",
        include_str!("../src/panel/links.rs"),
    ),
    (
        "app/src/pointer/links.rs",
        include_str!("../src/pointer/links.rs"),
    ),
    (
        "term-core/src/nonblocking.rs",
        include_str!("../../term-core/src/nonblocking.rs"),
    ),
    (
        "term-core/examples/closing.rs",
        include_str!("../../term-core/examples/closing.rs"),
    ),
    (
        "term-core/examples/termios_probe.rs",
        include_str!("../../term-core/examples/termios_probe.rs"),
    ),
    (
        "term-core/examples/writer_leak.rs",
        include_str!("../../term-core/examples/writer_leak.rs"),
    ),
    (
        "term-core/examples/writer_wake.rs",
        include_str!("../../term-core/examples/writer_wake.rs"),
    ),
    (
        "term-core/src/lib.rs",
        include_str!("../../term-core/src/lib.rs"),
    ),
    (
        "term-core/src/screen.rs",
        include_str!("../../term-core/src/screen.rs"),
    ),
    (
        "term-core/src/screen/title.rs",
        include_str!("../../term-core/src/screen/title.rs"),
    ),
    (
        "term-core/src/view.rs",
        include_str!("../../term-core/src/view.rs"),
    ),
    (
        "term-core/src/search.rs",
        include_str!("../../term-core/src/search.rs"),
    ),
    (
        "term-core/examples/search.rs",
        include_str!("../../term-core/examples/search.rs"),
    ),
    (
        "term-core/src/received.rs",
        include_str!("../../term-core/src/received.rs"),
    ),
    (
        "term-core/examples/hang.rs",
        include_str!("../../term-core/examples/hang.rs"),
    ),
    (
        "term-core/examples/delivery.rs",
        include_str!("../../term-core/examples/delivery.rs"),
    ),
    (
        "term-core/examples/grid.rs",
        include_str!("../../term-core/examples/grid.rs"),
    ),
    (
        "term-core/examples/scrollback.rs",
        include_str!("../../term-core/examples/scrollback.rs"),
    ),
    (
        "term-core/examples/shell.rs",
        include_str!("../../term-core/examples/shell.rs"),
    ),
    (
        "term-core/examples/throughput.rs",
        include_str!("../../term-core/examples/throughput.rs"),
    ),
    (
        "app/examples/fonts.rs",
        include_str!("../examples/fonts.rs"),
    ),
    ("app/examples/leak.rs", include_str!("../examples/leak.rs")),
    (
        "app/examples/screen_demo.rs",
        include_str!("../examples/screen_demo.rs"),
    ),
    (
        "app/examples/sweep.rs",
        include_str!("../examples/sweep.rs"),
    ),
    ("app/tests/batches.rs", include_str!("batches.rs")),
    ("app/tests/decisions.rs", include_str!("decisions.rs")),
    ("app/tests/glossary.rs", include_str!("glossary.rs")),
    ("app/tests/prose.rs", include_str!("prose.rs")),
    ("app/tests/shape.rs", include_str!("shape.rs")),
    ("app/tests/vendor_patch.rs", include_str!("vendor_patch.rs")),
    ("app/tests/waits.rs", include_str!("waits.rs")),
    ("app/tests/common/mod.rs", include_str!("common/mod.rs")),
    (
        "term-core/tests/api_without_deps.rs",
        include_str!("../../term-core/tests/api_without_deps.rs"),
    ),
    (
        "term-core/tests/idle_console_cpu.rs",
        include_str!("../../term-core/tests/idle_console_cpu.rs"),
    ),
];

/// The one file of the walked directories [`MODULES`] leaves out, with its reason: this file, whose
/// fixtures carry the defect on purpose so the rule has something to report.
const NOT_JUDGED: &str = "app/tests/scripts.rs";

/// The files [`MODULES`] has to keep naming.
///
/// **The list is checked against the tree, and the count stays beside it as a floor.** A count
/// raised by hand binds only the delivery that remembers to raise it: with one, this list still fell
/// twelve files behind, and one of those twelve printed its ready marker after a `stty` it did not
/// wait for. The tree is every Rust file of `app`, `term-core` and `panel-control` — their sources, examples and
/// tests — minus [`NOT_JUDGED`]; the remaining crates run no shell.
#[test]
fn the_list_still_names_every_module() {
    let declared: Vec<&str> = MODULES.iter().map(|(name, _)| *name).collect();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut on_disk = rust_sources(
        &crates,
        &[
            "app/src",
            "app/examples",
            "app/tests",
            "term-core/src",
            "term-core/examples",
            "term-core/tests",
            "panel-control/src",
            "panel-control/tests",
        ],
    );
    assert!(
        on_disk.remove(NOT_JUDGED),
        "{NOT_JUDGED} is left out by name and is no longer on disk"
    );
    if let Err(failure) = list_against_disk("MODULES", &declared, &on_disk) {
        panic!("{failure}. A module with no row is one whose scripts nobody reads");
    }
    assert!(
        MODULES.len() >= 70,
        "`MODULES` names {} files and the last delivery left seventy: a piece came out of a module \
         and its new home was not added, so whatever this guard reports, it is not reporting on \
         every script these crates run",
        MODULES.len()
    );
}

/// The commands that put the terminal into a mode.
///
/// A closed list of one, and it is honest about being one: `stty` is the only way these crates ask
/// for a mode, and F2.7 (macOS and Windows) is the milestone that will have to add to it or replace
/// it outright. A list is still the right shape — the alternative is the word buried in a matcher,
/// which is where the rule was before this file existed.
const MODE_COMMANDS: &[&str] = &["stty"];

/// The operands that make a [`MODE_COMMANDS`] invocation an **enquiry** rather than a setting.
///
/// This is the distinction the whole guard turns on. `stty size` asks the kernel what the grid is
/// and its answer *is* what the test reads, so a `;` after it swallows nothing — the two loops that
/// use it time out red on their own. `stty -echo -icanon` sets something nobody can see afterwards,
/// and that is the shape that lies.
///
/// `-a` and `-g` earn their place here despite the leading dash: they print the settings, they do
/// not change them.
const MODE_ENQUIRIES: &[&str] = &["size", "-a", "-g", "--all", "--save"];

/// The commands that put a marker on screen for a test to wait for.
const PRINTING_COMMANDS: &[&str] = &["echo", "printf"];

/// How many string literals naming a mode command the walk has to still find.
///
/// **Without a floor this file passes by finding nothing**, which is how the guards of this suite
/// die: somebody reshapes the literal scanner, it matches no scripts, every one of them looks clean
/// and nothing says otherwise.
///
/// **The number is the count, with no margin**, and that is deliberate: a floor one below what
/// exists lets exactly one script vanish in silence, which is the project's own second disguise —
/// picking a number where the defect and the fix give the same result. A script that genuinely
/// leaves has to come here and say so.
///
/// **Measured honestly, this floor and [`MARKERS_AT_LEAST`] are dominated by [`JUDGED_AT_LEAST`]:**
/// set either to zero and the suite still goes red, because a walk that finds nothing also judges
/// nothing. They are kept for the error message — «the scanner stopped matching» and «the wait
/// helpers were reshaped» name the cause, where a bare judged-count does not — and not because
/// either one catches something the third does not.
const SCRIPTS_AT_LEAST: usize = 10;

/// How many of those the rule has to actually be judging.
///
/// The one that has teeth. [`SCRIPTS_AT_LEAST`] only proves the scanner still finds text; this
/// proves the *classifier* still recognises the scripts the defect lived in — the two of #158 and
/// the three of #50. Deleting a ready marker, renaming a wait helper or narrowing the mode
/// detection all drop this count and all fail here; each of them would otherwise leave a script
/// unjudged in silence.
const JUDGED_AT_LEAST: usize = 5;

/// How many distinct texts the wait helpers are called with.
///
/// The marker list is harvested from the source rather than written down, so it too can silently
/// become empty — and an empty list means no script prints a marker and every one of them passes.
/// No margin, for the reason [`SCRIPTS_AT_LEAST`] gives.
const MARKERS_AT_LEAST: usize = 11;

/// How many lines of [`MODULES`] name a mode command at all — comments included.
///
/// The floor of [`no_line_naming_a_mode_command_escapes_the_scanner`], which without one asserts
/// that a list is empty and is satisfied by a walk that examined nothing. Comments count on purpose:
/// what this number measures is **reach**, not scripts.
const LINES_NAMING_A_MODE_COMMAND_AT_LEAST: usize = 28;

/// Every double-quoted string literal of a line, as it is spelled in the source.
///
/// **Comment lines are skipped** so that a guard cannot fail on the prose explaining the very defect
/// it hunts: this tree quotes broken scripts in comments constantly, and one written inside double
/// quotes would be read as code. **No comment in [`MODULES`] does that today**, so deleting the skip
/// changes nothing measurable — it is insurance, and saying otherwise would be inventing a scenario.
/// A trailing comment on a code line is **not** skipped, which is the same hole from the other side
/// and is left declared rather than papered over.
///
/// Nothing is unescaped. A shell separator is never written `\;` here, so the raw text splits the
/// same way and the report quotes what somebody will search the file for.
///
/// **Two shapes it does not see**, and neither is silent: a literal continued across lines, and a
/// raw string whose body contains a quote of its own. Both leave a line that names a mode command
/// with nothing extracted from it, which is what
/// [`no_line_naming_a_mode_command_escapes_the_scanner`] exists to catch.
fn literals(line: &str) -> Vec<&str> {
    if line.trim_start().starts_with("//") {
        return Vec::new();
    }
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'"' {
            // A backslash takes the next byte with it, so `\"` does not end the literal. Continuation
            // bytes of a multibyte character are all >= 0x80, so they never look like a delimiter.
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        // An unterminated literal —the line ends inside it— yields nothing: a Rust string does not
        // span lines here, so what is left is a quote inside prose rather than a script.
        if j < bytes.len()
            && let Some(text) = line.get(start..j)
        {
            out.push(text);
        }
        i = j + 1;
    }
    out
}

/// A shell script cut into `(the separator that leads to it, the command)`, the first command having
/// no separator before it.
///
/// Single quotes are tracked because the escape sequences these scripts print live inside them; a
/// `;` in there is data, not a separator. A double quote can only reach here escaped —`\"`, which
/// [`literals`] steps over— and an escaped quote inside a shell script is data too, so nothing on
/// that side is tracked either.
///
/// **A line break is a separator like any other**, and it arrives spelled `\n` because the literal
/// is never unescaped. A script written across lines would otherwise be one single command, with the
/// marker inside it and nothing to judge.
fn commands(script: &str) -> Vec<(Option<&str>, &str)> {
    let mut out = Vec::new();
    let (mut quoted, mut start, mut sep, mut i) = (false, 0usize, None, 0usize);
    // **Advanced a character at a time and not a byte at a time.** Every separator is ASCII, so
    // walking bytes finds the same ones — and then lands `i` inside a multibyte character, where
    // slicing the script panics and the guard stops being a test at all. This tree writes `«»` and
    // `—` into its literals routinely, so the first script that carries one would abort the binary.
    while i < script.len() {
        let rest = &script[i..];
        let width = rest.chars().next().map_or(1, char::len_utf8);
        if rest.starts_with('\'') {
            quoted = !quoted;
            i += width;
            continue;
        }
        let found = if quoted {
            None
        } else {
            ["&&", "||", "\\n", ";", "|", "&", "\n"]
                .into_iter()
                .find(|separator| rest.starts_with(separator))
        };
        match found {
            Some(f) => {
                out.push((sep, script[start..i].trim()));
                sep = Some(f);
                i += f.len();
                start = i;
            }
            None => i += width,
        }
    }
    out.push((sep, script[start..].trim()));
    out
}

/// Whether this command **sets** a terminal mode, as opposed to asking the terminal something.
///
/// The command word is looked for anywhere in the piece and not only at its head, because a loop
/// body arrives as `do stty size` — the shell keyword comes first and the command after it.
fn sets_a_mode(command: &str) -> bool {
    let mut words = command.split_whitespace();
    while let Some(word) = words.next() {
        if MODE_COMMANDS.contains(&word) {
            // A bare `stty` prints the settings, so an invocation with no operand asks rather than
            // sets and the `any` over an empty iterator says exactly that.
            return words.any(|operand| !MODE_ENQUIRIES.contains(&operand));
        }
    }
    false
}

/// Whether this command puts a text on screen that some test is waiting for.
///
/// **The printing word is looked for anywhere in the piece**, symmetrically with [`sets_a_mode`] and
/// for the same reason: a loop body arrives as `do echo ready` and a nested shell as
/// `sh -c 'echo ready'`. Looking only at the head word made both of those unjudged in silence, which
/// is the defect this file exists to stop.
///
/// A piece that names a mode command is never counted as printing: `stty echo` turns the echo on, it
/// does not write «echo» anywhere, and counting it would put a marker where there is none.
///
/// **A format placeholder counts as a marker**, and it has to: `term-core` writes its scripts as
/// `format!("stty raw -echo && printf {MARKER} && …")`, so the text a test waits for reaches the
/// script through a constant and never appears as a literal for [`markers`] to harvest. Without
/// this clause the crate where this whole lesson was learned would sit in [`MODULES`] and be judged
/// on nothing.
fn prints_a_marker(command: &str, markers: &BTreeSet<&str>) -> bool {
    !sets_a_mode(command)
        && command
            .split_whitespace()
            .any(|word| PRINTING_COMMANDS.contains(&word))
        && (command.contains('{') || markers.iter().any(|marker| command.contains(marker)))
}

/// The texts the bench's wait helpers are told to wait for, harvested from the source.
///
/// **Harvested and not listed**, because a list would be the snapshot this guard is supposed to
/// avoid: the marker is whatever the test asks for, and the day somebody spells it «started» instead
/// of «ready» the rule has to follow without being edited.
///
/// The helpers are recognised by the suffix their three names share — `wait_for_the_screen_to_say`,
/// `wait_for_the_pane_to_say`, `wait_for_this_console_to_say`. Format strings are dropped: a
/// `"{} {}"` is a shape, not a text, and no script prints it. What is deliberately **not** harvested
/// is the handful of waits written out by hand as a `contains` loop; they are waits over scripts
/// that set no mode, so the rule would not reach them anyway.
fn markers(source: &str) -> BTreeSet<&str> {
    source
        .lines()
        .filter(|line| line.contains("_to_say("))
        .flat_map(literals)
        .filter(|text| !text.is_empty() && !text.contains('{'))
        .collect()
}

/// One command that runs after a mode command without depending on it, on the way to a marker.
struct Loose<'a> {
    mode: &'a str,
    separator: &'a str,
    command: &'a str,
    marker: &'a str,
}

/// A script the rule has something to say about: it sets a mode and then prints a waited-for marker.
struct Judged<'a> {
    loose: Vec<Loose<'a>>,
}

impl Judged<'_> {
    /// Whether the rule reports this script.
    ///
    /// **A method and not `!judged.loose.is_empty()` at each call site.** `Some(Judged)` means «the
    /// rule reached this script», not «this script is broken», and a caller that spells the second
    /// out for itself is one edit away from testing a copy of the rule instead of the rule.
    fn reports(&self) -> bool {
        !self.loose.is_empty()
    }
}

/// The rule, in **one** place.
///
/// `None` for a script the rule does not reach: it sets no mode, or nothing after a mode prints a
/// text the bench waits for. Nothing to lie about in either case.
///
/// **Every mode command is looked at, not just the last one.** With only the last,
/// `stty -echo; echo ready; stty -icanon; cat -v` has the defect and comes back `None`: no marker
/// follows the mode nearest the end, so a script that lies would go unjudged in silence.
///
/// **One function and not a copy per caller**, and that is not tidiness. The fixtures below are this
/// guard's own mutation check, and a fixture list that re-implements the rule checks a copy nobody
/// runs: measured, with the comparison inside the walk neutered **and** the two scripts put back the
/// way #158 found them, all three tests of this file stayed green. It is the project's first
/// recorded disguise — asserting on the place a value is recorded instead of on the effect it has —
/// wearing the shape of a helper that looked like a convenience.
fn judge<'a>(script: &'a str, markers: &BTreeSet<&str>) -> Option<Judged<'a>> {
    let pieces = commands(script);
    let mut reached = false;
    let mut loose = Vec::new();
    for (at, _) in pieces
        .iter()
        .enumerate()
        .filter(|(_, (_, command))| sets_a_mode(command))
    {
        // **The LAST printing command after the mode, not the first.** One of the texts the bench
        // waits for is the single letter `m` —the tail of a mouse release report— and a marker is
        // matched as a substring, so almost any `printf` counts as one. Stopping at the first would
        // cut the span short and leave the `;` that actually gates the ready marker outside it:
        // measured, `stty -echo -icanon && printf 'mouse mode on'; echo ready` came back clean.
        // Taking the last can only widen the span, and everything a test waits for after a mode
        // command ought to depend on that mode anyway.
        let Some(marker) = pieces[at + 1..]
            .iter()
            .rposition(|(_, command)| prints_a_marker(command, markers))
            .map(|found| found + at + 1)
        else {
            continue;
        };
        reached = true;
        for (separator, command) in &pieces[at + 1..=marker] {
            if *separator != Some("&&") {
                loose.push(Loose {
                    mode: pieces[at].1,
                    separator: separator.unwrap_or_default(),
                    command,
                    marker: pieces[marker].1,
                });
            }
        }
    }
    reached.then_some(Judged { loose })
}

/// Every literal of the crates that names a mode command, with where it is.
fn scripts() -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (module, source) in MODULES {
        for (n, line) in source.lines().enumerate() {
            for text in literals(line) {
                if MODE_COMMANDS
                    .iter()
                    .any(|command| text.split_whitespace().any(|word| word == *command))
                {
                    out.push((format!("{module}:{}", n + 1), text));
                }
            }
        }
    }
    out
}

/// Every marker of every module, which is what the rule is read against.
fn every_marker() -> BTreeSet<&'static str> {
    MODULES
        .iter()
        .flat_map(|(_, source)| markers(source))
        .collect()
}

#[test]
fn no_script_can_print_its_marker_when_the_mode_command_failed() {
    let markers = every_marker();
    assert!(
        markers.len() >= MARKERS_AT_LEAST,
        "the walk harvested {} texts the bench waits for and this delivery left {MARKERS_AT_LEAST}: \
         the wait helpers were renamed or reshaped, so no script counts as printing a marker and \
         every one of them passes this guard by default",
        markers.len()
    );

    let scripts = scripts();
    assert!(
        scripts.len() >= SCRIPTS_AT_LEAST,
        "the walk found {} literals naming a mode command and this delivery left {SCRIPTS_AT_LEAST}: \
         the literal scanner stopped matching, and this guard is judging nothing",
        scripts.len()
    );

    let mut judged = 0usize;
    let mut loose = Vec::new();
    for (place, script) in &scripts {
        let Some(verdict) = judge(script, &markers) else {
            continue;
        };
        judged += 1;
        for one in &verdict.loose {
            loose.push(format!(
                "  {place}: «{}» is reached by `{}` after «{}», so the marker «{}» is printed \
                 whatever the mode command did",
                one.command, one.separator, one.mode, one.marker
            ));
        }
    }

    // The floor that has teeth, and the one a reverted fix has to walk past: it is not enough that
    // the scanner found text — the classifier has to still be recognising the scripts whose green
    // depended on a mode nobody checked.
    assert!(
        judged >= JUDGED_AT_LEAST,
        "this guard judged {judged} script(s) and there are {JUDGED_AT_LEAST} that set a mode and \
         then print a marker: either one lost its marker —in which case the test waiting for it now \
         hangs— or the classifier stopped recognising it, and an unjudged script is exactly what \
         #158 was"
    );
    assert!(
        loose.is_empty(),
        "{} command(s) run after a mode command without depending on it. `;` runs the next command \
         whatever the last one did, so the ready marker arrives with the mode never set — and the \
         line discipline's own echo then forges the text the test looks for, character for \
         character. Measured on #158: fourteen of fifteen tests stayed green over a terminal in \
         canonical mode. Chain with `&&` from the mode command to the marker:\n{}",
        loose.len(),
        loose.join("\n")
    );
}

#[test]
fn no_line_naming_a_mode_command_escapes_the_scanner() {
    // **The shapes [`literals`] cannot read, caught by their footprint instead of by parsing them.**
    // A literal continued across lines and a raw string carrying a quote of its own both leave the
    // walk with nothing extracted — and a script the scanner never sees is indistinguishable from a
    // clean one, which is the failure this whole file exists to make impossible.
    let mut invisible = Vec::new();
    let mut examined = 0usize;
    for (module, source) in MODULES {
        for (n, line) in source.lines().enumerate() {
            let names_one = MODE_COMMANDS.iter().any(|command| line.contains(*command));
            if !names_one {
                continue;
            }
            examined += 1;
            if !literals(line).is_empty() {
                continue;
            }
            if !line.trim_start().starts_with("//") {
                invisible.push(format!("  {module}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    // **Its floor, and it needs one as much as its three siblings do.** This test asserts that a
    // list is empty, and an empty list is what a walk that looked at nothing produces: measured,
    // forcing the mode-command match to `false` left it green. The count is what says the walk was
    // reachable at all.
    assert!(
        examined >= LINES_NAMING_A_MODE_COMMAND_AT_LEAST,
        "the walk looked at {examined} line(s) naming a mode command and this delivery left \
         {LINES_NAMING_A_MODE_COMMAND_AT_LEAST}: it is examining less of the tree than it was, so \
         an unreadable script would go past it unseen"
    );
    assert!(
        invisible.is_empty(),
        "{} line(s) name a mode command and the scanner extracted no literal from them, so whatever \
         script is in there is judged by nothing. Write it as a plain double-quoted literal on one \
         line, or teach `literals` the shape:\n{}",
        invisible.len(),
        invisible.join("\n")
    );
}

#[test]
fn the_semicolon_that_is_loop_syntax_is_still_let_through() {
    // **What separates this guard from «no `;` in a shell literal»**, which is the rule it would be
    // if nobody asserted the difference. `while :; do stty size; sleep 0.05; done` names the same
    // command, carries the same character and is safe: it asks the terminal something instead of
    // setting it, and the answer is what the test reads — a failing `stty` there times out red.
    //
    // Without this test the rule could quietly widen to the whole character and nothing would say
    // so until it fired on somebody else's correct code.
    let markers = every_marker();
    let let_through: Vec<&str> = scripts()
        .into_iter()
        .filter(|(_, script)| {
            let pieces = commands(script);
            pieces.iter().any(|(separator, _)| *separator == Some(";"))
                && !pieces.iter().any(|(_, command)| sets_a_mode(command))
        })
        .map(|(_, script)| script)
        .collect();
    assert!(
        !let_through.is_empty(),
        "not one script naming a mode command carries a `;` this guard accepts: either the two \
         `stty size` loops left the tree —say so and delete this test— or the rule widened into a \
         ban on the character and will now fire on correct code"
    );
    // The loop below is **vacuous today** and it is kept on purpose: neither `stty size` loop
    // contains an `echo` or a `printf`, so nothing it could catch exists yet. It is the assertion
    // above that has teeth. What this one is here for is the day somebody adds a `;` script that
    // does print — then the enquiry-versus-setting distinction stops being a technicality and
    // starts being the thing that decides whether a real defect is reported.
    for script in &let_through {
        assert!(
            !commands(script)
                .iter()
                .any(|(_, command)| prints_a_marker(command, &markers)),
            "«{script}» is being let through and it does print a marker the bench waits for: the \
             enquiry-versus-setting distinction is excusing a script that has the defect"
        );
    }
}

/// The rule fires on the defect and stays quiet on the shapes that were measured safe.
///
/// **This is the mutation, run as a test.** The walk above reads a tree that is already correct, so
/// on its own it cannot tell «the scripts are chained strictly» from «the rule stopped recognising
/// anything» — the failure mode this project has hit more than a dozen times with the suite green.
/// The fixtures are inputs to the rule rather than a copy of the tree: the walk never compares
/// against them, so a script that legitimately changes its spelling does not come here to be edited.
#[test]
fn the_rule_reports_the_defect_and_nothing_else() {
    let markers = every_marker();
    // `true` for a rule that reports the script, `false` for one that lets it through.
    let fixtures: &[(bool, &str)] = &[
        // The two of #158, as the ticket found them.
        (
            true,
            "stty -echo -icanon; printf '\\033[?1000h' ; echo ready; cat -v",
        ),
        (
            true,
            "stty -echo -icanon; printf '\\033[?1049h'; echo ready",
        ),
        // The same defect written with the separator that swallows the opposite outcome.
        (true, "stty -echo -icanon || echo ready"),
        // And written across two lines, where the break is the separator.
        (true, "stty -echo -icanon\\necho ready"),
        // The defect behind a second mode command, which a rule looking only at the last one misses.
        (true, "stty -echo; echo ready; stty -icanon; cat -v"),
        // The repaired form, and `term-core`'s shape where the marker arrives as a constant.
        (
            false,
            "stty -echo -icanon && printf '\\033[?1049h' && echo ready",
        ),
        (false, "stty raw -echo && printf {MARKER} && sleep 6"),
        (true, "stty raw -echo; printf {MARKER}; sleep 6"),
        // The four the ticket asked to check, each measured safe in the diagnosis.
        (false, "while :; do stty size; sleep 0.05; done"),
        (
            false,
            "head -c 400000 /dev/zero | tr '\\0' a; printf '\\r\\nEND'",
        ),
        (false, "printf 'JUST WRITTEN\\n'; sleep 30"),
        (false, "while :; do stty size; sleep 0.05; done"),
        // A mode is set and nothing waits for what follows: nothing to lie about.
        (false, "stty -echo -icanon; cat -v"),
        // No mode command at all.
        (false, "sleep 30"),
        // **The three below cover the rule's own halves**, each found by mutating it: without one
        // of them, deleting the matching clause of `prints_a_marker` or narrowing the span in
        // `judge` leaves this test green.
        //
        // It prints, and what it prints is nobody's marker — so the marker match is doing work.
        (false, "stty -echo -icanon; echo nobody-waits-for-this"),
        // It carries a marker and prints nothing — so the printing-word check is doing work.
        (false, "stty -echo -icanon; cat -v ready"),
        // The loose separator is **mid-chain**, not immediately before the marker — so the span
        // really runs from the mode command to the marker and not just over the last step.
        (true, "stty -echo -icanon; printf x && echo ready"),
        // **The marker is the LAST one, not the first.** One of the harvested markers is the single
        // letter `m`, so `printf 'mouse mode on'` matches one too; stopping there would end the span
        // before the `;` that actually gates «ready». The comment on `judge` names this script as the
        // reason, and until this fixture existed nothing held the claim.
        (
            true,
            "stty -echo -icanon && printf 'mouse mode on'; echo ready",
        ),
        // The separators that are neither `;` nor `&&`: a background `&` runs the marker without
        // waiting for the mode command at all.
        (true, "stty -echo -icanon & echo ready"),
        // An enquiry operand in front of a setting one still makes it a setting, so the whole operand
        // list has to be read and not just its head.
        (true, "stty --save -icanon; echo ready"),
    ];
    for (should_report, script) in fixtures {
        // **Through `judge` and `reports`, which are what the walk above calls.** Reading the rule a
        // second time here is what would turn this test into a decoration.
        let reported = judge(script, &markers).is_some_and(|verdict| verdict.reports());
        assert_eq!(
            reported,
            *should_report,
            "the rule {} «{script}», and it should {}",
            if reported { "reports" } else { "lets through" },
            if *should_report {
                "report it"
            } else {
                "let it through"
            }
        );
    }
}
