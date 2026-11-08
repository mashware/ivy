//! Parsed, read-only Git changes. Paths remain separate from their safe display labels.
use std::path::PathBuf;

#[derive(Debug, Default)]
pub(super) struct Document {
    pub(super) files: Vec<DiffFile>,
}
#[derive(Debug)]
pub(super) struct DiffFile {
    pub(super) path: PathBuf,
    pub(super) display: String,
    pub(super) hunks: Vec<Hunk>,
    pub(super) notice: Option<String>,
}
#[derive(Debug)]
pub(super) struct Hunk {
    pub(super) header: String,
    pub(super) lines: Vec<DiffLine>,
    pub(super) pairs: Vec<Pair>,
}
#[derive(Debug)]
pub(super) struct DiffLine {
    pub(super) old: Option<u32>,
    pub(super) new: Option<u32>,
    pub(super) text: String,
    pub(super) kind: Change,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Context,
    Added,
    Removed,
}
#[derive(Debug)]
pub(super) struct Pair {
    pub(super) old: Option<usize>,
    pub(super) new: Option<usize>,
}

impl Document {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let mut doc = Self::default();
        let mut file: Option<DiffFile> = None;
        let mut hunk: Option<Hunk> = None;
        let (mut old, mut new) = (0u32, 0u32);
        for line in text.lines() {
            if let Some(header) = line.strip_prefix("diff --git ") {
                finish_hunk(&mut file, &mut hunk);
                if let Some(f) = file.take() {
                    doc.files.push(f);
                }
                let (_, b) = split_header(header)?;
                let path = strip_prefix(b, "b/")?;
                file = Some(make_file(path));
            } else if let Some(f) = file.as_mut() {
                if let Some(header) = line.strip_prefix("@@ ") {
                    finish_hunk(&mut file, &mut hunk);
                    let mut parts = header.split_whitespace();
                    old = offset(parts.next(), '-')?;
                    new = offset(parts.next(), '+')?;
                    hunk = Some(Hunk {
                        header: line.into(),
                        lines: Vec::new(),
                        pairs: Vec::new(),
                    });
                } else if let Some(h) = hunk.as_mut() {
                    let (kind, text) = match line.as_bytes().first() {
                        Some(b' ') => (Change::Context, &line[1..]),
                        Some(b'+') => (Change::Added, &line[1..]),
                        Some(b'-') => (Change::Removed, &line[1..]),
                        _ => continue,
                    };
                    let old_line = (kind != Change::Added).then_some(old);
                    let new_line = (kind != Change::Removed).then_some(new);
                    if old_line.is_some() {
                        old = old.saturating_add(1);
                    }
                    if new_line.is_some() {
                        new = new.saturating_add(1);
                    }
                    h.lines.push(DiffLine {
                        old: old_line,
                        new: new_line,
                        text: text.into(),
                        kind,
                    });
                } else if let Some(name) = line
                    .strip_prefix("+++ ")
                    .filter(|name| *name != "/dev/null")
                {
                    f.path = strip_prefix(decode_path(name)?, "b/")?;
                    f.display = display(&f.path);
                } else if let Some(name) = line
                    .strip_prefix("--- ")
                    .filter(|name| *name != "/dev/null")
                {
                    f.path = strip_prefix(decode_path(name)?, "a/")?;
                    f.display = display(&f.path);
                } else if let Some(name) = line.strip_prefix("rename to ") {
                    let path = decode_path(name)?;
                    f.display = display(&path);
                    f.path = path;
                } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                    f.notice = Some("Binary file changed".into());
                }
            }
        }
        finish_hunk(&mut file, &mut hunk);
        if let Some(f) = file {
            doc.files.push(f);
        }
        Ok(doc)
    }
}

