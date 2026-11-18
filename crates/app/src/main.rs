//! Startup. All the work lives in the library, which is what the sweep bench consumes.

fn main() -> eframe::Result {
    if let Some(code) = term_core::dispatch_scope_helper() {
        std::process::exit(code);
    }
    if let Ok(path) = std::env::current_exe() {
        app::enable_scope_launcher(path);
    }
    app::run()
}
