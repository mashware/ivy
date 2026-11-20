use super::*;
use Token::{Comment, Keyword, Meta, Number, Text, Type};
use std::path::Path;

/// Each run of a line as the text it covers, so a test reads like the line it checks.
fn pieces(line: &Line) -> Vec<(&str, Token)> {
    line.runs
        .iter()
        .enumerate()
        .map(|(n, (start, token))| {
            let end = line
                .runs
                .get(n + 1)
                .map_or(line.text.len(), |(next, _)| *next);
            (&line.text[*start..end], *token)
        })
        .collect()
}

fn one(text: &str, family: Family) -> Vec<(String, Token)> {
    let lines = scan(text, family);
    assert_eq!(lines.len(), 1, "{lines:?}");
    pieces(&lines[0])
        .into_iter()
        .map(|(s, t)| (s.to_owned(), t))
        .collect()
}

fn has(pieces: &[(String, Token)], text: &str, token: Token) -> bool {
    pieces.iter().any(|(s, t)| s.trim() == text && *t == token)
}

#[test]
fn a_rust_line_colours_its_comment_string_number_keyword_and_type() {
    let line = one(
        r#"fn main() { let x: Vec<u8> = "text".into(); 42 } // note"#,
        Family::Rust,
    );
    assert!(has(&line, "fn", Keyword), "{line:?}");
    assert!(has(&line, "let", Keyword), "{line:?}");
    assert!(has(&line, "Vec", Type), "{line:?}");
    assert!(has(&line, "\"text\"", Token::String), "{line:?}");
    assert!(has(&line, "42", Number), "{line:?}");
    assert!(has(&line, "// note", Comment), "{line:?}");
    assert!(has(&line, "main", Text) || line.iter().any(|(s, t)| s.contains("main") && *t == Text));
}

#[test]
fn a_number_inside_a_word_and_a_lifetime_stay_text() {
    let line = one("fn f<'a>(x2: &'a str) {}", Family::Rust);
    assert!(!line.iter().any(|(_, t)| *t == Number), "{line:?}");
    assert!(!line.iter().any(|(_, t)| *t == Token::String), "{line:?}");
    let chars = one(r"let c = 'x'; let n = '\n';", Family::Rust);
    assert!(has(&chars, "'x'", Token::String), "{chars:?}");
    assert!(has(&chars, r"'\n'", Token::String), "{chars:?}");
}

#[test]
fn block_comments_carry_across_lines_and_nest_in_rust() {
    // The markers are spelled as escapes so the repository's comment walkers do not read them.
    let lines = scan(
        "a /\x2a one\n/\x2a two \x2a/ still\nend \x2a/ b",
        Family::Rust,
    );
    assert!(
        pieces(&lines[1]).iter().all(|(_, t)| *t == Comment),
        "{lines:?}"
    );
    let last = pieces(&lines[2]);
    assert_eq!(last.first().map(|p| p.1), Some(Comment), "{last:?}");
    assert!(
        last.iter().any(|(s, t)| s.contains('b') && *t == Text),
        "{last:?}"
    );
    // In C a block comment does not nest: the first closing marker closes it.
    let c = scan("/\x2a a /\x2a b \x2a/ x", Family::CLike);
    assert!(
        pieces(&c[0])
            .iter()
            .any(|(s, t)| s.contains('x') && *t == Text),
        "{c:?}"
    );
}

#[test]
fn strings_that_span_lines_carry_and_single_quotes_end_with_the_line() {
    let python = scan(
        "s = \"\"\"one\ntwo # not a comment\nthree\"\"\" + x",
        Family::Hash,
    );
    assert!(
        pieces(&python[1]).iter().all(|(_, t)| *t == Token::String),
        "{python:?}"
    );
    assert!(
        pieces(&python[2])
            .iter()
            .any(|(s, t)| s.contains('x') && *t == Text),
        "{python:?}"
    );
    let raw = scan("let s = r#\"a \"quoted\" b\"#; let y = 1;", Family::Rust);
    let raw = pieces(&raw[0]);
    assert!(
        raw.iter()
            .any(|(s, t)| *s == "r#\"a \"quoted\" b\"#" && *t == Token::String),
        "{raw:?}"
    );
    assert!(
        raw.iter().any(|(s, t)| *s == "1" && *t == Number),
        "{raw:?}"
    );
    // A JavaScript string the line leaves open does not swallow the next line.
    let js = scan("let a = 'open\nlet b = 2", Family::CLike);
    assert!(
        pieces(&js[1])
            .iter()
            .any(|(s, t)| *s == "let" && *t == Keyword),
        "{js:?}"
    );
}

