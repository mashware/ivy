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

/// The crates whose identifiers are **all English**, under `crates/`. One entry per delivery.
const FINISHED_CRATES: &[&str] = &["panel-contract", "host-load", "term-core"];

/// Single **files** whose identifiers are all English, for a crate that is not finished yet.
///
/// **This came in once, was deleted, and is back — and the round trip is the point.** It was built to
/// point the guard at `app/src/lib.rs`, and pointing it there proved that file could not be declared
/// finished: it still holds hundreds of Spanish names the glossary never had to decide. With nothing
/// left to name, a branch of the walk became reachable by nobody, so it went out — this project deletes
/// a mechanism with no scenario. It returns with the delivery that gives it one: five modules of `app`
/// that really are finished while their siblings are not.
///
/// A crate whose every file ends up here would be better spelled in [`FINISHED_CRATES`]; nothing
/// enforces that, and it does not matter much — the two lists cannot disagree about a file, they can
/// only both be right about it.
const FINISHED_FILES: &[&str] = &[
    "app/src/mouse.rs",
    "app/src/keyboard.rs",
    "app/src/painter.rs",
    "app/src/fonts.rs",
    "app/src/panel.rs",
];

/// Files whose **citations** are checked, though their identifiers are not yet.
///
/// **Every file of the crate, and that is what closes the class.** The first version listed only the
/// file the delivery was renaming, which left the other direction unguarded: nobody checked what a
/// *sibling* cites **towards** the renamed file. Four citations shipped broken that way — two in
/// `sesion.rs` sitting two and three lines from citations the same delivery did update, one in
/// `medida.rs`, and one in an example whose comment exists precisely to say «copied from
/// `app::PUMP_BUDGET`; if it changes there, this stops measuring the same thing». It changed there.
///
/// The price is thirty entries in [`NOT_OURS`], because a sibling's comments cite font tables, shell
/// commands and Unicode property names. They are listed once and they stay true.
///
/// **This is a weaker claim than [`FINISHED_CRATES`] and it is a separate list so it cannot be read as the
/// same one.** `app/src/lib.rs` has had its window family renamed and its own locals with it, but it
/// still holds hundreds of Spanish names the glossary never had to decide — the `desde` of a range,
/// the `cual` of an index — and a later delivery takes them. Calling it finished would be a claim the
/// guard cannot back.
///
/// What *can* be claimed today is the one thing renaming breaks and nothing else notices: **a comment
/// that still cites a name the code no longer has**. The compiler does not read comments, the suite
/// does not either, and a reader who greps for the name a comment gives them finds nothing. This
/// delivery renamed a hundred and thirty names in that file; without this, a hundred and fifty-eight
/// comments would have been left pointing at names that no longer exist.
///
/// The second half of each entry is the crate whose names the file may cite: a comment in `lib.rs` is
/// entitled to name something its sibling modules declare.
const CITATIONS_CHECKED: &[(&str, &str)] = &[
    ("app/src/lib.rs", "app"),
    ("app/src/settings.rs", "app"),
    ("app/src/fonts.rs", "app"),
    ("app/src/measure.rs", "app"),
    ("app/src/panel.rs", "app"),
    ("app/src/painter.rs", "app"),
    ("app/src/pty_output.rs", "app"),
    ("app/src/mouse.rs", "app"),
    ("app/src/session.rs", "app"),
    ("app/src/keyboard.rs", "app"),
    ("app/src/theme.rs", "app"),
    ("app/src/watcher.rs", "app"),
];

