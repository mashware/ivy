//! **Every blocking wait in production code is bounded, or says out loud that it is abandoned.**
//!
//! A thread that waits is not looking at the thing that would normally wake it, so every wait needs
//! its own way out. The rule was written down before this guard existed and `term-core` broke it
//! twice anyway: the writer thread, never joined because it could block for ever (#159; since that
//! issue, only off Linux), and the reader
//! thread, which had its stop flag and hung regardless, because it was parked inside a `read` where
//! the flag is never consulted (D32). A third reminder in a comment would not have held, so this
//! breaks the build instead. **D57** carries the decision.
//!
//! # What it judges
//!
//! Every call named `recv`, `wait` or `wait_while`, and every `join` called **with nothing between
//! its parentheses** — a `join` with an argument joins a path or a string, never a thread. It walks
//! the production source of **every crate** under `crates/`, discovered rather than listed: the guard
//! that watched one crate is the mistake this repository has already made once.
//!
//! Such a call passes only when a paragraph of the comment block ending right above it — or a
//! comment trailing on the call's own line — **opens** with one of the two cases, its reason on the
//! same line. Only the first wait on a line can be declared, so a line holding two waits fails on the
//! second until it is split:
//!
//! - «Bounded:» — what ends the wait.
//! - «Abandoned on purpose:» — who is left behind, and why that is the better trade.
//!
//! A deadline form (`recv_timeout`, `wait_timeout`, `wait_timeout_while`, `recv_deadline`) is a
//! different name, so it is never judged.
//!
//! # What it does not see, said out loud
//!
//! - **A lock held across a wait.** A walk over text cannot see how long a guard lives. A condition
//!   variable's `wait` is still caught by its name.
//! - **A wait it cannot name**: a `for` over a receiver or its `iter()` — too common a name to match —
//!   the listener's blocking `accept()`, a blocking channel `send` (unbounded channels use the same
//!   name and never block), and the pseudoterminal's own `read`, which is the parked side of D32.
//!   Nor the waits hidden under other names: running a command to completion (`output()`,
//!   `status()`, `wait_with_output()`), a thread scope that joins at its end, a call written as a
//!   path (`JoinHandle::join(h)`), or parking a thread. Turning a declared wait into one of those
//!   lowers the declared count, and the equality below is what notices.
//! - **Test modules in shapes other than one**: a `#[cfg(test)]` line of its own, other attributes
//!   after it, then a `mod … {` header on one line. Any other shape is read as production code: an
//!   undeclared wait inside it fails the guard, and a declared one counts toward the pinned number.
//! - **Whether a declaration is true.** It checks that a reason is written, not that it holds. That
//!   is the reviewer's job, and it is why the number of declarations is pinned below: every new one
//!   is an edit here that a reviewer sees.
//! - **Test code**: every `#[cfg(test)]` module is removed before judging, by counting its braces, so
//!   production code written *after* a test module is still read.

use std::path::{Path, PathBuf};

/// How many waits the workspace declares today. **An equality, not a floor**: a floor would let the
/// number go stale the day a wait is removed, and an equality is also what goes red if the matcher
/// stops matching anything at all.
const DECLARED_WAITS: usize = 9;

/// The crates, files and production lines the walk must reach. A walk that examined nothing finds
/// no undeclared wait, and would pass.
const CRATES_AT_LEAST: usize = 5;
const FILES_AT_LEAST: usize = 50;
const LINES_AT_LEAST: usize = 70_000;

const MARKERS: &[&str] = &["Bounded:", "Abandoned on purpose:"];

/// One source file seen as two layers of the same lines: what is code, and what is comment.
///
/// String and character literals are blanked out of the code, not only so a literal that names a
/// call is not read as one: a `"{}"` inside a test module would throw off the brace count that
/// removes it, and a `"//"` would open a comment that is not there.
struct Layers {
    code: Vec<String>,
    comments: Vec<String>,
}

