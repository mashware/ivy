//! What Files' name search matches a name against: the typed text or a pattern, compiled the way the
//! console's search compiles its query (D61), so a query means the same thing in both places.

use std::ops::Range;
use std::sync::Arc;
use term_core::{Options, Pattern, Refused};

/// A query, compiled once per change and shared with the worker that walks the folders.
pub(crate) struct Matcher(Pattern);

impl Matcher {
    /// The query as text or as a pattern, case-insensitive as Files always was — a pattern's own
    /// `(?-i)` overrides it. One that cannot be built says why, in words the list shows instead of
    /// an empty result.
    pub(crate) fn new(query: &str, regex: bool) -> Result<Self, String> {
        let options = Options {
            regex,
            ..Options::default()
        };
        Pattern::new(query, options)
            .map(Self)
            .map_err(|refused| match refused {
                Refused::Empty => "Type a name to search.".into(),
                Refused::TooLarge => "Too complex to search.".into(),
                Refused::Invalid(why) => format!("Invalid pattern: {why}."),
            })
    }

    /// The part of `name` this query matches, if any.
    pub(crate) fn span(&self, name: &str) -> Option<Range<usize>> {
        self.0.find(name)
    }

    /// Whether `name` is listed: exactly when it has a part to mark.
    pub(crate) fn matches(&self, name: &str) -> bool {
        self.span(name).is_some()
    }
}

/// The query a matcher was compiled from, kept beside it: the list asks for the matcher on every
/// frame, and compiling again is only due when either changed.
pub(crate) struct Compiled {
    pub(crate) query: String,
    pub(crate) regex: bool,
    pub(crate) result: Result<Arc<Matcher>, String>,
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
        let invalid = Matcher::new("(", true).err().unwrap();
        assert!(
            invalid.starts_with("Invalid pattern: ") && invalid.contains("unclosed"),
            "{invalid}"
        );
        assert!(!invalid.contains('\n'), "{invalid}");
        assert_eq!(
            Matcher::new(r"\w{50}", true).err().unwrap(),
            "Too complex to search."
        );
        assert!(
            Matcher::new("(", false).is_ok(),
            "as text, «(» is a character"
        );
    }
}
