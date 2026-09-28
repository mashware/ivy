//! Reading a repository document the way the guards that confront it with the code need to.
//!
//! **Every guard that reads a document used to walk it by hand**, each with its own anchor and its own
//! idea of where a column is: one took the first backticked token of the row, one hardcoded the
//! second cell, one the first and the third. A table that gains a column in front breaks the ones that
//! count positions by reading the wrong cell, and the failure names the cell's content rather than
//! the table's shape. Here a column is found **by its header**, and a header that is not there says so.
//!
//! **Every shape it cannot read is a panic, never an empty answer**: a missing header, a body row
//! with the wrong number of cells, a table with no rows, a fence that never closes. An empty answer
//! is how a guard that reads another file passes by looking at nothing.
//!
//! It is declared inside `lib.rs`'s `mod tests`, which keeps that file's ratchet from growing; the
//! file itself is a module like any other and `shape` holds it to the same ceiling.

/// A Markdown table: its header cells and its body rows, each row split into trimmed cells.
///
/// It owns its cells rather than borrowing the document, and that is for `shape`: the type walk that
/// holds each type to its ceiling reads an `impl` header with no lifetime parameters.
pub(crate) struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// The first table in `doc` whose header row is exactly `header` once its cells are trimmed.
///
/// **Exactly and not «starts with»**: a header that is a prefix of another table's — `| Key |`
/// against `| Key | What it is |` — would otherwise read whichever table comes first. Two tables
/// with the same header in one document are told apart by the caller, which hands in the part of the
/// document that holds the one it means.
pub(crate) fn table(doc: &str, header: &[&str]) -> Table {
    let mut lines = doc.lines();
    let found = lines
        .by_ref()
        .find(|line| cells(line) == header)
        .unwrap_or_else(|| {
            panic!(
                "the document has no table whose header is {header:?}: it changed shape, and the guard \
                 reading it would be looking at nothing"
            )
        });
    let separator = cells(lines.next().unwrap_or_default());
    assert!(
        separator.len() == header.len()
            && separator
                .iter()
                .all(|cell| cell.len() >= 3 && cell.chars().all(|c| c == '-' || c == ':')),
        "the header {header:?} is not followed by a separator row, so it is not a table"
    );
    let owned =
        |line: &str| -> Vec<String> { cells(line).into_iter().map(str::to_owned).collect() };
    let rows: Vec<Vec<String>> = lines
        .take_while(|line| line.trim_start().starts_with('|'))
        .map(owned)
        .collect();
    assert!(!rows.is_empty(), "the table {header:?} has no rows");
    for row in &rows {
        assert_eq!(
            row.len(),
            header.len(),
            "a row of the table {header:?} has {} cells: {row:?}. A row that splits badly would read \
             its cells as empty",
            row.len()
        );
    }
    Table {
        header: owned(found),
        rows,
    }
}

impl Table {
    /// Every body row's cell under the column called `name`, in the order the rows are written.
    pub(crate) fn column(&self, name: &str) -> Vec<&str> {
        let at = self.position(name);
        self.rows.iter().map(|row| row[at].as_str()).collect()
    }

    /// The cell under `column` of the one row whose cell under `key_column` is `key`.
    ///
    /// `None` when no row carries that key. **Two rows carrying it is a panic**, because answering
    /// with the first would make the guard's verdict depend on the order of the rows.
    pub(crate) fn cell(&self, key_column: &str, key: &str, column: &str) -> Option<&str> {
        let (at_key, at) = (self.position(key_column), self.position(column));
        let mut found = self.rows.iter().filter(|row| row[at_key] == key);
        let row = found.next()?;
        assert!(
            found.next().is_none(),
            "two rows of the table carry «{key}» under «{key_column}»"
        );
        Some(row[at].as_str())
    }

    fn position(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|cell| *cell == name)
            .unwrap_or_else(|| panic!("the table has no column «{name}»: {:?}", self.header))
    }
}

/// The cells of a table row, trimmed, without the empty ones the outer pipes leave.
fn cells(line: &str) -> Vec<&str> {
    let Some(inner) = line.trim().strip_prefix('|') else {
        return Vec::new();
    };
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(str::trim).collect()
}

/// A document split by its fenced blocks, fence lines in neither half.
///
/// Two named halves rather than a pair, because the two are the same type and a caller that took
/// the wrong one would still compile — and would still find something to read.
pub(crate) struct Fenced<'a> {
    /// What is inside the blocks: what somebody copies.
    pub(crate) code: Vec<&'a str>,
    /// What is outside them: the prose.
    pub(crate) prose: Vec<&'a str>,
}

/// `doc` split by its fences. A fence is a line that starts with three backticks.
pub(crate) fn fenced(doc: &str) -> Fenced<'_> {
    let mut inside = false;
    let mut split = Fenced {
        code: Vec::new(),
        prose: Vec::new(),
    };
    for line in doc.lines() {
        if line.starts_with("```") {
            inside = !inside;
        } else if inside {
            split.code.push(line);
        } else {
            split.prose.push(line);
        }
    }
    assert!(
        !inside,
        "the document opens a fenced block it never closes, and everything after it would read as code"
    );
    split
}

#[test]
fn a_column_is_found_by_its_header_and_a_fence_splits_the_document() {
    const DOC: &str = "intro\n\
                       | Name | Chord |\n\
                       |---|:---:|\n\
                       | `a` | `ctrl+a` |\n\
                       | `b` | `ctrl+b` |\n\
                       after\n\
                       ```\n\
                       key = value\n\
                       ```\n\
                       | Chord | Name |\n\
                       |---|---|\n\
                       | `ctrl+z` | `z` |\n";
    let first = table(DOC, &["Name", "Chord"]);
    assert_eq!(first.column("Chord"), ["`ctrl+a`", "`ctrl+b`"]);
    assert_eq!(first.cell("Name", "`b`", "Chord"), Some("`ctrl+b`"));
    assert_eq!(first.cell("Name", "`c`", "Chord"), None);
    // The same headers in another order are another table, and the column follows its name.
    let second = table(DOC, &["Chord", "Name"]);
    assert_eq!(second.column("Name"), ["`z`"]);
    assert_eq!(second.cell("Chord", "`ctrl+z`", "Name"), Some("`z`"));

    let split = fenced(DOC);
    assert_eq!(split.code, ["key = value"]);
    assert!(split.prose.contains(&"after") && !split.prose.contains(&"key = value"));
}

#[test]
fn a_shape_the_reader_cannot_take_is_refused_rather_than_read_as_empty() {
    let refused = |doc: &'static str, reads: fn(&'static str)| {
        std::panic::catch_unwind(|| reads(doc)).is_err()
    };
    assert!(
        refused("| A | B |\n|---|---|\n| only one |\n", |doc| {
            table(doc, &["A", "B"]);
        }),
        "a row short of a cell"
    );
    assert!(
        refused("| A | B |\n|---|---|\n\nprose\n", |doc| {
            table(doc, &["A", "B"]);
        }),
        "a table with no rows"
    );
    assert!(
        refused("prose\n```\nnever closed\n", |doc| {
            fenced(doc);
        }),
        "a fence that never closes"
    );
}