fn layers(src: &str) -> Layers {
    #[derive(PartialEq)]
    enum Where {
        Code,
        LineComment,
        BlockComment,
        Text,
    }

    let chars: Vec<char> = src.chars().collect();
    let mut code = vec![String::new()];
    let mut comments = vec![String::new()];
    let mut place = Where::Code;
    // Rust block comments nest: leaving at the first `*/` would hand the prose after it to the code.
    let mut depth = 0usize;
    let mut i = 0usize;

    // Blanked characters become spaces so the columns of what stays code do not move.
    let push = |code: &mut Vec<String>, comments: &mut Vec<String>, c: char, into_code: bool| {
        if c == '\n' {
            code.push(String::new());
            comments.push(String::new());
            return;
        }
        let (keep, blank) = if into_code {
            (code.last_mut(), comments.last_mut())
        } else {
            (comments.last_mut(), code.last_mut())
        };
        keep.expect("never empty").push(c);
        blank.expect("never empty").push(' ');
    };

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match place {
            Where::LineComment => {
                if c == '\n' {
                    place = Where::Code;
                }
                push(&mut code, &mut comments, c, false);
                i += 1;
            }
            Where::BlockComment => {
                if c == '/' && next == Some('*') {
                    depth += 1;
                } else if c == '*' && next == Some('/') {
                    depth -= 1;
                    if depth == 0 {
                        place = Where::Code;
                    }
                    push(&mut code, &mut comments, c, false);
                    push(&mut code, &mut comments, '/', false);
                    i += 2;
                    continue;
                }
                push(&mut code, &mut comments, c, false);
                i += 1;
            }
            Where::Text => {
                if c == '\\' {
                    push(&mut code, &mut comments, ' ', true);
                    if let Some(escaped) = next {
                        push(
                            &mut code,
                            &mut comments,
                            blank_unless_newline(escaped),
                            true,
                        );
                    }
                    i += 2;
                    continue;
                }
                if c == '"' {
                    place = Where::Code;
                    push(&mut code, &mut comments, c, true);
                } else {
                    push(&mut code, &mut comments, blank_unless_newline(c), true);
                }
                i += 1;
            }
            Where::Code => {
                if c == '/' && next == Some('/') {
                    place = Where::LineComment;
                    push(&mut code, &mut comments, c, false);
                    i += 1;
                    continue;
                }
                if c == '/' && next == Some('*') {
                    place = Where::BlockComment;
                    depth = 1;
                    push(&mut code, &mut comments, c, false);
                    push(&mut code, &mut comments, '*', false);
                    i += 2;
                    continue;
                }
                // A raw string ends only at a quote followed by as many `#` as opened it, and this
                // repository writes its JSON fixtures that way, full of plain quotes — `br#"…"#`
                // included, whose `b` is what a check for «not the tail of a word» would trip on.
                if c == 'r'
                    && matches!(next, Some('#') | Some('"'))
                    && opens_a_raw_string(&chars, i)
                {
                    let mut hashes = 0usize;
                    while chars.get(i + 1 + hashes) == Some(&'#') {
                        hashes += 1;
                    }
                    if chars.get(i + 1 + hashes) == Some(&'"') {
                        let mut j = i + 2 + hashes;
                        while j < chars.len()
                            && !(chars[j] == '"'
                                && (0..hashes).all(|k| chars.get(j + 1 + k) == Some(&'#')))
                        {
                            j += 1;
                        }
                        let end = (j + 1 + hashes).min(chars.len());
                        for &inside in &chars[i..end] {
                            push(&mut code, &mut comments, blank_unless_newline(inside), true);
                        }
                        i = end;
                        continue;
                    }
                }
                if c == '"' {
                    place = Where::Text;
                    push(&mut code, &mut comments, c, true);
                    i += 1;
                    continue;
                }
                // A character literal, told apart from a lifetime by what closes it.
                if c == '\'' {
                    let escaped = next == Some('\\');
                    if escaped || chars.get(i + 2) == Some(&'\'') {
                        let mut j = i + 1;
                        while j < chars.len() && chars[j] != '\'' {
                            j += if chars[j] == '\\' { 2 } else { 1 };
                        }
                        let end = (j + 1).min(chars.len());
                        for _ in i..end {
                            push(&mut code, &mut comments, ' ', true);
                        }
                        i = end;
                        continue;
                    }
                }
                push(&mut code, &mut comments, c, true);
                i += 1;
            }
        }
    }
    Layers { code, comments }
}

