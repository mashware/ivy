//! **A guard over text, because behaviour is out of reach here** — issue #203, D33's patched
//! `egui-winit`.
//!
//! # What the bug is, and why nothing in this crate's suite can drive it
//!
//! `Ctrl+V` over a clipboard holding no text (an image, or nothing at all) used to reach the
//! program inside a console as **nothing at all** — no byte, no event — because upstream
//! `egui-winit`'s paste-shortcut branch does an unconditional `return` the moment it decides a
//! chord means paste, whether or not it actually produced one. D33 fixes it with a copy of the
//! crate under `vendor/egui-winit` whose first change makes that `return` conditional on having
//! pushed a `Paste`.
//!
//! **The second change (#274)** is one arm of `is_cut_command`, which upstream matches on `command
//! && X` whatever Shift says — so `Ctrl+Shift+X`, this terminal's close chord, became a cut before it
//! was a key and reached the program as `0x18`. The copy excludes Shift there. Every close test in
//! this crate seeds `Key::X` with Ctrl+Shift; before that change the desktop never delivered it.
//!
//! **The third change (D115)** is Windows-only and lives in the three clipboard predicates:
//! `is_cut_command` loses its Shift+Delete arm, `is_copy_command` its Ctrl+Insert arm, and
//! `is_paste_command`'s Shift+Insert arm requires Shift without Ctrl or Alt. Those three functions
//! are cut out of the residual hash and pinned whole, line by line, below.
//!
//! **A synthetic KeyEvent test cannot exercise the adapter behaviourally.** `winit::event::KeyEvent` carries `pub(crate) platform_specific`
//! (`winit-0.30.13/src/event.rs:655`), private to the `winit` crate itself — not to `egui-winit`,
//! which sits one crate away and has no more access to that field than this one does — so no test
//! outside `winit` can construct the event that triggers the bug. D122 adds a Windows
//! native example using real SendInput and a live event loop instead. And
//! `on_window_event` demands a real `&winit::window::Window`, which winit only
//! ever hands out from a live event loop bound to a running compositor. Every existing paste test
//! in this crate builds `egui::Event::Paste` by hand via `enqueue()` and starts **after** the layer
//! that swallows the key — all of them stay green with the bug back in place, which is exactly the
//! disguise D30 names: the scenario the test's world cannot produce.
//!
//! # What this guard is instead
//!
//! **It reads source, not behaviour, and it proves only that the patch is still where it was put —
//! nothing about what pressing the key does.** Four things, each checked against something that
//! was itself verified by hand once — the whole account is in **D33**, which is the only place that
//! survives a clone — and is now locked so a later change has to disturb a number here in order to
//! break it silently:
//!
//! a) the workspace manifest still patches `egui-winit` to the vendored copy;
//! b) the vendored copy's paste branch still gates its `return` on having pushed a paste, instead
//!    of upstream's unconditional one — and its three clipboard predicates hold exactly D33's and
//!    D115's arms;
//! D122 adds a separately fingerprinted Ctrl+Alt dispatcher and pins its Windows call before
//! key and clipboard dispatch. Portable effect tests exercise that dispatcher; the native
//! `windows_altgr.rs` example tests actual OS events, with installed-terminal manual checks separate.
//!
//! c) nothing else in the vendored copy has been *edited* — every file but `src/lib.rs` is locked by
//!    a content hash, and `src/lib.rs` is locked **outside** its patched places, so a change
//!    anywhere else in the crate is caught even though this test never re-reads the crates.io copy;
//! d) and no file has been *added to* or *removed from* it. Hashing a list of known files is silent
//!    about a file that was not on the list, which a review confirmed by adding one and watching all
//!    of (a) to (c) stay green — so the directory is walked and compared against the list, not merely
//!    indexed by it.
//!
//! **Why a hash lock instead of a live `diff` against the local cargo registry cache** — the
//! obvious thing, and the one this guard deliberately does not do. The registry copy lives at
//! `~/.cargo/registry/src/…/egui-winit-0.36.1/`, put there by whoever last needed the *unpatched*
//! crate. With `[patch.crates-io]` in effect cargo has no reason to ever fetch it again — a clean
//! checkout on a fresh machine or a CI runner may simply not have it, and a test that panics on a
//! missing directory there is a test that cannot run in CI, which is worse than a test that proves
//! less. What is checked into this repository is always present, so that is what gets locked.
//!
//! The published source **was** read to build the numbers below, and only there —
//! `diff -rq` against `egui-winit-0.36.1/` confirms that only `src/lib.rs` differs at all (the
//! registry's own bookkeeping files, `.cargo-ok` and friends, aside), and a further, narrower diff —
//! the same file with its patched places cut out of both copies — confirms the rest of `src/lib.rs`
//! is byte-for-byte the published one too. That check is not repeated at test time; its result is
//! what [`LOCKED_FILES`] and [`LIB_RS_RESIDUAL_HASH`] encode.
//!
//! **The hash is FNV-1a, 64 bits, hand-rolled, and that is a deliberate choice, not a shortcut taken
//! for lack of one.** Nobody is trying to defeat this guard on purpose — the only adversary is an
//! accidental edit — so a non-cryptographic hash is exactly as much guarantee as the job needs, and
//! it costs adding no dependency to a crate whose own manifest argues, line by line, about every
//! package that enters the tree.
//!
//! D74 adds the two original license texts and points the manifest at those local files.
//! Their exact bytes and the adjusted manifest are locked too; runtime source remains unchanged.
//!
//! # What defeats it
//!
//! **Reformatting.** Every check here matches literal, `rustfmt`-shaped text — a line reflowed, a
//! comment reworded, a brace moved to its own line. None of that would fail `cargo fmt --check` (the
//! vendored copy is someone else's code and is not asked to pass it), and any of it would make this
//! guard fail even though the patch's *meaning* survived — or, in the length/hash locks, pass by
//! accident if an edit happened to keep the same byte length and the same hash, which for a
//! non-cryptographic hash is merely astronomically unlikely rather than impossible. **A guard that
//! goes red on a harmless reformat is a false positive this file is willing to pay for**: the
//! alternative is a guard loose enough to also miss the patch actually being lost, which is the one
//! failure this file exists to catch.

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above this crate")
}

