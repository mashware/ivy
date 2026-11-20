//! macOS Gatekeeper App Translocation (D106): a quarantined app opened from where it was downloaded runs
//! from a randomized read-only copy under `…/AppTranslocation/<uuid>/d/`. That copy exists only
//! while this launch lives, so a path derived from it must never be offered for a client to save.
//!
//! The path component is the observable signal. The Security framework's translocation query is
//! the authoritative one, but it needs a native binding for an answer the path already gives.
//! Removing the quarantine attribute is ruled out as an installation step
//! (`docs/10-installation.md`), so the only remedy offered is moving the app with Finder.
use std::path::Path;

/// What the guide shows in place of any path while Ivy runs from the temporary copy.
pub(super) const NOTICE: &str = "Ivy is running from a temporary copy that macOS makes for a downloaded app it has not seen moved with Finder. That copy disappears when Ivy quits, so no path is offered here. Quit Ivy, drag Ivy.app into the Applications folder in Finder, and open it again from there.";

/// The recovery section's own line, kept apart from [`NOTICE`] so the guide never repeats it.
pub(super) const NO_RECOVERY_PATHS: &str =
    "Recovery paths are not offered while Ivy runs from a temporary copy of itself.";

pub(super) fn translocated(executable: &Path) -> bool {
    executable
        .components()
        .any(|part| part.as_os_str() == "AppTranslocation")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_whole_app_translocation_component_counts() {
        assert!(translocated(Path::new(
            "/private/var/folders/rq/x/T/AppTranslocation/78459825-AB/d/Ivy.app/Contents/MacOS/ivy"
        )));
        assert!(!translocated(Path::new(
            "/Applications/Ivy.app/Contents/MacOS/ivy"
        )));
        assert!(!translocated(Path::new(
            "/Users/me/AppTranslocationNotes/Ivy.app/Contents/MacOS/ivy"
        )));
    }
}