fn blank_unless_newline(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

/// Whether the `r` at `i` opens a raw string: it starts a token, or follows the `b` or `c` of a byte
/// or C string that does. An `r` at the tail of an identifier is not one.
fn opens_a_raw_string(chars: &[char], i: usize) -> bool {
    let starts_a_token = |at: usize| at == 0 || !is_word(chars[at - 1]);
    starts_a_token(i) || (matches!(chars[i - 1], 'b' | 'c') && starts_a_token(i - 1))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Blanks every `#[cfg(test)] mod … { … }` out of both layers, counting braces in the code layer —
/// where literals and comments are already gone — so the lines after the block are still judged.
fn without_test_modules(layers: &mut Layers) {
    let mut row = 0;
    while row < layers.code.len() {
        if layers.code[row].trim() != "#[cfg(test)]" {
            row += 1;
            continue;
        }
        let mut open = row + 1;
        while open < layers.code.len() && layers.code[open].trim().starts_with("#[") {
            open += 1;
        }
        let Some(header) = layers.code.get(open) else {
            break;
        };
        let words: Vec<&str> = header.split_whitespace().collect();
        let is_module = words.first() == Some(&"mod")
            || (words.first().is_some_and(|w| w.starts_with("pub"))
                && words.get(1) == Some(&"mod"));
        if !is_module || !header.contains('{') {
            row += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut end = open;
        'block: for (at, line) in layers.code.iter().enumerate().skip(open) {
            for c in line.chars() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = at;
                            break 'block;
                        }
                    }
                    _ => {}
                }
            }
        }
        for gone in row..=end {
            layers.code[gone].clear();
            layers.comments[gone].clear();
        }
        row = end + 1;
    }
}

/// The waits a code line calls, by name, in the order they appear.
fn waits_on(line: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for (dot, _) in line.match_indices('.') {
        let after = line[dot + 1..].trim_start();
        let name_len = after
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let (name, rest) = after.split_at(name_len);
        let Some(arguments) = rest.trim_start().strip_prefix('(') else {
            continue;
        };
        let wait = match name {
            "recv" => "recv",
            "wait" => "wait",
            "wait_while" => "wait_while",
            "join" if arguments.trim_start().starts_with(')') => "join",
            _ => continue,
        };
        found.push(wait);
    }
    found
}

/// Whether a comment line **starts** with one of the two markers and carries a reason after it.
///
/// Starts, not contains: «Not Bounded: nothing wakes this» mentions the marker and says the opposite.
fn declares(comment: &str) -> bool {
    let text = comment_text(comment);
    MARKERS.iter().any(|marker| {
        text.strip_prefix(marker)
            .is_some_and(|reason| !comment_text(reason).is_empty())
    })
}

/// A comment line without its slashes, bold markers and surrounding spaces.
fn comment_text(comment: &str) -> &str {
    comment.trim_matches(['/', '*', ' '])
}

/// What one source says about its waits: those with no declaration, and how many are declared.
struct Judgement {
    undeclared: Vec<(usize, &'static str)>,
    declared: usize,
}

/// **The rule, and the only place it lives.** The walk and every fixture call this one function.
fn judge(src: &str) -> Judgement {
    let mut layers = layers(src);
    // An inner crate/module attribute makes every item in an external test source test-only.
    if layers
        .code
        .iter()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.trim() == "#![cfg(test)]")
    {
        return Judgement {
            undeclared: Vec::new(),
            declared: 0,
        };
    }
    without_test_modules(&mut layers);
    let mut judgement = Judgement {
        undeclared: Vec::new(),
        declared: 0,
    };
    for (row, line) in layers.code.iter().enumerate() {
        // One declaration answers for one wait: a second wait on the same line would otherwise
        // inherit a reason written for the first.
        for (nth, wait) in waits_on(line).into_iter().enumerate() {
            if nth == 0 && declared_at(&layers, row) {
                judgement.declared += 1;
            } else {
                judgement.undeclared.push((row + 1, wait));
            }
        }
    }
    judgement
}

/// A trailing comment on the call's line, or a paragraph of the comment block ending right above
/// it.
///
/// **The marker has to open a paragraph** — the first line of the block, or the first after an empty
/// `//` line. Comments here are wrapped by hand, so «deliberately not» at the end of one line and
/// «Bounded: nothing wakes it» at the start of the next would otherwise declare the opposite.
fn declared_at(layers: &Layers, row: usize) -> bool {
    if declares(&layers.comments[row]) {
        return true;
    }
    let only_comment =
        |at: usize| layers.code[at].trim().is_empty() && !layers.comments[at].trim().is_empty();
    let opens_a_paragraph = |at: usize| {
        at == 0 || !only_comment(at - 1) || comment_text(&layers.comments[at - 1]).is_empty()
    };
    let mut above = row;
    while above > 0 {
        above -= 1;
        if !only_comment(above) {
            return false;
        }
        if declares(&layers.comments[above]) && opens_a_paragraph(above) {
            return true;
        }
    }
    false
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above this crate")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|failure| panic!("cannot read {}: {failure}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Every crate under `crates/` that has a `src/`, and the Rust files under it.
fn production_sources(root: &Path) -> Vec<(String, Vec<PathBuf>)> {
    let mut crates: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .expect("the crates directory is readable")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("src").is_dir())
        .collect();
    crates.sort();
    crates
        .into_iter()
        .map(|dir| {
            let name = dir
                .file_name()
                .expect("a crate directory has a name")
                .to_string_lossy()
                .into_owned();
            (name, rust_files(&dir.join("src")))
        })
        .collect()
}