/// Names that are **still Spanish on purpose**, because a later delivery owns them.
///
/// Every entry is a name a finished file still carries, with the delivery that takes it. They are
/// listed one by one instead of being matched by a rule: **a short explicit list ages better than a
/// criterion that widens until it stops failing**, and every line here is a line somebody has to
/// delete, which is the point.
///
/// The seven at the top are the types whose field and variant names **are the layout file on disk**:
/// they move with the measurement bench and the version bump, so that renaming the format and
/// reseeding the script that writes it happen in one commit. The rest are types a finished file
/// legitimately names because a file this milestone has not reached yet declares them.
///
/// **Every entry is checked against the file that declares the name, not against the intention of
/// whoever added it.** Three entries have already been deleted for failing that check: two named
/// types declared in the very file the delivery was renaming — so they hid its own work instead of
/// somebody else's — and one, `MEDIR`, matched no identifier at all.
///
/// **And this list exempts nothing today. Measured:** make the exemption below never fire and
/// [`no_spanish_identifier_survives_in_a_finished_crate`] stays green, because the three finished
/// crates have no name that needs exempting — they are finished. Its user is the delivery that puts
/// `app/src/lib.rs` in [`FINISHED_CRATES`], and it is written now because that is when it was
/// **derived**: pointing the guard at that file printed these names one by one, each verified against
/// the file that declares it. Saying so here is the point — a list that quietly exempted nothing
/// would read, to the next person, as a list that was doing work.
const STILL_SPANISH: &[&str] = &[
    // The seven whose field and variant names **are** the layout file on disk.
    "Hoja",
    "Orientacion",
    "Nodo",
    "Hijo",
    "Pestaña",
    "Documento",
    "Sonda",
    // Types a finished file legitimately names because they are declared in a file that is not
    // finished. **Every one of these is a line a later delivery deletes**, which is why they are
    // written out one by one: the list shrinking is how the milestone shows its progress.
    "Ajustes",
    "Arrastres",
    "Avisos",
    "Bandera",
    "Barra",
    "DeLaSesion",
    "Descubrimiento",
    "EnPuntos",
    "Guardado",
    "LecturaDeAjustes",
    "Medida",
    "Metrica",
    "Persistencia",
    "Puente",
    "Rasterizado",
    "Reflujo",
    "Restauracion",
    "RestoDelFotograma",
    "Tema",
    "tema",
    "con_atributos",
    "color_del_reclamo",
    "color_del_panel",
    "fondo_del_panel",
    "Tramos",
    "Valido",
    "Vigilante",
];

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

/// Spanish accents folded onto ASCII, **with the case left alone**.
///
/// **The exemption list is matched with this and not with [`fold`], and the difference is what keeps
/// the list from blinding the guard.** A type a later delivery owns folds, lowercased, onto the very
/// name a file gives its locals: `Gesto` and `gesto`, `Hueco` and `hueco`, `Nodo` and `nodo`. Listing
/// the type would exempt the local too — and the local is the one thing this guard exists to catch.
///
/// **Said plainly: today this changes no verdict.** None of the three finished crates has a local
/// named after a listed type, so lowercasing and not lowercasing give the same answer for every file
/// the guard currently reads. It was found while renaming the crate that comes next, where seventeen
/// names did land on an entry that was never about them, and it is one line — so it goes in now
/// rather than after the delivery it would have bitten.
///
/// Rust settles it for free: types are `CamelCase`, locals and fields are `snake_case`, so keeping
/// the case makes the two unable to collide in the first place.
fn unaccent(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            'Á' => 'A',
            'É' => 'E',
            'Í' => 'I',
            'Ó' => 'O',
            'Ú' | 'Ü' => 'U',
            'Ñ' => 'N',
            other => other,
        })
        .collect()
}

