//! Startup. All the work lives in the library, which is what the sweep bench consumes.

fn main() -> std::process::ExitCode {
    if let Some(code) = term_core::dispatch_scope_helper() {
        std::process::exit(code);
    }
    // Held here and not in `run`: a window that cannot open is reported after `run` returns (D113).
    let capture = app::diagnostics_log::start();
    if let Ok(path) = std::env::current_exe() {
        app::enable_scope_launcher(path);
    }
    app::diagnostics_log::finish(app::run(), capture)
}