fn finish_hunk(file: &mut Option<DiffFile>, hunk: &mut Option<Hunk>) {
    if let (Some(file), Some(mut h)) = (file.as_mut(), hunk.take()) {
        let mut i = 0;
        while i < h.lines.len() {
            if h.lines[i].kind == Change::Context {
                h.pairs.push(Pair {
                    old: Some(i),
                    new: Some(i),
                });
                i += 1;
                continue;
            }
            let start = i;
            while i < h.lines.len() && h.lines[i].kind != Change::Context {
                i += 1;
            }
            let removed: Vec<_> = (start..i)
                .filter(|&j| h.lines[j].kind == Change::Removed)
                .collect();
            let added: Vec<_> = (start..i)
                .filter(|&j| h.lines[j].kind == Change::Added)
                .collect();
            for n in 0..removed.len().max(added.len()) {
                h.pairs.push(Pair {
                    old: removed.get(n).copied(),
                    new: added.get(n).copied(),
                });
            }
        }
        file.hunks.push(h);
    }
}
fn offset(part: Option<&str>, prefix: char) -> Result<u32, String> {
    part.and_then(|s| s.strip_prefix(prefix))
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| "Invalid Git hunk header".into())
}
fn split_header(s: &str) -> Result<(PathBuf, PathBuf), String> {
    if !s.starts_with('"')
        && let Some(i) = s.rfind(" b/")
    {
        return Ok((PathBuf::from(&s[..i]), PathBuf::from(&s[i + 1..])));
    }
    let (a, rest) = token(s)?;
    let (b, _) = token(rest.trim_start())?;
    Ok((a, b))
}
fn token(s: &str) -> Result<(PathBuf, &str), String> {
    if s.starts_with('"') {
        let bytes = s.as_bytes();
        let mut i = 1;
        let mut escaped = false;
        while i < bytes.len() {
            if bytes[i] == b'"' && !escaped {
                return Ok((decode_path(&s[..=i])?, &s[i + 1..]));
            }
            if bytes[i] == b'\\' {
                escaped = !escaped;
            } else {
                escaped = false;
            }
            i += 1;
        }
        Err("Invalid quoted Git path".into())
    } else {
        let end = s.find(' ').unwrap_or(s.len());
        Ok((PathBuf::from(&s[..end]), &s[end..]))
    }
}
fn decode_path(s: &str) -> Result<PathBuf, String> {
    if !s.starts_with('"') {
        return Ok(PathBuf::from(s));
    }
    let b = s.as_bytes();
    if b.len() < 2 || b[b.len() - 1] != b'"' {
        return Err("Invalid quoted Git path".into());
    }
    let mut out = Vec::new();
    let mut i = 1;
    while i < b.len() - 1 {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= b.len() - 1 {
            return Err("Invalid Git path escape".into());
        }
        match b[i] {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'\\' => out.push(b'\\'),
            b'"' => out.push(b'"'),
            b'0'..=b'7' if i + 2 < b.len() - 1 => {
                let n = (b[i] - b'0') as u16 * 64
                    + (b[i + 1] - b'0') as u16 * 8
                    + (b[i + 2] - b'0') as u16;
                if n > 255
                    || !(b'0'..=b'7').contains(&b[i + 1])
                    || !(b'0'..=b'7').contains(&b[i + 2])
                {
                    return Err("Invalid Git path escape".into());
                }
                out.push(n as u8);
                i += 2;
            }
            _ => return Err("Invalid Git path escape".into()),
        }
        i += 1;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(PathBuf::from(std::ffi::OsString::from_vec(out)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(out)
            .map(PathBuf::from)
            .map_err(|_| "Invalid Git path encoding".into())
    }
}
fn strip_prefix(path: PathBuf, prefix: &str) -> Result<PathBuf, String> {
    path.strip_prefix(prefix.trim_end_matches('/'))
        .map(PathBuf::from)
        .map_err(|_| "Invalid Git path".into())
}
pub(super) fn display(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default()
                    .collect::<String>()
                    .chars()
                    .collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
pub(super) fn make_file(path: PathBuf) -> DiffFile {
    DiffFile {
        display: display(&path),
        path,
        hunks: Vec::new(),
        notice: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_markers_disambiguate_spaces_that_look_like_header_separators() {
        let doc = Document::parse("diff --git a/foo b/bar b/foo b/bar\n--- a/foo b/bar\n+++ b/foo b/bar\n@@ -1 +1 @@\n-old\n+new\n").unwrap();
        assert_eq!(doc.files[0].path, std::path::Path::new("foo b/bar"));
    }
    #[test]
    fn parses_quoted_renamed_paths_offsets_and_pairs() {
        let text = "diff --git \"a/old\\tname\" \"b/new\\tname\"\nsimilarity index 75%\nrename from \"old\\tname\"\nrename to \"new\\tname\"\n@@ -4,3 +7,3 @@\n context\n-old\n+新\n \n";
        let doc = Document::parse(text).unwrap();
        let file = &doc.files[0];
        assert_eq!(file.path, PathBuf::from("new\tname"));
        assert_eq!(file.display, "new\\tname");
        let h = &file.hunks[0];
        assert_eq!((h.lines[0].old, h.lines[0].new), (Some(4), Some(7)));
        assert_eq!((h.lines[2].old, h.lines[2].new), (None, Some(8)));
        assert_eq!((h.pairs[1].old, h.pairs[1].new), (Some(1), Some(2)));
        assert_eq!(h.lines[3].text, "");
    }
    #[test]
    fn parses_space_names_new_deleted_and_binary() {
        let text = "diff --git a/has space b/has space\nnew file mode 100644\n@@ -0,0 +1 @@\n+hello\ndiff --git a/deleted b/deleted\ndeleted file mode 100644\n@@ -1 +0,0 @@\n-old\ndiff --git a/image b/image\nBinary files a/image and b/image differ\n";
        let doc = Document::parse(text).unwrap();
        assert_eq!(doc.files[0].path, PathBuf::from("has space"));
        assert_eq!(doc.files[0].hunks[0].lines[0].new, Some(1));
        assert_eq!(doc.files[1].hunks[0].lines[0].old, Some(1));
        assert!(doc.files[2].notice.is_some());
    }
}