#[test]
fn json_keys_are_keywords_and_values_are_strings() {
    let line = one(r#"{"name": "ivy", "n": 3, "ok": true}"#, Family::Json);
    assert!(has(&line, "\"name\"", Keyword), "{line:?}");
    assert!(has(&line, "\"ivy\"", Token::String), "{line:?}");
    assert!(has(&line, "3", Number), "{line:?}");
    assert!(has(&line, "true", Keyword), "{line:?}");
}

#[test]
fn hash_comments_variables_and_preprocessor_lines_are_marked() {
    let shell = one("echo $HOME # where", Family::Hash);
    assert!(has(&shell, "$HOME", Meta), "{shell:?}");
    assert!(has(&shell, "# where", Comment), "{shell:?}");
    let c = scan("#include <stdio.h>\nint x = a # b;", Family::CLike);
    assert!(pieces(&c[0]).iter().all(|(_, t)| *t == Meta), "{c:?}");
    assert!(!pieces(&c[1]).iter().any(|(_, t)| *t == Meta), "{c:?}");
    let php = one("$user = new User(); # done", Family::Php);
    assert!(has(&php, "$user", Meta), "{php:?}");
    assert!(has(&php, "new", Keyword), "{php:?}");
    assert!(has(&php, "# done", Comment), "{php:?}");
    let sql = one("SELECT id FROM t -- all", Family::Sql);
    assert!(
        has(&sql, "SELECT", Keyword) && has(&sql, "-- all", Comment),
        "{sql:?}"
    );
}

#[test]
fn plain_text_is_one_run_per_line_and_line_endings_are_dropped() {
    let lines = scan("a \"b\" 3 // c\r\nsecond\r\n", Family::Plain);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "a \"b\" 3 // c");
    assert_eq!(lines[0].runs, vec![(0, Text)]);
    assert_eq!(lines[1].text, "second");
}

#[test]
fn a_line_over_the_limit_is_cut_on_a_character_and_painted_plain() {
    let long = format!("{}é tail", "x".repeat(LINE_LIMIT - 1));
    let lines = scan(&format!("{long}\nfn"), Family::Rust);
    assert!(lines[0].truncated());
    assert_eq!(lines[0].text, long, "the line keeps all of its text");
    assert!(lines[0].shown <= LINE_LIMIT);
    assert!(lines[0].text.is_char_boundary(lines[0].shown));
    assert_eq!(lines[0].runs, vec![(0, Text)]);
    assert!(!lines[1].truncated());
    assert_eq!(pieces(&lines[1]), vec![("fn", Keyword)]);
}

#[test]
fn families_come_from_the_extension_the_name_or_a_shebang() {
    let of = |name: &str, first: &str| Family::of(Path::new(name), first);
    assert_eq!(of("/r/main.rs", ""), Family::Rust);
    assert_eq!(of("/r/App.TSX", ""), Family::CLike);
    assert_eq!(of("/r/index.PHP", ""), Family::Php);
    assert_eq!(of("/r/ci.yml", ""), Family::Hash);
    assert_eq!(of("/r/Makefile", ""), Family::Hash);
    assert_eq!(of("/r/run", "#!/bin/sh"), Family::Hash);
    assert_eq!(of("/r/data.json", ""), Family::Json);
    assert_eq!(of("/r/notes.txt", "#!/bin/sh"), Family::Plain);
    assert_eq!(of("/r/LICENSE", "MIT"), Family::Plain);
}

#[test]
fn css_has_no_line_comments_and_rust_raw_strings_take_their_prefixes() {
    let css = one("a { background: url(https://x.org/a.png); }", Family::Css);
    assert!(!css.iter().any(|(_, t)| *t == Comment), "{css:?}");
    assert_eq!(Family::of(Path::new("/r/site.css"), ""), Family::Css);
    for raw in ["br#\"a \" b\"#", "cr\"x\"", "r\"y\""] {
        let lines = scan(&format!("let v = {raw}; let n = 1;\nfn"), Family::Rust);
        let first = pieces(&lines[0]);
        assert!(
            first
                .iter()
                .any(|(s, t)| s.ends_with(raw) && *t == Token::String)
                || first.iter().any(|(s, t)| *s == raw && *t == Token::String),
            "{raw}: {first:?}"
        );
        assert_eq!(
            pieces(&lines[1]),
            vec![("fn", Keyword)],
            "{raw} bled into the next line"
        );
    }
}
