//! The one boundary between producer-controlled text and the desktop's URL opener.

mod console;
mod local;
pub(crate) use local::LocalTarget;

pub(crate) use console::{ForTheBench, PaintedConsole};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    Plain,
    Osc8,
    Panel,
}

impl From<term_core::LinkSource> for Origin {
    fn from(source: term_core::LinkSource) -> Self {
        match source {
            term_core::LinkSource::Plain => Self::Plain,
            term_core::LinkSource::Osc8 => Self::Osc8,
        }
    }
}

/// Whether a press or a hover with this chord is the console's link gesture: `Ctrl` alone.
///
/// One predicate for the click and for the hover that discloses it, so a hand is never shown for a
/// chord that would not open. On Linux and Windows egui sets `command` together with `ctrl`; on
/// macOS Command sets `mac_cmd` without `ctrl`, and the gesture is `Ctrl`, not the platform command.
/// `Ctrl+Shift` is moving a pane (D20).
pub(crate) fn opens_a_console_link(chord: eframe::egui::Modifiers) -> bool {
    chord.ctrl && !chord.shift && !chord.alt && !chord.mac_cmd
}

/// A complete web address the application may hand to the desktop.
///
/// **It holds the parser's form of the address, not the producer's bytes.** The desktop's opener
/// parses the string again and hands the browser that form — host lowercased, an international
/// host in punycode, the path percent-encoded — so keeping the original would let the tooltip show
/// one address while the browser loads another. What is disclosed on hover is this string, and so
/// is what opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebTarget(String);

impl WebTarget {
    /// Accepts only an absolute HTTP(S) address with a host, and nothing that paints as something
    /// other than itself: no control, whitespace or invisible formatting character, which would let
    /// a tooltip read differently from the address it stands for.
    pub(crate) fn parse(candidate: &str) -> Option<Self> {
        if candidate
            .chars()
            .any(|scalar| scalar.is_whitespace() || !panel_contract::is_shown_as_itself(scalar))
        {
            return None;
        }

        // The parser refuses an «http» or «https» address with an empty host on its own, so the
        // scheme is the one thing left to check on what it returns.
        let parsed = url::Url::parse(candidate).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }

        // The URL parser deliberately repairs «https:///path» into a host named «path». That is a
        // useful browser convenience, but it would turn incomplete producer text into a different
        // destination here. Require the authority to have existed in the original value too.
        let after_scheme = candidate.split_once(':')?.1.strip_prefix("//")?;
        let authority = after_scheme.split(['/', '?', '#']).next()?;
        if authority.is_empty() {
            return None;
        }