#[test]
fn every_blocking_wait_in_production_code_is_bounded_or_declared_abandoned() {
    let root = repo();
    let sources = production_sources(&root);
    let mut undeclared = Vec::new();
    let mut declared = 0;
    let mut files = 0;
    let mut lines = 0;
    for (_, paths) in &sources {
        for path in paths {
            let src = std::fs::read_to_string(path)
                .unwrap_or_else(|failure| panic!("cannot read {}: {failure}", path.display()));
            files += 1;
            lines += src.lines().count();
            let judgement = judge(&src);
            declared += judgement.declared;
            let shown = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            for (line, wait) in judgement.undeclared {
                undeclared.push(format!("{shown}:{line}: `{wait}`"));
            }
        }
    }

    assert!(
        sources.len() >= CRATES_AT_LEAST && files >= FILES_AT_LEAST && lines >= LINES_AT_LEAST,
        "the walk reached {} crates, {files} files and {lines} lines; a walk that reaches less than \
         {CRATES_AT_LEAST} / {FILES_AT_LEAST} / {LINES_AT_LEAST} is not looking where the code is",
        sources.len()
    );
    assert!(
        undeclared.is_empty(),
        "these waits can block for ever and say nothing about it. Give each one a deadline form, or \
         a comment paragraph right above it that opens with `Bounded:` (what ends the wait) or \
         `Abandoned on purpose:` (who is left behind, and why). One wait per line — see D57:\n{}",
        undeclared.join("\n")
    );
    assert_eq!(
        declared, DECLARED_WAITS,
        "the workspace declares {declared} waits and this guard records {DECLARED_WAITS}. If a \
         declared wait was added or removed on purpose, update `DECLARED_WAITS` — the edit is how a \
         reviewer finds out"
    );
}

#[test]
fn an_undeclared_wait_is_reported_with_its_line() {
    let src = "fn f(rx: Receiver<()>, t: JoinHandle<()>, cv: Condvar) {\n\
               \x20   let _ = rx.recv();\n\
               \x20   let _ = t.join();\n\
               \x20   let _ = cv.wait(guard);\n\
               \x20   let _ = cv.wait_while(guard, |x| *x);\n\
               }\n";
    let judgement = judge(src);
    assert_eq!(
        judgement.undeclared,
        vec![(2, "recv"), (3, "join"), (4, "wait"), (5, "wait_while")]
    );
    assert_eq!(judgement.declared, 0);
}

#[test]
fn deadline_forms_and_joins_with_an_argument_are_not_waits() {
    let src = "fn f() {\n\
               \x20   let _ = rx.recv_timeout(d);\n\
               \x20   let _ = cv.wait_timeout(g, d);\n\
               \x20   let _ = cv.wait_timeout_while(g, d, |x| *x);\n\
               \x20   let _ = lines.join(\"\\n\");\n\
               \x20   let _ = root.join(\"crates\");\n\
               }\n";
    assert!(judge(src).undeclared.is_empty());
}

#[test]
fn a_wait_named_in_a_comment_or_a_literal_is_not_a_call() {
    let src = "fn f() {\n\
               \x20   // then `rx.recv()` returns\n\
               \x20   /* t.join() /* nested */ cv.wait(g) */\n\
               \x20   let a = \"rx.recv()\";\n\
               \x20   let b = r#\"t.join() \"quoted\" \"#;\n\
               \x20   let c = '\"'; let d = x.len();\n\
               }\n";
    assert!(judge(src).undeclared.is_empty());
}

#[test]
fn an_external_source_requires_a_real_inner_test_only_attribute() {
    assert!(
        judge("#![cfg(test)]\nfn fixture() { rx.recv(); }\n")
            .undeclared
            .is_empty()
    );
    assert_eq!(
        judge("// #![cfg(test)]\nfn production() { rx.recv(); }\n").undeclared,
        vec![(2, "recv")]
    );
    assert_eq!(
        judge("const TEXT: &str = \"#![cfg(test)]\";\nfn production() { rx.recv(); }\n").undeclared,
        vec![(2, "recv")]
    );
}

#[test]
fn a_wait_inside_a_test_module_is_not_judged_but_one_after_it_is() {
    let src = "fn production() {}\n\
               \n\
               #[cfg(test)]\n\
               mod tests {\n\
               \x20   fn helper() { let _ = format!(\"{{\"); }\n\
               \x20   #[test]\n\
               \x20   fn waits() { let _ = rx.recv(); }\n\
               }\n\
               \n\
               fn after() {\n\
               \x20   let _ = rx.recv();\n\
               }\n";
    assert_eq!(judge(src).undeclared, vec![(11, "recv")]);
}

