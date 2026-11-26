//! Shared ordinary-shell recovery follows D107; explicit native launch follows D90.

pub(super) fn spawn(
    program: &str,
    args: &[&str],
    allow_fallback: bool,
    mut backend: impl FnMut(&str, &[&str]) -> term_core::Result<super::Spawned>,
) -> term_core::Result<super::Spawned> {
    match backend(program, args) {
        Err(error) if allow_fallback && program != crate::shell_integration::fallback_shell() => {
            let fallback = crate::shell_integration::fallback_shell();
            crate::pty_output::say(&format!(
                "'{program}' could not be launched ({error}); opening {fallback}"
            ));
            backend(fallback, &[])
        }
        result => result,
    }
}
