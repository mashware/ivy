//! The language a console's program starts with when the application inherited none.
//!
//! An app opened from the macOS Dock or Finder inherits launchd's environment, which has no `LANG`,
//! so every console ran in the C locale. There zsh counts each byte of a UTF-8 prompt glyph as a
//! column (`❯` is three bytes and one cell), its idea of the cursor drifts, and a redraw leaves text
//! on screen that Backspace cannot reach. WezTerm, kitty and Ghostty answer it the same way, and so
//! does this module: only on macOS, only when nothing was inherited, a UTF-8 locale built from the
//! system's language and region, checked to exist, with `en_US.UTF-8` behind it.

// The decision is only applied on macOS, and still compiled and tested everywhere.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::ffi::OsStr;

/// What is set when the system's own choice cannot be used. Present on every macOS install.
pub(crate) const FALLBACK: &str = "en_US.UTF-8";

/// The `LANG` a console should be given, or `None` to leave its environment as inherited.
///
/// `inherited` is `LC_ALL`, `LC_CTYPE` and `LANG` as the application received them. Any of them
/// non-empty is somebody's choice — `C` included — and is never replaced. `system` is the
/// language and region codes the platform reports; `exists` says whether a locale name is installed.
pub(crate) fn lang_to_set(
    inherited: [Option<&OsStr>; 3],
    system: Option<(&str, Option<&str>)>,
    exists: impl Fn(&str) -> bool,
) -> Option<String> {
    if inherited
        .iter()
        .any(|value| value.is_some_and(|value| !value.is_empty()))
    {
        return None;
    }
    let candidate = system.and_then(|(language, region)| {
        let region = region?;
        let plain = |code: &str, allowed: fn(&u8) -> bool| {
            (2..=3).contains(&code.len()) && code.as_bytes().iter().all(allowed)
        };
        (plain(language, u8::is_ascii_alphabetic) && plain(region, u8::is_ascii_alphanumeric))
            .then(|| format!("{language}_{region}.UTF-8"))
    });
    Some(
        candidate
            .filter(|name| exists(name))
            .unwrap_or_else(|| FALLBACK.to_owned()),
    )
}

/// Whether the C library can open `name` as a locale.
///
/// `newlocale` instead of `setlocale`: it does not touch the process's own locale, which other
/// threads may be reading.
#[cfg(unix)]
pub(crate) fn exists(name: &str) -> bool {
    let Ok(name) = std::ffi::CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is a valid NUL-terminated string that outlives the call, and a null base asks
    // for a fresh locale object.
    let locale =
        unsafe { libc::newlocale(libc::LC_CTYPE_MASK, name.as_ptr(), std::ptr::null_mut()) };
    if locale.is_null() {
        return false;
    }
    // SAFETY: `locale` was just returned non-null by `newlocale` and is freed exactly once.
    unsafe { libc::freelocale(locale) };
    true
}

/// The system's language and region codes, as the Mac's settings name them.
#[cfg(target_os = "macos")]
fn system() -> Option<(String, Option<String>)> {
    use objc2_foundation::NSLocale;
    let locale = NSLocale::currentLocale();
    // `countryCode` and not `regionCode`: the latter needs macOS 13, and WezTerm reads the same one.
    #[allow(deprecated)]
    let region = locale.countryCode().map(|code| code.to_string());
    Some((locale.languageCode().to_string(), region))
}

/// What [`lang_to_set`] decides for this process, read from its real environment once.
///
/// Only macOS: a Linux or Windows session always carries its locale, and the reference terminals
/// leave it alone there too. Not covered by a test — it reads the process environment, which the
/// suite must not write (`std::env::set_var` is undefined behaviour while consoles are launched in
/// parallel); the decision it hands over is what the tests cover.
pub(crate) fn for_consoles() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        static DECIDED: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        DECIDED
            .get_or_init(|| {
                let inherited = ["LC_ALL", "LC_CTYPE", "LANG"].map(std::env::var_os);
                let system = system();
                lang_to_set(
                    [0, 1, 2].map(|index| inherited[index].as_deref()),
                    system
                        .as_ref()
                        .map(|(language, region)| (language.as_str(), region.as_deref())),
                    exists,
                )
            })
            .as_deref()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTHING: [Option<&OsStr>; 3] = [None, None, None];

    fn installed(name: &str) -> bool {
        name == "es_ES.UTF-8"
    }

    /// The Mac's case: nothing inherited, and the system's own language and region are installed.
    #[test]
    fn with_nothing_inherited_the_systems_locale_is_set() {
        assert_eq!(
            lang_to_set(NOTHING, Some(("es", Some("ES"))), installed).as_deref(),
            Some("es_ES.UTF-8")
        );
    }

    /// Empty is what launchd hands over, and it counts as nothing.
    #[test]
    fn empty_values_count_as_nothing_inherited() {
        let empty = Some(OsStr::new(""));
        assert_eq!(
            lang_to_set([empty, empty, empty], Some(("es", Some("ES"))), installed).as_deref(),
            Some("es_ES.UTF-8")
        );
    }

    /// Each of the three, on its own, is somebody's choice and stops the decision — `C` included.
    #[test]
    fn any_inherited_value_is_left_alone() {
        for index in 0..3 {
            let mut inherited = NOTHING;
            inherited[index] = Some(OsStr::new("C"));
            assert_eq!(
                lang_to_set(inherited, Some(("es", Some("ES"))), installed),
                None,
                "the variable at position {index} was overridden"
            );
        }
    }

    /// A pair the system can report and no locale exists for — English with a Spanish region.
    #[test]
    fn a_locale_that_is_not_installed_falls_back() {
        assert_eq!(
            lang_to_set(NOTHING, Some(("en", Some("ES"))), installed).as_deref(),
            Some(FALLBACK)
        );
    }

    #[test]
    fn no_region_or_no_system_answer_falls_back() {
        assert_eq!(
            lang_to_set(NOTHING, Some(("es", None)), |_| true).as_deref(),
            Some(FALLBACK)
        );
        assert_eq!(
            lang_to_set(NOTHING, None, |_| true).as_deref(),
            Some(FALLBACK)
        );
    }

    /// Codes that are not plain letters never reach the name, even if something claims it exists.
    #[test]
    fn codes_that_are_not_plain_fall_back() {
        for (language, region) in [
            ("e", "ES"),
            ("es", "E/S"),
            ("es\0", "ES"),
            ("español", "ES"),
        ] {
            assert_eq!(
                lang_to_set(NOTHING, Some((language, Some(region))), |_| true).as_deref(),
                Some(FALLBACK),
                "{language:?}_{region:?} was used"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_c_library_is_asked_whether_a_locale_exists() {
        assert!(exists("C"), "the C locale always exists");
        assert!(!exists("zz_QQ.UTF-8"), "an invented locale was accepted");
        assert!(!exists("C\0x"), "a name with a NUL was accepted");
    }
}
