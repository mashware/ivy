//! The Mac desktop: `open`, and `open -R` to show a file in the Finder.
use super::Call;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn run(call: &Call) -> Result<(), String> {
    match call {
        Call::Open(path) => wait(open(path, false)),
        Call::Reveal(path) => wait(open(path, true)),
    }
}

/// `open` with `path` as its last argument, and nothing that names an application or passes it
/// arguments.
fn open(path: &Path, reveal: bool) -> Command {
    let mut command = Command::new("open");
    if reveal {
        command.arg("-R");
    }
    command.arg(path).stdin(Stdio::null()).stdout(Stdio::null());
    command
}

/// Run `command` and wait for it, so it is reaped; a launcher that exits unhappy says so.
fn wait(mut command: Command) -> Result<(), String> {
    // Abandoned on purpose: nothing joins the click's thread, so a launcher that never exits
    // keeps only that thread, never the window.
    let status = command.status().map_err(|why| why.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("open ended with {status}"))
    }
}
