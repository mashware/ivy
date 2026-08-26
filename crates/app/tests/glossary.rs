//! The inverted ratchet: **it fails for what is still in Spanish**, crate by crate.
//!
//! Every other guard in this repository protects behaviour. This one protects a decision — the
//! glossary written in `docs/02-decisiones.md` — and it is the only thing standing between a
//! milestone that renames tens of thousands of lines and a tree that ends up half translated
//! without anybody noticing. The compiler cannot help: a Spanish identifier compiles perfectly.
//!
//! **It derives the Spanish vocabulary from the glossary instead of repeating it.** A test that
//! copied the list would be a second place to keep it up to date, and the first time the two
//! disagreed the guard would be the one that is wrong. This is the same mechanism that ties the
//! measurement script to the report it reads: derive, never duplicate.
//!
//! **And it is scoped per crate, on purpose.** A single repo-wide list that shrinks towards zero
//! would be edited by several deliveries at once — four of them translate prose in parallel — and
//! every one of them would land on the same lines. A list of *finished crates* only ever grows, one
//! entry per delivery, so two deliveries never touch it together.
//!
//! # What it does not see, said out loud
//!
//! - **A lowercase Spanish noun that the glossary never names.** The glossary decides *types*, so a
//!   local called `ruta` or `hueco` is invisible here. Precision was chosen over reach on purpose:
//!   this guard breaks the build, and a lexicon wide enough to catch every Spanish word would also
//!   catch English ones. What closes that gap is reading the diff, not widening the pattern.
//! - **Test function names.** They are prose — sentences, not identifiers — and they are translated
//!   in the prose deliveries, so they are skipped by the `#[test]` above them. Everything else
//!   inside a test module is checked, locals included.
//! - **Comments, rustdoc and string literals**, for the same reason: prose has its own delivery.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The decisions record, which carries the glossary in its annex.
const DECISIONS: &str = include_str!("../../../docs/02-decisiones.md");

/// The crates whose identifiers are **finished**.
///
/// One entry per delivery that finishes a crate. `app` joins the list when the second half of its
/// names lands; until then its Spanish identifiers are the expected state, not a defect, and having
/// them reported would train everybody to ignore this test.
const FINISHED_CRATES: &[&str] = &["panel-contract", "host-load", "term-core"];

/// How much each shape of the annex has to yield for this test to mean anything.
///
/// **Without a floor this test passes by being empty**, which is the failure mode a guard is most
/// likely to end in: someone reshapes a table in the decisions record, the extractor stops matching,
/// every crate looks clean, and nothing says otherwise. **One floor per shape and not one over the
/// sum**: the tables carry eight names no arrow carries, and a total of 112 losing 8 still clears any
/// floor worth setting. The real counts are comfortably above these; they are here to notice a
/// collapse, not to pin the glossary's size.
const ARROWS_AT_LEAST: usize = 100;
const TABLES_AT_LEAST: usize = 9;

/// Spanish function words, which give away a compound name the glossary never had to name.
///
/// The glossary decides **types**, so on its own it cannot see `base_de`, `mitad_del_motor` or
/// `lo_quiere` — and in the crate that is still to come those are most of what there is. A function
/// word is the safe half of a Spanish lexicon: it is short, it is closed, and it is checked as a
/// **whole segment**, so `serde` is not `de` and `config` is not `con`.
///
/// **`sin` is deliberately absent**: it is also the English trigonometric function, and a guard that
/// breaks the build has to be wrong less often than the code it judges.
const FUNCTION_WORDS: &[&str] = &[
    "de", "del", "la", "el", "los", "las", "lo", "que", "con", "por", "para", "una", "uno", "al",
    "mas", "ya",
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above this crate")
}

