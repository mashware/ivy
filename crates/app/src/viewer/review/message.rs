use super::Review;
use std::path::Path;

pub(super) fn preview_text(review: &Review, source: &str, repository: &Path) -> String {
    let mut out = format!(
        "Git review for console {}\nRepository: {}\n",
        quote(source),
        quote(&repository.to_string_lossy())
    );
    for c in &review.comments {
        out.push_str(&format!(
            "\nFile: {}\n{} line {}\n{}\n",
            quote(&c.location.path.to_string_lossy()),
            c.location.side,
            c.location.line,
            c.text
        ));
    }
    out
}

pub(super) fn safe(s: &str) -> bool {
    s.chars()
        .all(|c| !c.is_control() || matches!(c, '\n' | '\t'))
}

pub(super) fn quote(s: &str) -> String {
    format!("\"{}\"", s.escape_debug())
}