        Some(Self(safe_for_the_opener(parsed.as_str())))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The parser's form with the characters a command interpreter reads percent-encoded.
///
/// **Under WSL the opener runs `cmd.exe /c start <address>`** and escapes only `^` and `&`, and the
/// URL parser leaves `|` — and a `%` that is not an escape — as they are, so an address from a
/// program's output or a foreign panel could pipe into a command. Encoding them keeps the address
/// the same one for a browser and makes it inert for `cmd.exe`; the few the parser already encodes
/// (`"`, `<`, `>`) are here so the rule does not depend on which part of the address they sit in.
///
/// **Two gaps are known and left**, both only under WSL (D51): two valid escapes can still spell a
/// `%NAME%` that `cmd.exe` expands — encoding a valid escape would change the address — and when
/// `cmd.exe` cannot start, the opener falls back to `powershell.exe Start <address>`, which reads
/// `;`, `$`, a backtick, `@`, `{` and `(` among others, none of which this touches.
fn safe_for_the_opener(address: &str) -> String {
    let bytes = address.as_bytes();
    let mut safe = String::with_capacity(address.len());
    for (at, scalar) in address.char_indices() {
        let is_an_escape = scalar == '%'
            && bytes.get(at + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(at + 2).is_some_and(u8::is_ascii_hexdigit);
        match scalar {
            '|' => safe.push_str("%7C"),
            '"' => safe.push_str("%22"),
            '<' => safe.push_str("%3C"),
            '>' => safe.push_str("%3E"),
            '%' if !is_an_escape => safe.push_str("%25"),
            other => safe.push(other),
        }
    }
    safe
}

/// The line a click the opener refused leaves on standard error.
fn could_not_open(target: &WebTarget, why: &std::io::Error) -> String {
    format!(
        "ivy: no opener could be started for {}: {why}",
        target.as_str()
    )
}

/// Where an accepted address goes when somebody opens it.
pub(crate) enum Desktop {
    /// The desktop's own opener, on a thread of its own. **It says so when the opener reports it
    /// could not start one** — nothing registered, a spawn refused — with one line on standard
    /// error, because that click changed nothing a person can see (D51). A browser that started and
    /// then failed is past what the opener reports, and says nothing here.
    ///
    /// **Not on the interface thread**: on Linux the opener asks `xdg-settings` for the default
    /// browser on every call and waits for a text browser to exit, and either would stop the window.
    System,
    /// What the tests read instead of launching a browser.
    #[cfg(test)]
    Recorded(Vec<String>),
}

impl Desktop {
    pub(crate) fn open(&mut self, target: &WebTarget) {
        match self {
            Self::System => {
                let opening = target.clone();
                let spawned = std::thread::Builder::new()
                    .name("link opener".into())
                    .spawn(move || {
                        if let Err(why) = webbrowser::open(opening.as_str()) {
                            eprintln!("{}", could_not_open(&opening, &why));
                        }
                    });
                // A refused thread is the same answer as a refused opener, not a reason to panic
                // on the interface thread.
                if let Err(why) = spawned {
                    eprintln!("{}", could_not_open(target, &why));
                }
            }
            #[cfg(test)]
            Self::Recorded(opened) => opened.push(target.as_str().to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_http_and_https_addresses_with_a_host_are_accepted() {
        for (accepted, as_opened) in [
            ("http://example.test", "http://example.test/"),
            ("https://example.test/a?b=1", "https://example.test/a?b=1"),
            (
                "HTTPS://example.test/case-is-for-the-parser",
                "https://example.test/case-is-for-the-parser",
            ),
        ] {
            assert_eq!(
                WebTarget::parse(accepted).as_ref().map(WebTarget::as_str),
                Some(as_opened),
                "a supported address was refused or not normalised"
            );
        }

        for rejected in [
            "file:///tmp/a",
            "javascript:alert(1)",
            "mailto:person@example.test",
            "/relative",
            "https://",
            "https:///only-a-path",
            "https://example.test/line\nbreak",
            "https://example.test/with space",
            "https://example.test/\u{202e}gpj.exe",
            "https://exa\u{200b}mple.test/",
            "",
        ] {
            assert!(
                WebTarget::parse(rejected).is_none(),
                "an unsupported target was accepted: {rejected:?}"
            );
        }
    }

    #[test]
    fn what_is_disclosed_is_the_form_the_browser_receives() {
        // The opener re-parses what it is given and loads the parser's form, so the value kept —
        // and shown on hover — has to be that form and not the producer's spelling of it.
        let target = WebTarget::parse("HTTPS://Example.Test:443/a%2Fb?q=x#fragment")
            .expect("the address is valid");
        assert_eq!(target.as_str(), "https://example.test/a%2Fb?q=x#fragment");

        let international = WebTarget::parse("https://\u{430}pple.test/").expect("a valid host");
        assert_eq!(international.as_str(), "https://xn--pple-43d.test/");
    }

    #[test]
    fn what_a_command_interpreter_reads_reaches_the_opener_encoded() {
        let target =
            WebTarget::parse("https://example.test/a|calc?q=%PATH%&ok=%41").expect("valid");
        assert_eq!(
            target.as_str(),
            "https://example.test/a%7Ccalc?q=%25PATH%25&ok=%41",
            "a pipe or a variable reference would reach `cmd.exe` under WSL"
        );
    }

    #[test]
    fn a_refused_opener_is_said_with_the_address_and_the_reason() {
        let target = WebTarget::parse("https://example.test/said").expect("valid");
        let why = std::io::Error::new(std::io::ErrorKind::NotFound, "no browser found");
        assert_eq!(
            could_not_open(&target, &why),
            "ivy: no opener could be started for https://example.test/said: no browser found"
        );
    }

    #[test]
    fn opening_hands_the_desktop_the_target_it_was_given() {
        let mut desktop = Desktop::Recorded(Vec::new());
        desktop.open(&WebTarget::parse("https://example.test/open").expect("the target is valid"));

        let Desktop::Recorded(opened) = desktop else {
            unreachable!("the desktop was built recording")
        };
        assert_eq!(opened, ["https://example.test/open"]);
    }
}