/// The Spanish side of the glossary, the English side, and how many came from each shape.
///
/// The English side matters because de-pluralising an identifier can land it on a Spanish entry by
/// accident — `Series` becomes `serie`, and `Serie` is precisely the name it replaced. A segment that
/// *is* an English glossary name is never a leftover, so it is exempt before anything else is asked.
///
/// **The counts are returned per shape and not as one total**, which is the lesson the previous
/// delivery paid for: a single floor over the sum hides the loss of a whole shape. Measured — losing
/// both tables drops the total from 112 to 104 and a floor of 100 never notices.
struct Glossary {
    spanish: HashSet<String>,
    english: HashSet<String>,
    from_arrows: usize,
    from_tables: usize,
}

/// Lowercase, and Spanish accents folded onto ASCII.
///
/// **Both sides get folded, and that is what closes a whole family of leftovers.** The glossary
/// writes `Pestaña` and `TamañoPedido`; the code writes `pestanas` and `tamano_pedido`, because
/// identifiers in this repository avoid the tilde on purpose. Without folding, the two never meet —
/// measured, with `TAMANO`, which is a real constant of the measurement script.
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

fn glossary() -> Glossary {
    let annex = DECISIONS
        .split_once("### El glosario, y las dos fronteras")
        .expect("the decisions record no longer carries the glossary annex")
        .1;

    let mut spanish = HashSet::new();
    let mut english = HashSet::new();
    let mut from_arrows = 0;
    let mut from_tables = 0;

    // Shape 1, and the bulk of it: `Spanish`→`English`, sometimes with a parenthesis in between and
    // sometimes with the English name in bold.
    for line in annex.lines() {
        let mut rest = line;
        while let Some((before, after)) = rest.split_once('→') {
            if let (Some(a), Some(b)) = (last_backticked(before), first_backticked(after)) {
                spanish.insert(fold(a));
                english.insert(fold(b));
                from_arrows += 1;
            }
            rest = after;
        }
    }

    // Shape 2: the two tables whose first column is Spanish **by their own heading**. Keying off the
    // heading is what makes this safe to widen: the `Stored*` table also has a name in its first
    // column, but that name is already English, and reading it as Spanish would break the build for
    // every crate that adopted it.
    for heading in ["| Tipo | Primer nombre", "| Hoy | Qué es"] {
        // **The heading row is looked up WHOLE**, not with `split_once` over the text: splitting on
        // the prefix leaves half a row — from «| Queda |» onwards — and the column index taken from
        // that half points at the first column, which is the Spanish one. The first version of this
        // put the Spanish names into the **English** set, exempting them: worse than the defect it
        // came to fix. Its own `reparto` test case caught it.
        let mut rows = annex
            .lines()
            .skip_while(|line| !line.starts_with(heading))
            .peekable();
        let head: Vec<&str> = rows
            .peek()
            .unwrap_or_else(|| {
                panic!(
                    "the glossary annex no longer has a table headed «{heading}». If it was \
                     translated or reshaped, this anchor has to move with it — skipping the table \
                     instead would leave the guard reporting every crate clean with eight names \
                     missing from its vocabulary"
                )
            })
            .split('|')
            .collect();
        // Which column holds the adopted name is read off the heading rather than counted by hand: in
        // one of these two tables the first backticked token after column 1 is the **discarded** name,
        // and in the other the last one is a Spanish name quoted inside the prose.
        let kept = head
            .iter()
            .position(|c| c.trim() == "Queda")
            .unwrap_or_else(|| {
                panic!("the table headed «{heading}» no longer has a «Queda» column")
            });

        for line in rows.skip(1) {
            if !line.starts_with('|') {
                break;
            }
            let cells: Vec<&str> = line.split('|').collect();
            if let (Some(a), Some(b)) = (
                first_backticked(cells.get(1).unwrap_or(&"")),
                cells.get(kept).and_then(|c| first_backticked(c)),
            ) {
                spanish.insert(fold(a));
                english.insert(fold(b));
                from_tables += 1;
            }
        }
    }

    Glossary {
        spanish,
        english,
        from_arrows,
        from_tables,
    }
}

fn first_backticked(text: &str) -> Option<&str> {
    let (_, after) = text.split_once('`')?;
    let (inside, _) = after.split_once('`')?;
    (!inside.is_empty() && !inside.contains(' ')).then_some(inside)
}