#[test]
fn braces_and_slashes_inside_literals_do_not_fool_the_test_module_count() {
    // An unbalanced `{` in a literal would keep the block open to the end of the file, and a `//`
    // in a literal would hide the rest of its line: either way the wait below would go unseen.
    let src = "#[cfg(test)]\n\
               mod tests {\n\
               \x20   const OPEN: &str = \"{\";\n\
               \x20   const URL: char = '{';\n\
               }\n\
               fn after() { let s = \"//\"; let _ = rx.recv(); }\n";
    assert_eq!(judge(src).undeclared, vec![(6, "recv")]);
}

#[test]
fn either_marker_with_a_reason_declares_the_wait() {
    let src = "fn f() {\n\
               \x20   // Some context first.\n\
               \x20   //\n\
               \x20   // **Bounded:** the sender is dropped by the destructor before anything else.\n\
               \x20   let _ = rx.recv();\n\
               \x20   // Abandoned on purpose: waiting here froze the window.\n\
               \x20   let _ = t.join();\n\
               \x20   let _ = cv.wait(g); // Bounded: every read ends under the lock.\n\
               }\n";
    let judgement = judge(src);
    assert!(
        judgement.undeclared.is_empty(),
        "{:?}",
        judgement.undeclared
    );
    assert_eq!(judgement.declared, 3);
}

#[test]
fn a_marker_without_a_reason_away_from_the_call_or_in_other_words_does_not_declare() {
    let src = "fn f() {\n\
               \x20   // Bounded:\n\
               \x20   let _ = rx.recv();\n\
               \x20   // **Abandoned on purpose:**\n\
               \x20   let _ = t.join();\n\
               \x20   // Bounded: by the knock.\n\
               \x20   let x = 1;\n\
               \x20   let _ = cv.wait(g);\n\
               \x20   // Unbounded on purpose: nobody wakes it.\n\
               \x20   let _ = rx.recv();\n\
               \x20   // Bounded: a reason.\n\
               \n\
               \x20   let _ = rx.recv();\n\
               }\n";
    assert_eq!(
        judge(src).undeclared,
        vec![
            (3, "recv"),
            (5, "join"),
            (8, "wait"),
            (10, "recv"),
            (13, "recv")
        ]
    );
}

#[test]
fn a_wait_split_across_a_chain_is_still_seen() {
    let src = "fn f() {\n\
               \x20   state = self\n\
               \x20       .finished\n\
               \x20       .wait(state)\n\
               \x20       .expect(\"poisoned\");\n\
               }\n";
    assert_eq!(judge(src).undeclared, vec![(4, "wait")]);
}

#[test]
fn a_raw_byte_string_full_of_quotes_does_not_hide_the_code_after_it() {
    // Read as an ordinary string, an odd number of quotes inside leaves the scanner in text, and the
    // `recv` after it lands inside a string. An even number would hide nothing, so this one is odd.
    let src = "fn f() {\n\
               \x20   let s = br#\"a\"b\"#; let _ = rx.recv();\n\
               \x20   let t = cr#\"a\"b\"#; let _ = t.join();\n\
               }\n";
    assert_eq!(judge(src).undeclared, vec![(2, "recv"), (3, "join")]);
}

#[test]
fn a_marker_mentioned_mid_sentence_does_not_declare() {
    let src = "fn f() {\n\
               \x20   // Not Bounded: nothing ever wakes this.\n\
               \x20   let _ = rx.recv();\n\
               \x20   // This is not Abandoned on purpose: we forgot.\n\
               \x20   let _ = t.join();\n\
               \x20   /// **Bounded:** a doc comment counts too.\n\
               \x20   let _ = cv.wait(g);\n\
               \x20   // This loop is deliberately not\n\
               \x20   // Bounded: nothing ever wakes it.\n\
               \x20   let _ = rx.recv();\n\
               }\n";
    let judgement = judge(src);
    assert_eq!(
        judgement.undeclared,
        vec![(3, "recv"), (5, "join"), (10, "recv")]
    );
    assert_eq!(judgement.declared, 1);
}

#[test]
fn one_declaration_answers_for_one_wait() {
    let src = "fn f() {\n\
               \x20   let _ = (a.recv(), t.join()); // Bounded: a's sender drops on close.\n\
               }\n";
    let judgement = judge(src);
    assert_eq!(judgement.undeclared, vec![(2, "join")]);
    assert_eq!(judgement.declared, 1);
}
