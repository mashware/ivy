//! What Files' name search matches a name against: the typed text, or a pattern (D61, #121).

use regex_automata::meta::Regex;
use regex_automata::util::syntax;
use std::ops::Range;

/// The same ceiling as the console's search: a pattern is compiled on every keystroke, and the
/// engine's own default lets a short Unicode pattern take tens of milliseconds to build.
const SIZE_LIMIT: usize = 2 * 1024 * 1024;

/// A query, compiled once per change and shared with the worker that walks the folders.
#[derive(Debug)]
pub(crate) enum Matcher {
    /// The text, lowercased: a name matches when its lowercase holds it, as Files always did.
    Text(String),
    /// A pattern, case-insensitive unless it says otherwise with `(?-i)`.
    Pattern(Regex),
}

impl Matcher {
    /// The query as text or as a pattern; a pattern that cannot be built says why, in words the
    /// list can show instead of an empty result.
    pub(crate) fn new(query: &str, regex: bool) -> Result<Self, String> {
        if !regex {
            return Ok(Self::Text(query.to_lowercase()));
        }
        Regex::builder()
            .configure(Regex::config().nfa_size_limit(Some(SIZE_LIMIT)))
            .syntax(syntax::Config::new().case_insensitive(true))
            .build(query)
            .map(Self::Pattern)
            .map_err(|error| {
                if let Some(syntax) = error.syntax_error() {
                    // The parser's own message draws the pattern with a caret under the fault
                    // over several lines; the list's footer has one.
                    let why = match syntax {
                        regex_syntax::Error::Parse(error) => error.kind().to_string(),
                        regex_syntax::Error::Translate(error) => error.kind().to_string(),
                        other => other.to_string(),
                    };
                    format!("Invalid pattern: {why}.")
                } else if error.size_limit().is_some() {
                    "Pattern too complex to search.".into()
                } else {
                    format!("Invalid pattern: {error}.")
                }
            })
    }

    /// The part of `name` this query matches, if any. **An empty match is no match**: a pattern
    /// such as `x*` would otherwise list every entry with nothing to show for it.
    pub(crate) fn span(&self, name: &str) -> Option<Range<usize>> {
        match self {
            Self::Text(needle) => {
                // Lowercasing can change a character's length, so an offset found in the
                // lowercased name is only kept where it falls on the name's own boundaries.
                let at = name.to_lowercase().find(needle.as_str())?;
                let end = at + needle.len();
                (name.is_char_boundary(at) && name.is_char_boundary(end)).then_some(at..end)
            }
            Self::Pattern(regex) => regex
                .find_iter(name)
                .map(|found| found.range())
                .find(|range| !range.is_empty()),
        }
    }

    /// Whether `name` is listed.
    pub(crate) fn matches(&self, name: &str) -> bool {
        match self {
            Self::Text(needle) => name.to_lowercase().contains(needle.as_str()),
            Self::Pattern(_) => self.span(name).is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_text_the_query_is_a_case_insensitive_piece_of_the_name() {
        let matcher = Matcher::new("Main.RS", false).unwrap();
        assert!(matcher.matches("main.rs"));
        assert!(matcher.matches("domain.rsx"));
        assert!(!matcher.matches("mainXrs"), "«.» was read as a pattern");
        assert_eq!(matcher.span("my_main.rs"), Some(3..10));
    }

    #[test]
    fn as_a_pattern_it_is_anchored_where_it_says_and_marks_what_it_matched() {
        let matcher = Matcher::new(r"^main\.rs$", true).unwrap();
        assert!(matcher.matches("main.rs"));
        assert!(
            matcher.matches("MAIN.RS"),
            "case is ignored unless the pattern says otherwise"
        );
        assert!(!matcher.matches("domain.rs"));
        assert!(!matcher.matches("main.rsx"));
        assert_eq!(matcher.span("main.rs"), Some(0..7));

        let exact = Matcher::new(r"(?-i)^main", true).unwrap();
        assert!(!exact.matches("Main.rs"));

        let digits = Matcher::new(r"\d+", true).unwrap();
        assert_eq!(digits.span("log-2024.txt"), Some(4..8));
    }

    #[test]
    fn a_pattern_that_only_matches_nothing_lists_nothing() {
        let matcher = Matcher::new("x*", true).unwrap();
        assert!(!matcher.matches("abc"));
        assert_eq!(matcher.span("abxc"), Some(2..3));
    }

    #[test]
    fn a_pattern_that_cannot_be_built_says_why() {
        let invalid = Matcher::new("(", true).unwrap_err();
        assert!(invalid.starts_with("Invalid pattern: "), "{invalid}");
        assert!(!invalid.contains('\n'), "{invalid}");
        assert_eq!(
            Matcher::new(r"\w{50}", true).unwrap_err(),
            "Pattern too complex to search."
        );
        assert!(
            Matcher::new("(", false).is_ok(),
            "as text, «(» is a character"
        );
    }
}