fn last_backticked(text: &str) -> Option<&str> {
    let end = text.rfind('`')?;
    let start = text[..end].rfind('`')?;
    let inside = &text[start + 1..end];
    (!inside.is_empty() && !inside.contains(' ')).then_some(inside)
}

/// The identifiers of a Rust file, with prose and literals removed and test names skipped.
///
/// Written by hand rather than pulled in as a dependency: the whole job is «which characters are
/// code», and a parser would be a new package in the tree for the sake of four states.
fn identifiers(src: &str) -> Vec<(usize, String)> {
    #[derive(PartialEq)]
    enum Where {
        Code,
        LineComment,
        BlockComment,
        Text,
    }

    let mut found = Vec::new();
    let bytes: Vec<char> = src.chars().collect();
    let mut place = Where::Code;
    // **Rust block comments nest**, and leaving at the first `*/` bites both ways: the prose after it
    // reads as code — a false positive breaks the build — and a `#[test]` living in there switches off
    // the next function's name. Both measured.
    let mut comment_depth = 0usize;
    let mut line = 1usize;
    let mut i = 0usize;
    let mut word = String::new();
    let mut word_line = 1usize;
    // Set by the attribute `#[test]`, cleared by the next `fn`: what it skips is that one
    // function's **name**.
    //
    // **It has to match the attribute, not the word.** The first version raised this flag on any
    // bare `test`, and `#[cfg(test)]` opens every test module in this repository — so the first
    // function of each one had its name skipped, silently. A mutation found it: a helper renamed
    // back to Spanish stayed green because it happened to be that first function.
    let mut inside_a_test_attribute = false;
    let mut previous_word = String::new();

    while i < bytes.len() {
        let c = bytes[i];
        if c == '\n' {
            line += 1;
        }
        match place {
            Where::LineComment => {
                if c == '\n' {
                    place = Where::Code;
                }
                i += 1;
                continue;
            }
            Where::BlockComment => {
                if c == '/' && bytes.get(i + 1) == Some(&'*') {
                    comment_depth += 1;
                    i += 2;
                    continue;
                }
                if c == '*' && bytes.get(i + 1) == Some(&'/') {
                    comment_depth -= 1;
                    if comment_depth == 0 {
                        place = Where::Code;
                    }
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            Where::Text => {
                if c == '\\' {
                    // **The line is counted by hand here.** Skipping two characters also skips the
                    // increment at the top of the loop, and a `\\` + newline continuation is exactly
                    // that: the number drifted by one line per earlier continuation in the file, and
                    // this repository breaks almost every assertion message with a `\\`.
                    if bytes.get(i + 1) == Some(&'\n') {
                        line += 1;
                    }
                    i += 2;
                    continue;
                }
                if c == '"' {
                    place = Where::Code;
                }
                i += 1;
                continue;
            }
            Where::Code => {}
        }

        if c == '/' && bytes.get(i + 1) == Some(&'/') {
            place = Where::LineComment;
            i += 2;
            continue;
        }
        if c == '/' && bytes.get(i + 1) == Some(&'*') {
            place = Where::BlockComment;
            comment_depth = 1;
            i += 2;
            continue;
        }
        // A raw string, and it has to be its own case: this repository writes its JSON fixtures as
        // `r#"…"#`, and JSON is full of quotes. Treating one as an ordinary string ends it at the
        // first quote **inside** the JSON and hands the rest of the fixture over as if it were code.
        if c == 'r' && matches!(bytes.get(i + 1), Some('#') | Some('"')) {
            let mut hashes = 0usize;
            while bytes.get(i + 1 + hashes) == Some(&'#') {
                hashes += 1;
            }
            if bytes.get(i + 1 + hashes) == Some(&'"') {
                let mut j = i + 2 + hashes;
                while j < bytes.len() {
                    if bytes[j] == '"' && (0..hashes).all(|k| bytes.get(j + 1 + k) == Some(&'#')) {
                        break;
                    }
                    if bytes[j] == '\n' {
                        line += 1;
                    }
                    j += 1;
                }
                i = j + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            place = Where::Text;
            i += 1;
            continue;
        }
        // A character literal, told apart from a lifetime by what closes it. Without this the CJK
        // load and the wide-character assertions read as identifiers, and the not-even-ASCII half of
        // the guard fires on the test data it was never meant to judge.
        if c == '\'' {
            let escaped = bytes.get(i + 1) == Some(&'\\');
            let closes = if escaped { None } else { bytes.get(i + 2) };
            if escaped || closes == Some(&'\'') {
                let mut j = i + 1;
                while j < bytes.len() {
                    if bytes[j] == '\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == '\'' {
                        break;
                    }
                    j += 1;
                }
                i = j + 1;
                continue;
            }
        }

        if c == '#' {
            let ahead: String = bytes[i..bytes.len().min(i + 16)].iter().collect();
            if ahead.starts_with("#[test]") || ahead.starts_with("#[should_panic") {
                inside_a_test_attribute = true;
            }
            i += 1;
            continue;
        }

        if c.is_alphanumeric() || c == '_' {
            if word.is_empty() {
                word_line = line;
            }
            word.push(c);
            i += 1;
            continue;
        }

        if !word.is_empty() {
            let done = std::mem::take(&mut word);
            if previous_word == "fn" && inside_a_test_attribute {
                inside_a_test_attribute = false;
            } else {
                found.push((word_line, done.clone()));
            }
            previous_word = done;
        }
        i += 1;
    }
    if !word.is_empty() {
        found.push((word_line, word));
    }
    found
}

/// `mouse_reporting` → `mouse`, `reporting`; `CombiningSpan` → `combining`, `span`.
fn segments(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for piece in name.split('_') {
        let mut current = String::new();
        for c in piece.chars() {
            if c.is_uppercase() && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current.extend(c.to_lowercase());
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

/// The `.rs` files under a directory, and **it panics if it cannot read it**.
///
/// Returning an empty list was a silence of the expensive kind: a misspelled name in the list of
/// finished crates left this guard with no file to look at, and a crate with no files reads exactly
/// like a clean one. Measured: turning one hyphen into an underscore in a crate name left all four
/// tests green.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|failure| {
        panic!(
            "cannot read {}: {failure}. If that is a name from the finished-crates list, it is \
             misspelled — and without this panic the guard would call clean a crate it never read",
            dir.display()
        )
    });
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

/// The extractor is still finding the glossary — **counted per shape, not in total**.
///
/// A floor over the sum is the disguise this project has on file: a number with which the defect and
/// the fix give the same answer. Measured — knocking both tables out of shape drops the total from 112
/// to 104, and a floor of 100 waves it through while eight names quietly leave the vocabulary.
#[test]
fn the_glossary_is_still_where_this_test_reads_it_from() {
    let glossary = glossary();
    assert!(
        glossary.from_arrows >= ARROWS_AT_LEAST,
        "the arrow shape `Spanish`→`English` yielded {} pairs and there are at least \
         {ARROWS_AT_LEAST}: the annex writes them some other way now, and this guard is running on a \
         vocabulary it no longer has",
        glossary.from_arrows
    );
    assert!(
        glossary.from_tables >= TABLES_AT_LEAST,
        "the two tables with a «Queda» column yielded {} rows and there are at least \
         {TABLES_AT_LEAST}: their eight names are the ones no arrow in the annex carries, so losing \
         them is invisible in any total",
        glossary.from_tables
    );
}

/// Why this one identifier is a leftover, or `None` if it is not one.
///
/// Split out from the walk so the rules can be asked directly, without a crate to point it at: some
/// of the answers below are only ever reached by names that do not exist in the tree yet, and a rule
/// with no oracle is the same thing as no rule.
fn why_it_is_a_leftover(name: &str, glossary: &Glossary) -> Option<String> {
    // An identifier with a letter outside ASCII cannot be English. It is the cheapest half of this
    // guard and it needs no glossary at all.
    if name.chars().any(|c| c.is_alphabetic() && !c.is_ascii()) {
        return Some(format!("`{name}` is not even ASCII"));
    }
    let pieces = segments(name);

    // **The compound rule, and it was the biggest hole in the first version.** The glossary stores
    // `RatonReclamado` as one token, `ratonreclamado`; the identifier splits into `raton` + `reclamado`
    // and neither piece is in the set, so the two never met. Eighteen of the twenty compound names
    // survived **by luck**, because they happen to carry a particle the function words catch
    // (`DeLaBarra`, `RestoDelFotograma`, `EnElAtlas`…); the two without one — `RatonReclamado` and
    // `EnPuntos` — walked straight through a crate declared finished. Measured.
    //
    // Gluing the pieces back together is what fixes it, and it is done this way rather than by adding
    // each piece to the Spanish set: `CosteDelColor` would have put `color` in there, and `color` is
    // an English identifier this repository uses everywhere.
    // No English exemption here, unlike the one below: no name the glossary adopts has a glued form
    // that collides with a Spanish one, and removing the exemption broke nothing — so it was a
    // mechanism with no scenario. If one ever shows up, this test will say so loudly.
    let joined = fold(&pieces.join(""));
    if glossary.spanish.contains(&joined) {
        return Some(format!("`{name}` is a compound the glossary retired"));
    }

    // A single-piece name cannot be a compound, and there a lone function word is almost always
    // English — the `la` of a matrix, the `el` of an index. The check only applies where it means
    // something: inside a compound.
    let compound = pieces.len() > 1;
    for piece in &pieces {
        let segment = fold(piece);
        if glossary.english.contains(&segment) {
            continue;
        }
        if compound && FUNCTION_WORDS.contains(&segment.as_str()) {
            return Some(format!("`{name}` is built with the Spanish «{piece}»"));
        }
        if segment.len() <= 2 {
            continue;
        }
        // **Both plural endings, and the trailing digits.** Trying `es` and giving up left `banderas`
        // uncaught; trying `s` alone left `divisiones` uncaught; and not stripping digits left `celda2`
        // uncaught while `celda_2` fell — Spanish uses both endings and this repository numbers its
        // fixtures, so all of them are asked. The order used to matter and that was a bug: with `es`
        // first, `Series` became `seri` and never landed on `Serie`, which made the English exemption
        // above look load-bearing when in fact nothing reached it.
        let bare = segment.trim_end_matches(|c: char| c.is_ascii_digit());
        let forms = [
            Some(segment.as_str()),
            segment.strip_suffix('s'),
            segment.strip_suffix("es"),
            (bare != segment.as_str()).then_some(bare),
        ];
        if forms
            .iter()
            .flatten()
            .any(|form| glossary.spanish.contains(*form))
        {
            return Some(format!("`{name}` still carries «{piece}»"));
        }
    }
    None
}

/// The three rules, asked directly, with the cases the tree cannot supply.
///
/// **The English exemption exists for names that have not arrived yet**, and without this test it
/// would be code with nothing judging it: removing it leaves every crate green today. It is needed
/// the moment the measurement module is translated — de-pluralising `Series` gives `serie`, and
/// `Serie` is exactly the name it replaced.
#[test]
fn the_three_rules_answer_what_they_promise() {
    let glossary = glossary();
    let verdict = |name: &str| why_it_is_a_leftover(name, &glossary);

    // Retired vocabulary, as a whole name and as a piece of a compound.
    assert!(verdict("Celda").is_some(), "a retired type name");
    assert!(
        verdict("celda_de_arriba").is_some(),
        "a retired name inside a compound"
    );
    assert!(
        verdict("banderas").is_some(),
        "a retired name in the plural"
    );
    // Spanish grammar, which the glossary never had to name.
    assert!(verdict("base_de").is_some(), "a compound built with «de»");
    assert!(verdict("lo_quiere").is_some(), "a compound built with «lo»");
    // Anything outside ASCII.
    assert!(
        verdict("que_pestaña").is_some(),
        "a name with a letter outside ASCII"
    );
    // **The compound names, which the glossary stores glued together.** Two of the twenty carry no
    // Spanish particle at all, so the function words above cannot see them either: these two are the
    // whole reason the pieces get glued back before asking.
    assert!(
        verdict("RatonReclamado").is_some(),
        "a compound with no Spanish particle in it"
    );
    assert!(
        verdict("en_puntos").is_some(),
        "the other one, in snake_case"
    );
    // The tilde the code drops on purpose, and the digit a fixture adds.
    assert!(
        verdict("tamano_pedido").is_some(),
        "a compound the glossary writes with a tilde and the code writes without one"
    );
    assert!(verdict("pestanas").is_some(), "the same, in the plural");
    assert!(
        verdict("celda2").is_some(),
        "a retired name with a digit stuck on the end"
    );
    // The three names that live only in the table with a «Queda» column, which no arrow carries.
    assert!(
        verdict("reparto").is_some(),
        "a name only the Layout table names"
    );
    assert!(verdict("maqueta").is_some(), "another one");
    assert!(
        verdict("Resultado").is_some(),
        "a name only the calque table names"
    );

    // And what must NOT be reported.
    assert_eq!(
        verdict("Series"),
        None,
        "«Series» is the English name the glossary chose, and de-pluralising it lands on «Serie», \
         the very name it replaced: without the English exemption this guard would refuse the \
         rename it is supposed to be enforcing"
    );
    assert_eq!(
        verdict("Timings"),
        None,
        "another English glossary name in the plural"
    );
    assert_eq!(
        verdict("el"),
        None,
        "a lone function word is not a compound: `el` on its own is an English identifier far more \
         often than a Spanish article"
    );
    assert_eq!(
        verdict("panel_vacio"),
        None,
        "the declared blind spot, kept honest by asserting it"
    );
    // The three names the calque table adopts. They used to pass by luck: the extractor was taking the
    // **discarded** name out of that table, so `Outcome`, `PtyOutput` and `MouseReport` were never in
    // the English set at all and only survived because no segment of theirs collided.
    for adopted in [
        "Outcome",
        "PtyOutput",
        "MouseReport",
        "Pickup",
        "Tiling",
        "CombiningSpan",
    ] {
        assert_eq!(
            verdict(adopted),
            None,
            "«{adopted}» is the name the glossary adopts, and it has to be exempt by the English \
             side and not by chance"
        );
    }
    // **Gluing the pieces must not have widened the Spanish side piece by piece.** Adding each piece
    // of every compound would have put `color` in there, out of `CosteDelColor` — and `color` is an
    // English identifier this repository uses everywhere.
    for english_word in ["color", "colors", "cost", "atlas", "points", "size"] {
        assert_eq!(
            verdict(english_word),
            None,
            "«{english_word}» is a piece of a Spanish compound in the glossary and an English word \
             on its own: gluing has to be by the whole name, never piece by piece"
        );
    }
    // And the same name **as a Spanish compound** is still caught, so the rule above is not a hole.
    assert!(
        verdict("color_del_reclamo").is_some(),
        "a Spanish compound whose first piece happens to be an English word"
    );
}

#[test]
fn no_spanish_identifier_survives_in_a_finished_crate() {
    let glossary = glossary();
    let root = repo();
    let mut leftovers = Vec::new();

    for crate_name in FINISHED_CRATES {
        let files = rust_files(&root.join("crates").join(crate_name));
        assert!(
            !files.is_empty(),
            "«{crate_name}» is declared finished and has not a single `.rs` file to look at: either \
             the crate is gone, or its name on the list is misspelled. Either way this guard would be \
             calling clean something it never read"
        );
        for file in files {
            let shown = file
                .strip_prefix(&root)
                .unwrap_or(&file)
                .display()
                .to_string();
            let src = std::fs::read_to_string(&file).expect("a file the walk just listed");

            for (line, name) in identifiers(&src) {
                if let Some(why) = why_it_is_a_leftover(&name, &glossary) {
                    leftovers.push(format!("  {shown}:{line}: {why}"));
                }
            }
        }
    }

    assert!(
        leftovers.is_empty(),
        "{} identifier(s) in crates this milestone has already finished are still using a name the \
         glossary retired. Either the rename was left half done, or the glossary changed its mind \
         and nobody told the code:\n{}",
        leftovers.len(),
        leftovers.join("\n")
    );
}

#[test]
fn no_file_of_a_finished_crate_is_still_named_in_spanish() {
    let spanish = glossary().spanish;
    let root = repo();
    let mut leftovers = Vec::new();

    for crate_name in FINISHED_CRATES {
        let files = rust_files(&root.join("crates").join(crate_name));
        assert!(!files.is_empty(), "«{crate_name}» has no files to look at");
        for file in files {
            let name = file
                .file_name()
                .expect("a path the walk built from a directory entry")
                .to_string_lossy()
                .to_string();
            if spanish.contains(&fold(&name)) {
                let shown = file.strip_prefix(&root).unwrap_or(&file).display();
                leftovers.push(format!("  {shown}"));
            }
        }
    }

    assert!(
        leftovers.is_empty(),
        "{} file(s) of a finished crate still carry the Spanish name the glossary retired, and the \
         glossary lists the name they should have:\n{}",
        leftovers.len(),
        leftovers.join("\n")
    );
}

/// The scanner, over the battery of shapes Rust allows and this repository writes.
///
/// **It exists because half of its branches are not touched by any file in today's tree**, and a
/// branch with no oracle is a branch nobody knows works: nested-comment depth was exercised by
/// nothing — removing it left the other four tests green — and its defect still bites both ways, with
/// a false positive that breaks the build and a false negative that switches a name off.
#[test]
fn the_scanner_tells_code_from_prose() {
    let source = r####"
// la celda de arriba
/// la celda de la documentación
/* la celda del bloque */
/* uno /* dos */ la celda de dentro */
/* uno /* dos */ #[test] */
fn celda_escondida() {}
fn f<'a>(_x: &'a str) -> &'a str {
    let _ = "la celda de la cadena";
    let _ = r#"{"celda": 1}"#;
    let _ = b"la celda en bytes";
    let _ = 'ñ';
    let _ = "// celda";
    let _ = "/* celda */";
    let _ = "una \" celda";
    let partida = "una linea \
                   y su continuacion";
    partida
}
#[test]
fn nombre_de_prueba_en_espanol() {}
#[test]
#[ignore]
fn otro_nombre_de_prueba() {}
fn ayudante_espanol() {}
"####;

    let scanned = identifiers(source);
    let found: Vec<&str> = scanned.iter().map(|(_, n)| n.as_str()).collect();

    // What **is** code and has to come out.
    for expected in ["celda_escondida", "f", "partida", "ayudante_espanol"] {
        assert!(
            found.contains(&expected),
            "`{expected}` is code and the scanner did not see it: {found:?}"
        );
    }
    // What is prose or a literal and must **not** come out. `celda` appears only inside comments and
    // strings, so seeing it means one of the six shapes has thrown the scanner off.
    assert!(
        !found.contains(&"celda"),
        "the scanner read prose or a literal as code: {found:?}"
    );
    // And the test names, which are another delivery's prose.
    for skipped in ["nombre_de_prueba_en_espanol", "otro_nombre_de_prueba"] {
        assert!(
            !found.contains(&skipped),
            "`{skipped}` is a test name and has to be skipped: {found:?}"
        );
    }

    // And the line it reports, which is what whoever fixes it reads. `ayudante_espanol` sits after a
    // string continuation, which is exactly what used to make the number drift.
    let line_of = |name: &str| {
        scanned
            .iter()
            .find(|(_, n)| n == name)
            .map(|(l, _)| *l)
            .unwrap_or_else(|| panic!("`{name}` is missing"))
    };
    let real = |name: &str| {
        source
            .lines()
            .position(|l| l.contains(&format!("fn {name}")))
            .expect("the line exists")
            + 1
    };
    for name in ["celda_escondida", "ayudante_espanol"] {
        assert_eq!(
            line_of(name),
            real(name),
            "the line reported for `{name}` is not its own"
        );
    }
}