/// The substring of `source` from the start of `anchor` to the `}` matching the `{` it ends in, both
/// included — so a function's signature comes with its body.
///
/// **Leans on the same assumption `shape.rs` names for its own brace walk: every `{` in the text
/// closes with a `}`**, which is true of Rust source that compiles. `anchor` must itself end in the
/// brace it opens — asserted below rather than trusted, because a typo'd anchor that drops the `{`
/// would otherwise make this function scan to the end of the file and blame the wrong thing.
fn brace_block<'a>(source: &'a str, anchor: &str) -> &'a str {
    let start = source.find(anchor).unwrap_or_else(|| {
        panic!("`{anchor}` was not found: the vendored copy no longer has this shape")
    });
    let open = start + anchor.len() - 1;
    assert_eq!(
        source.as_bytes().get(open),
        Some(&b'{'),
        "`{anchor}` does not end in the brace it is supposed to open"
    );
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.unwrap_or_else(|| panic!("`{anchor}`'s block is never closed"));
    &source[start..end]
}

/// The whole `else if is_paste_command(…) { … }` branch, comments included.
fn paste_branch(lib_rs: &str) -> &str {
    brace_block(lib_rs, "is_paste_command(self.modifiers, active_key) {")
}

/// The whole `fn is_cut_command(…) { … }`, comments included — the copy's second change (D33) and
/// part of its third (D115).
fn cut_command(lib_rs: &str) -> &str {
    brace_block(
        lib_rs,
        "fn is_cut_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
    )
}

/// The whole `fn is_copy_command(…) { … }`, comments included — part of the third change (D115).
fn copy_command(lib_rs: &str) -> &str {
    brace_block(
        lib_rs,
        "fn is_copy_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
    )
}

/// The whole `fn is_paste_command(…) { … }`, comments included — part of the third change (D115).
fn paste_command(lib_rs: &str) -> &str {
    brace_block(
        lib_rs,
        "fn is_paste_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
    )
}

/// A function's code lines, trimmed, comments and blank lines left out.
fn code_lines(function: &str) -> Vec<&str> {
    function
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .collect()
}