/// Lowercase, and Spanish accents folded onto ASCII.
///
/// **Both sides get folded, and that is what closes a whole family of leftovers.** The glossary
/// writes `Pestaña` and `TamañoPedido`; the code writes `pestanas` and `tamano_pedido`, because
/// identifiers in this repository avoid the tilde on purpose. Without folding, the two never meet —
/// measured, with `TAMANO`, which is a real constant of the measurement script.
fn fold(text: &str) -> String {
    unaccent(text).to_lowercase()
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

/// The `.rs` files under a path, which may be a directory or a single file.
///
/// **It panics if it cannot read it.**
///
/// Returning an empty list was a silence of the expensive kind: a misspelled name in the list of
/// finished crates left this guard with no file to look at, and a crate with no files reads exactly
/// like a clean one. Measured: turning one hyphen into an underscore in a crate name left all four
/// tests green.
/// Everything this milestone has declared finished, whole crates and single files alike.
///
/// **The two lists are walked by one function so they cannot drift apart.** Adding a guard that reads
/// only one of them is the shape of hole this file keeps finding: a list that looks watched and is not.
/// The label comes back with the files so a failure can name what was declared, not just the path.
fn finished_targets(root: &Path) -> Vec<(String, Vec<PathBuf>)> {
    let mut out = Vec::new();
    for crate_name in FINISHED_CRATES {
        out.push((
            (*crate_name).to_string(),
            rust_files(&root.join("crates").join(crate_name)),
        ));
    }
    for path in FINISHED_FILES {
        let file = root.join("crates").join(path);
        out.push((
            (*path).to_string(),
            if file.is_file() {
                vec![file]
            } else {
                Vec::new()
            },
        ));
    }
    out
}

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
    // **Still Spanish on purpose, matched by the WHOLE name and with the CASE KEPT.** Matching by
    // segment would have exempted every identifier carrying one of these pieces — a local called
    // `pestanas` as much as the stored type `Pestaña` — and the local is precisely what has to be
    // caught. Case matters for the same reason and it cost a pass to see: lowercased, the entry for
    // somebody else's `Gesto` would also exempt a file's own `gesto`. See [`unaccent`].
    //
    // **A prefix form used to be accepted here and it exempted nothing.** It was put in for the
    // measurement gate's variables, on the belief that they are identifiers starting with `MEDIR_`;
    // they are not — those are the environment variables' **strings**, and the constants that hold
    // them are called `VARIABLE_DE_LAS_PESTAÑAS` and the like. So the entry matched no name, the
    // prefix matched no entry, and both are gone. Whole name or nothing.
    let whole = unaccent(name);
    if STILL_SPANISH.iter().any(|kept| whole == unaccent(kept)) {
        return None;
    }

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
    //
    // **The name here must not be one the exemption list carries**, and that is not hypothetical: this
    // case used to be `color_del_reclamo`, and the delivery that finished the panel put that very name
    // on the list — because the theme still declares it — which silenced the case without touching the
    // rule. A case that an exemption can switch off is a case that stops testing while staying green.
    assert!(
        verdict("color_del_borde").is_some(),
        "a Spanish compound whose first piece happens to be an English word"
    );

    // **The exemption list keeps the case, and this is the pair that forced it.** `Gesto` is on the
    // list because a file this milestone has not reached yet declares it; `gesto` is what a finished
    // file called the local that holds one. Lowercased they are the same string, so the entry for the
    // type exempted the local — and the local is the whole reason this guard runs. Same shape for
    // `Nodo`/`nodo` and `Hueco`/`hueco`, which is seventeen names in the file this delivery finishes.
    for kept in STILL_SPANISH {
        assert_eq!(
            verdict(kept),
            None,
            "«{kept}» is on the exemption list and has to come out exempt"
        );
    }
    for own in ["gesto", "nodo", "hueco", "orientacion"] {
        assert!(
            verdict(own).is_some(),
            "«{own}» is what a finished file calls its own local, and the exemption list carries the \
             CamelCase type of the same word: lowercasing the comparison would let this one through"
        );
    }
}