/// The first non-blank, trimmed line of `text` after skipping `skip` leading lines.
fn first_line_after(text: &str, skip: usize) -> &str {
    text.lines()
        .skip(skip)
        .map(str::trim)
        .find(|l| !l.is_empty())
        .expect("a block with a statement after its opening line")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// Git may check text out as CRLF. Keep every other byte significant to the drift lock.
fn canonical_source(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .enumerate()
        .filter(|(at, byte)| **byte != b'\r' || bytes.get(at + 1) != Some(&b'\n'))
        .map(|(_, byte)| *byte)
        .collect()
}

fn source_fingerprint(bytes: &[u8]) -> (usize, u64) {
    let bytes = canonical_source(bytes);
    (bytes.len(), fnv1a64(&bytes))
}

fn unpatched_source(source: &str) -> String {
    let source = String::from_utf8(canonical_source(source.as_bytes()))
        .expect("normalizing line endings preserves UTF-8");
    let source = source.replacen("#[cfg(target_os = \"windows\")]\nmod altgr;\n\n", "", 1);
    let dispatch = brace_block(
        &source,
        "        #[cfg(target_os = \"windows\")]\n        {",
    );
    let source = source.replacen(&format!("{dispatch}\n\n"), "", 1);
    let residual = source.replacen(paste_branch(&source), "", 1);
    let residual = residual.replacen(cut_command(&residual), "", 1);
    let residual = residual.replacen(copy_command(&residual), "", 1);
    residual.replacen(paste_command(&residual), "", 1)
}

#[test]
fn the_workspace_still_patches_egui_winit_to_the_vendored_copy() {
    let path = repo().join("Cargo.toml");
    let manifest = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let header = "[patch.crates-io]";
    let mut lines = manifest.lines();
    assert!(
        lines.by_ref().any(|l| l.trim() == header),
        "the workspace manifest no longer has a `{header}` section: D33's whole fix rests on it, \
         and without it cargo resolves `egui-winit` from crates.io again — the unconditional \
         `return` this repository patched around comes straight back, silently"
    );
    let section: Vec<&str> = lines
        .take_while(|l| !l.trim_start().starts_with('['))
        .collect();

    let target = r#"egui-winit = { path = "vendor/egui-winit" }"#;
    assert!(
        section.iter().any(|l| l.trim() == target),
        "`[patch.crates-io]` no longer points `egui-winit` at the vendored copy — found instead:\n\
         {}\nA `path` patch works only while it names the right path; anything else here (a \
         version pin, a git patch, a different directory) leaves the copy under `vendor/` \
         unreferenced and the published crate's swallow bug back in effect",
        section.join("\n")
    );
}

/// The vendored source and approved licensing files, as
/// `(path relative to vendor/egui-winit, FNV-1a-64 of its bytes, its byte length)`.
/// CRLF checkout pairs are read as LF before comparing lengths and hashes.
///
/// **Locked *and* scanned**, which is not what this list looked like at first. A fixed list says
/// nothing about a file that is not on it, so the test that reads these hashes also walks the
/// directory and demands that what is there is exactly this list plus `src/lib.rs` — a review found
/// the gap by adding a file and watching every check stay green.
const LOCKED_FILES: &[(&str, u64, usize)] = &[
    ("Cargo.toml", 0xc3d1058a15a32a77, 10316),
    ("LICENSE-APACHE", 0x519795a3ca7bc6f0, 10850),
    ("LICENSE-MIT", 0x98e169e17925e47e, 1092),
    ("README.md", 0x869047775257e977, 571),
    ("src/altgr.rs", 0xc0b8c9edb44d60f1, 751),
    ("src/clipboard.rs", 0xc134c090beecb79a, 5801),
    ("src/dropped_file.rs", 0x6e83ae380deac032, 425),
    ("src/safe_area.rs", 0x29c8f9ee6133a5ad, 2394),
    ("src/window_settings.rs", 0xb35505c8f99ec593, 7753),
];

/// FNV-1a-64 of `vendor/egui-winit/src/lib.rs` with the paste branch and the three clipboard
/// predicates (`is_cut_command`, `is_copy_command`, `is_paste_command`) cut out.
///
/// The first cut was recorded by diffing its residual against the same cut made in the published
/// crate's own `src/lib.rs` — the two were byte-for-byte identical. Each later cut was added with its
/// change, and its residual computed from the tree **before** that change was made, by a script that
/// first reproduced the old constant exactly — so what it removes is upstream's functions and nothing
/// else. See the module doc comment for why the comparison lives here as a number.
const LIB_RS_RESIDUAL_HASH: u64 = 0xeb37e04499eefa4a;
const LIB_RS_RESIDUAL_LEN: usize = 93_012;

#[test]
fn nothing_else_in_the_vendored_copy_has_drifted_from_the_published_crate() {
    let vendor_root = repo().join("vendor/egui-winit");

    for (rel, expected_hash, expected_len) in LOCKED_FILES {
        let path = vendor_root.join(rel);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let (length, hash) = source_fingerprint(&bytes);
        assert_eq!(
            length, *expected_len,
            "`vendor/egui-winit/{rel}` changed size ({} bytes, recorded {expected_len}): this file \
             must match the recorded egui-winit 0.36.1 plus D74 licensing snapshot; \
             runtime differences remain confined to the two D33 patches in `src/lib.rs`",
            length
        );
        assert_eq!(
            hash, *expected_hash,
            "`vendor/egui-winit/{rel}` has the recorded length but different content: it was edited \
             without this guard's numbers being updated. This is not `src/lib.rs` — nothing here is \
             allowed to drift beyond the published source and recorded D74 licensing changes"
        );
    }

    let lib_rs_path = vendor_root.join("src/lib.rs");
    let lib_rs = fs::read_to_string(&lib_rs_path)
        .unwrap_or_else(|e| panic!("{}: {e}", lib_rs_path.display()));
    let residual = unpatched_source(&lib_rs);

    assert_eq!(
        residual.len(),
        LIB_RS_RESIDUAL_LEN,
        "`src/lib.rs` changed outside its patched places: its length without the \
         `is_paste_command` branch and the three clipboard predicates moved from \
         {LIB_RS_RESIDUAL_LEN} to {}. The \
         published crate's `src/lib.rs`, cut the same way, matches this residual — a change here \
         means the vendored copy differs from crates.io somewhere nobody is watching",
        residual.len()
    );
    assert_eq!(
        fnv1a64(residual.as_bytes()),
        LIB_RS_RESIDUAL_HASH,
        "`src/lib.rs` changed outside its patched places: same length as recorded, different \
         content"
    );

    // **A list of files says nothing about a file that was ADDED**, and without this the guard reads
    // an arrival as cleanliness: hashing six known files finds nothing wrong with a seventh. It was
    // confirmed by adding one and watching all three tests stay green. Today such a file compiles
    // into nothing unless something declares it — but «harmless because nothing references it yet»
    // is the argument that ages worst, and the whole point of this file is that the copy differs
    // from the published crate in two known places and no other.
    let mut present: Vec<String> = Vec::new();
    walk_files(&vendor_root, &vendor_root, &mut present);
    present.sort();
    let mut expected: Vec<String> = LOCKED_FILES
        .iter()
        .map(|(rel, _, _)| (*rel).to_owned())
        .chain(std::iter::once("src/lib.rs".to_owned()))
        .collect();
    expected.sort();
    assert_eq!(
        present, expected,
        "the vendored copy no longer holds exactly the files this guard knows about. Anything \
         listed here and not above was added to somebody else's crate; anything above and not here \
         was deleted from it"
    );
}

#[test]
fn checkout_line_endings_do_not_change_the_vendor_snapshot() {
    let vendor_root = repo().join("vendor/egui-winit");
    for (relative, hash, length) in LOCKED_FILES {
        let source = fs::read_to_string(vendor_root.join(relative)).expect("locked source");
        let lf = source.replace("\r\n", "\n");
        let crlf = lf.replace('\n', "\r\n");
        for source in [&lf, &crlf] {
            assert_eq!(source_fingerprint(source.as_bytes()), (*length, *hash));
        }
    }
    let source = fs::read_to_string(vendor_root.join("src/lib.rs")).expect("vendored lib.rs");
    let lf = source.replace("\r\n", "\n");
    for source in [&lf, &lf.replace('\n', "\r\n")] {
        let residual = unpatched_source(source);
        assert_eq!(
            (residual.len(), fnv1a64(residual.as_bytes())),
            (LIB_RS_RESIDUAL_LEN, LIB_RS_RESIDUAL_HASH),
        );
    }
}

#[test]
fn vendor_fingerprints_still_reject_edits_beyond_crlf_pairs() {
    let original = source_fingerprint(b"let x = 1;\n");
    assert_eq!(original, source_fingerprint(b"let x = 1;\r\n"));
    for edited in [
        &b"let x = 2;\n"[..],
        &b"let x = 1;"[..],
        &b"let x = 1;\r"[..],
        &b"let x\r = 1;\n"[..],
        &b"let x = 1; \n"[..],
    ] {
        assert_ne!(source_fingerprint(edited), original);
    }
}

/// Every file under `root`, as paths relative to `base`, with `/` as the separator.
///
/// **It walks rather than reading a manifest** because the thing being looked for is precisely a
/// file nobody declared. Directories are descended in whatever order the filesystem gives them and
/// the caller sorts, so the comparison does not depend on that order.
fn walk_files(root: &Path, base: &Path, into: &mut Vec<String>) {
    let entries = fs::read_dir(root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, base, into);
        } else {
            let relative = path
                .strip_prefix(base)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn the_vendored_return_is_gated_on_having_actually_pushed_a_paste() {
    let path = repo().join("vendor/egui-winit/src/lib.rs");
    let lib_rs = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let branch = paste_branch(&lib_rs);

    // **Upstream's shape and the patch's differ by where one `return;` sits, and that is the whole
    // fix.** Upstream: the clipboard-read block closes and the very next statement is a bare
    // `return;`, which fires whether or not anything was pushed. The patch: that trailing `return;`
    // is gone, and one sits immediately after the push instead — so a chord that produced nothing
    // falls through and the key event is emitted like any other.
    //
    // Both halves are asserted, because either alone passes over a live bug. Only checking that the
    // push is followed by `return;` would stay green with the trailing one also back — which is
    // upstream's behaviour exactly. Only checking that the trailing one is gone would stay green if
    // the branch stopped returning at all, which swallows nothing but sends the paste **and** the
    // key on every ordinary paste.
    let pushed = "self.egui_input.events.push(egui::Event::Paste(contents));";
    let after_push = branch
        .find(pushed)
        .map(|start| &branch[start + pushed.len()..])
        .unwrap_or_else(|| {
            panic!(
                "the paste branch no longer pushes a `Paste` event at all: this is not the patched \
                 shape and it is not upstream's either"
            )
        });
    let gated_statement = first_line_after(after_push, 0);
    assert_eq!(
        gated_statement, "return;",
        "the statement right after the `Paste` push is `{gated_statement}`, not `return;`: the \
         patch's defining move — returning **only** when a paste was actually produced — is gone"
    );

    let clipboard_block = brace_block(branch, "if let Some(contents) = self.clipboard.get() {");
    let after_clipboard_block = branch
        .find(clipboard_block)
        .map(|start| &branch[start + clipboard_block.len()..])
        .expect("the extracted clipboard block is a literal substring of the branch it came from");
    let trailing = first_line_after(after_clipboard_block, 0);
    assert_ne!(
        trailing, "return;",
        "the clipboard-read block is still followed by a bare `return;` — upstream's own shape, \
         which discards the keypress whenever the clipboard holds no text. That is exactly the bug \
         this copy exists to remove, come back"
    );

    // **The two lines the `return` hangs off, pinned as well.** The drift hash above cuts this whole
    // branch out before hashing — it has to, since this is one of the two places the copy is
    // *supposed* to differ — so nothing else in this file watches what happens inside it. A review pointed out what
    // that lets through on the next rebase: invert the emptiness test and every ordinary paste is
    // swallowed while an empty clipboard pastes nothing at all, with all three tests green. Both
    // lines below are upstream's own, unchanged by the patch, and neither has any business moving.
    for line in [
        "let contents = contents.replace(\"\\r\\n\", \"\\n\");",
        "if !contents.is_empty() {",
    ] {
        assert!(
            branch.lines().any(|l| l.trim() == line),
            "the paste branch no longer contains `{line}`. That line is the published crate's, not \
             this copy's: the patch moves one `return` and changes nothing else inside this branch, \
             so a change here means the copy has drifted where nothing is watching it"
        );
    }
}

#[test]
fn the_vendored_cut_command_leaves_ctrl_shift_x_to_be_a_key() {
    let path = repo().join("vendor/egui-winit/src/lib.rs");
    let lib_rs = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let arms = code_lines(cut_command(&lib_rs));

    // **The second change, and what it is for (#274).** Upstream's `command && X` ignores Shift, so
    // `Ctrl+Shift+X` — this terminal's close chord — became a cut before it was a key, the key was
    // never emitted, and the program received a cut's `0x18` instead. With Shift excluded the chord
    // arrives as `Key::X`, which is what the keymap and every close test in this crate seed.
    assert!(
        arms.contains(&"|| (modifiers.command && !modifiers.shift && keycode == egui::Key::X)"),
        "`is_cut_command`'s Ctrl+X arm no longer excludes Shift: `Ctrl+Shift+X` is a cut again, the \
         close chord never reaches the keymap, and the program in the focused console receives \
         `0x18` instead of the console closing. Found instead:\n{}",
        arms.join("\n")
    );
    // **And exactly that.** This function is cut out of the residual hash, so an arm added here is
    // watched by nobody else. Two are likely on a rebase: upstream's own `command && X` coming back
    // beside the patched arm, which makes `Ctrl+Shift+X` a cut again, and upstream's Windows
    // Shift+Delete arm, which D115 removed because the cut reached the program as `0x18` and
    // `nano` exited.
    let expected = [
        "fn is_cut_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
        "keycode == egui::Key::Cut",
        "|| (modifiers.command && !modifiers.shift && keycode == egui::Key::X)",
        "}",
    ];
    assert_eq!(
        arms, expected,
        "`is_cut_command` holds code beyond its signature and two arms: whatever was added is \
         outside every hash in this file"
    );
}

#[test]
fn the_vendored_copy_command_leaves_ctrl_insert_to_be_a_key() {
    let path = repo().join("vendor/egui-winit/src/lib.rs");
    let lib_rs = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // **D115.** A copy request carries no key, so with upstream's Windows `ctrl && Insert` arm back
    // the application cannot tell Ctrl+Insert from Ctrl+C, and Ctrl+Insert interrupts the program
    // with `0x03` instead of copying. The two arms left are upstream's, untouched.
    let expected = [
        "fn is_copy_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
        "keycode == egui::Key::Copy || (modifiers.command && keycode == egui::Key::C)",
        "}",
    ];
    assert_eq!(
        code_lines(copy_command(&lib_rs)),
        expected,
        "`is_copy_command` is not upstream's minus its Windows Ctrl+Insert arm (D115)"
    );
}

#[test]
fn the_vendored_paste_command_takes_shift_insert_only_with_shift_alone() {
    let path = repo().join("vendor/egui-winit/src/lib.rs");
    let lib_rs = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // **D115.** Upstream's Windows arm reads Shift and ignores the rest, so `Ctrl+Shift+Insert` and
    // `Alt+Shift+Insert` were pastes; with the copy arm gone the first would paste instead of copying.
    // Shift without Ctrl or Alt is the one chord that pastes.
    let expected = [
        "fn is_paste_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {",
        "keycode == egui::Key::Paste",
        "|| (modifiers.command && keycode == egui::Key::V)",
        "|| (cfg!(target_os = \"windows\")",
        "&& modifiers.shift",
        "&& !modifiers.ctrl",
        "&& !modifiers.alt",
        "&& keycode == egui::Key::Insert)",
        "}",
    ];
    assert_eq!(
        code_lines(paste_command(&lib_rs)),
        expected,
        "`is_paste_command` is not upstream's with its Windows Shift+Insert arm narrowed to Shift \
         alone (D115)"
    );
}

fn check_windows_altgr_dispatch(source: &str) {
    let source = String::from_utf8(canonical_source(source.as_bytes())).unwrap();
    let body = brace_block(
        &source,
        "fn on_keyboard_input(&mut self, event: &winit::event::KeyEvent) {",
    );
    let dispatch = brace_block(body, "        #[cfg(target_os = \"windows\")]\n        {");
    assert_eq!(
        code_lines(dispatch),
        [
            "#[cfg(target_os = \"windows\")]",
            "{",
            "use winit::platform::modifier_supplement::KeyEventExtModifierSupplement as _;",
            "if altgr::consume(",
            "event.state == winit::event::ElementState::Pressed,",
            "self.modifiers,",
            "event.text_with_all_modifiers(),",
            "&mut self.egui_input.events,",
            ") {",
            "return;",
            "}",
            "}",
        ]
    );
    assert!(body.find(dispatch).unwrap() < body.find("let winit::event::KeyEvent {").unwrap());
    assert!(source.contains("#[cfg(target_os = \"windows\")]\nmod altgr;"));
}

#[test]
fn windows_altgr_dispatch_precedes_clipboard_and_key_events() {
    let source = fs::read_to_string(repo().join("vendor/egui-winit/src/lib.rs")).unwrap();
    let lf = source.replace("\r\n", "\n");
    for source in [&lf, &lf.replace('\n', "\r\n")] {
        check_windows_altgr_dispatch(source);
    }
}