#[test]
fn no_spanish_identifier_survives_in_a_finished_crate() {
    let glossary = glossary();
    let root = repo();
    let mut leftovers = Vec::new();

    for (what, files) in finished_targets(&root) {
        assert!(
            !files.is_empty(),
            "«{what}» is declared finished and has not a single `.rs` file to look at: either it is \
             gone, or its name on the list is misspelled. Either way this guard would be calling clean \
             something it never read"
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

    for (what, files) in finished_targets(&root) {
        assert!(!files.is_empty(), "«{what}» has no files to look at");
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

/// Names a comment may cite that are **not** identifiers of the crate and never will be.
///
/// This one list is copied rather than derived, and there is no source to derive it from: a program
/// the prose names (`vim`), a constant of the system (`OPOST`), an item of a dependency
/// (`LineWriter`) and a method of `std` (`recv`) are all outside every crate in this tree. Keeping
/// it closed is what makes the guard below safe to break the build with — and the failure message
/// says so, because the fix for a genuine foreign name is to add it here, not to widen the rule.
const NOT_OURS: &[&str] = &[
    // Other programs, and what the operating system gives them.
    "bash",
    "cat",
    "clear",
    "emacs",
    "exec",
    "exit",
    "fork",
    "htop",
    "ioctl",
    "less",
    "ls",
    "man",
    "nano",
    "nvim",
    "O_APPEND",
    "OPOST",
    "printf",
    "sh",
    "strace",
    "stty",
    "tmux",
    "vim",
    "xterm",
    // Environment variables — ours and other people's. A variable is not an identifier of this tree,
    // and the ones the measurement script exports leave with the script, in a later delivery.
    "MEDIR_MIB",
    "MEDIR_TANDA",
    "PANEL_DIR",
    "PANEL_ID",
    "PWD",
    "SHELL",
    "SHLVL",
    "TAMANO",
    "TERM",
    // Items of the crates this one depends on.
    "click",
    "click_and_drag",
    "CLICK_AND_DRAG",
    "content_rect",
    "distance_sq_to_pos",
    "dragged",
    "FOCUSABLE",
    "inner_size",
    "InputState",
    "interact_radius",
    "LineWriter",
    "NaN",
    "NonZeroUsize",
    "on_hover_cursor",
    "or_else",
    "portable_pty",
    "prepare_raw_input",
    "recv",
    "serde",
    "set_var",
    "vars_os",
    "with",
    // Values, not names: what a key is called in the settings file, what the panel contract admits
    // for its `attention` field, what the measurement variable accepts, and what a report line is
    // labelled. Every one of them is a string, so no set of identifiers can hold it — and every one
    // is translated with the file that defines it, not with this one.
    "block",
    "Ctrl",
    "Fin",
    "frontera",
    "Inicio",
    "ninguno",
    "restauradas",
    "stale_after_minutes",
    "wait",
    // Single letters standing for a quantity in prose.
    "f",
    "w",
    // ── What the sibling files cite, now that their citations are checked too ──
    // The standard library and the language.
    "fsync",
    "read",
    // Other programs, and the shell.
    "echo",
    "grep",
    "sed",
    "VTE",
    // Items of the crates this one depends on.
    "break_on_newline",
    "Fonts",
    "from_name",
    "has_glyph",
    "has_glyphs",
    "NEAREST",
    "Single",
    // Font tables and font families, which belong to whoever made the font.
    "cmap",
    "COLR",
    "Hack",
    "Noto",
    // Unicode property names, which belong to Unicode.
    "Emoji_Component",
    "EmojiModifierBase",
    "EmojiOtherAndEmojiComponent",
    "Emoji_Presentation",
    // Another environment variable.
    "HOSTNAME",
    // More values that are not names: keys of the settings file, values of the panel contract, and
    // what a key is called when somebody types it.
    "Alt",
    "f4",
    "pageup",
    "pending",
    "prior",
    "supr",
    "waiting",
    // **A name that never existed, and the argument needs it.** The measurement bench's naming rule
    // is stated by counter-example — «a type called `Configuracion` inside the measurement module
    // would read as *the configuration* instead of as *what it cost to read it*» — so the citation is
    // of something deliberately absent. Nothing else in this list is of that kind.
    "Configuracion",
];

/// Every identifier a pair of backticks cites — **all the segments of a path, not just the first**.
///
/// Shapes: `` `name` ``, `` `name()` ``, `` `name: Type` `` (a field with its type, which is how this
/// repository writes a measured layout) and `` `Type::member` `` with any number of segments.
///
/// **The qualified form used to be skipped, and the reason written here for skipping it was false.**
/// It said `` `Panel::written_at` `` is «a rustdoc link that `cargo doc` already judges». It is not:
/// rustdoc resolves links inside **brackets** — `[`Panel::written_at`]` — and a bare pair of
/// backticks is code formatting, nothing more. Worse, most of these live in `//` comments, which
/// rustdoc does not read at all. Fourteen citations naming methods and types this milestone had
/// already renamed were sitting behind that sentence with the suite green, two of them left by the
/// delivery that wrote the sentence. Measured, and the lesson is the one this project keeps
/// relearning: a plausible written justification is the cheapest place for a hole to hide.
///
/// So every segment is asked about. A bracketed rustdoc link passes anyway — its segments all exist,
/// or `cargo doc-check` was already failing — and the overlap costs nothing.
fn cited_names(inside: &str) -> Vec<&str> {
    let inside = inside.strip_suffix("()").unwrap_or(inside);
    // A single colon separates a field from its type; a double one separates path segments.
    let path = match inside.split_once(':') {
        Some((_, rest)) if rest.starts_with(':') => inside,
        Some((before, _)) => before.trim_end(),
        None => inside,
    };
    let segments: Vec<&str> = path.split("::").collect();
    let an_identifier = |name: &str| {
        let mut chars = name.chars();
        chars.next().is_some_and(|first| {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    // **All or nothing.** `panel.json` splits into one segment that is not an identifier, and so does
    // `cargo test`; reporting the good half of something that is not a citation at all would be a
    // false alarm, and a guard that breaks the build has to be wrong less often than the code.
    if segments.iter().all(|s| an_identifier(s)) {
        segments
    } else {
        Vec::new()
    }
}

/// Every identifier a crate's code actually defines or uses, comments and literals excluded.
fn code_names(files: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for file in files {
        let src = std::fs::read_to_string(file).expect("a file the walk just listed");
        names.extend(identifiers(&src).into_iter().map(|(_, name)| name));
        // The scanner drops the name of the function under a `#[test]`, which is exactly right when
        // it is hunting Spanish and exactly wrong here: a comment is allowed to cite a test by name.
        for line in src.lines() {
            if let Some((_, rest)) = line.split_once("fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
    }
    names
}

/// A comment that cites an identifier by a name the code no longer has.
///
/// **This is the half of a rename that nothing else in the tree can see.** The compiler checks the
/// code, `cargo doc` checks the `[`link`]` form, and this delivery's own rule says a reference in a
/// comment is not prose and gets edited with the code — but a reference written `` `like_this` ``
/// has no oracle at all, so the rule was kept by hand. Twenty-one of them were not.
///
/// It is scoped per crate and not repo-wide on purpose: `desde`, `parar` and `prioridad` are all
/// live identifiers **in `app`**, so a union of every crate's names would call each of these
/// citations fine and report nothing.
#[test]
fn no_comment_cites_a_name_the_code_no_longer_has() {
    let root = repo();
    let mut stale = Vec::new();

    // Two kinds of entry, and the difference is **which files supply the names**: a finished crate
    // answers for itself, while a single file answers with its whole crate behind it, because a
    // comment there is entitled to cite what a sibling module declares.
    let mut jobs: Vec<(Vec<PathBuf>, Vec<PathBuf>)> = Vec::new();
    for crate_name in FINISHED_CRATES {
        let files = rust_files(&root.join("crates").join(crate_name));
        assert!(
            !files.is_empty(),
            "«{crate_name}» is declared finished and has no file to look at: its name on the list \
             is misspelled, and without this the guard would report a crate it never read as clean"
        );
        jobs.push((files.clone(), files));
    }
    for (path, crate_name) in CITATIONS_CHECKED {
        let file = root.join("crates").join(path);
        assert!(
            file.is_file(),
            "«{path}» is on the citations list and is not a file: the name is misspelled, and \
             without this the guard would report a file it never read as clean"
        );
        jobs.push((
            vec![file],
            rust_files(&root.join("crates").join(crate_name)),
        ));
    }

    for (to_read, for_names) in jobs {
        let files = to_read;
        let names = code_names(&for_names);
        for file in &files {
            let shown = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let src = std::fs::read_to_string(file).expect("a file the walk just listed");
            for (number, line) in src.lines().enumerate() {
                if !line.trim_start().starts_with("//") {
                    continue;
                }
                for piece in line.split('`').skip(1).step_by(2) {
                    for cited in cited_names(piece) {
                        if names.contains(cited) || NOT_OURS.contains(&cited) {
                            continue;
                        }
                        stale.push(format!("  {shown}:{}: `{cited}`", number + 1));
                    }
                }
            }
        }
    }

    assert!(
        stale.is_empty(),
        "{} comment(s) cite an identifier the crate around them does not have. A rename \
         does not break a comment, it makes it lie — and a reader who greps for the name it gives \
         finds nothing. Either the citation has to move to the new name, or the name belongs to \
         something outside this tree and goes in `NOT_OURS`:\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// The reader of citations, over the shapes that decide whether one is even asked about.
///
/// **Half of these are exemptions, and an exemption with no test is what lets a guard go quiet.**
/// The double colon is the one that matters: this repository writes hundreds of
/// `[`Type::method`]` links and every one of them would be reported as a stale citation.
#[test]
fn the_reader_of_citations_asks_about_the_right_things() {
    assert_eq!(cited_names("cuantos"), vec!["cuantos"], "a bare name");
    assert_eq!(cited_names("lo_quiere()"), vec!["lo_quiere"], "a call");
    assert_eq!(
        cited_names("desde: u32"),
        vec!["desde"],
        "a field with its type, which is how this repository writes a measured layout"
    );
    // **The qualified form, which used to come back empty behind a false reason.** Both halves are
    // asked about, because both can die: `Ventana::pintar_division` was two dead names at once.
    assert_eq!(
        cited_names("Panel::written_at"),
        vec!["Panel", "written_at"],
        "a qualified path is every one of its segments: a bare pair of backticks is not a rustdoc \
         link, so nothing else in the tree judges it"
    );
    assert_eq!(
        cited_names("session::Nodo::Rama"),
        vec!["session", "Nodo", "Rama"],
        "however many segments it has"
    );
    assert!(
        cited_names("panel.json").is_empty(),
        "a file name is not an identifier"
    );
    assert!(cited_names("cargo test").is_empty(), "nor is a command");
    assert!(
        cited_names("").is_empty(),
        "nor is an empty pair of backticks"
    );
    assert!(cited_names("--mib").is_empty(), "nor is a flag");
    // **All or nothing, and this is the case that forced it.** Splitting on `::` first would have
    // turned a version number into two «identifiers» and reported the half that looks like one.
    assert!(
        cited_names("egui::0.29").is_empty(),
        "a path with one segment that is not an identifier is not a citation at all"
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
