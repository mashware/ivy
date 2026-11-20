//! The Linux desktop: `xdg-open` for a file, the file manager bus service to show one.
use super::Call;
use futures_lite::future;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long the file manager has to answer before its folder is opened instead. Seconds, not the
/// fraction a scope placement gets: this runs off the window's thread, and a file manager that
/// starts slowly and answers after the fallback has fired would open a second window.
const REVEAL_DEADLINE: Duration = Duration::from_secs(5);

pub(super) fn run(call: &Call) -> Result<(), String> {
    match call {
        Call::Open(path) => wait(opener(path)),
        Call::Reveal(path) => match fallback(path, show_items)? {
            Some(command) => wait(command),
            None => Ok(()),
        },
    }
}

/// `xdg-open` with `path` as its one argument: no shell reads it, and an absolute path never
/// starts with a dash an option parser could take. What it and the application print is not the
/// terminal's: a failure is said once, by this module.
pub(super) fn opener(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Ask `bus` to show `path` selected; when nothing answers, the command that opens its folder
/// instead. Never the file itself: a reveal that failed must not become an open.
pub(super) fn fallback(
    path: &Path,
    bus: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Option<Command>, String> {
    let uri = url::Url::from_file_path(path).map_err(|()| "not an absolute path".to_owned())?;
    if bus(uri.as_str()).is_ok() {
        return Ok(None);
    }
    let folder = path.parent().ok_or("it has no folder")?;
    Ok(Some(opener(folder)))
}

/// Run `command` and wait for it, so it is reaped; a launcher that exits unhappy says so.
fn wait(mut command: Command) -> Result<(), String> {
    // Abandoned on purpose: nothing joins the click's thread, so a launcher that never exits
    // keeps only that thread, never the window.
    let status = command.status().map_err(|why| why.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} ended with {status}", command.get_program()))
    }
}

/// The file manager asked over the session bus to show the item, given up at its deadline.
fn show_items(uri: &str) -> Result<(), String> {
    let deadline = Instant::now() + REVEAL_DEADLINE;
    future::block_on(future::or(
        async {
            let connection = zbus::Connection::session().await?;
            connection
                .call_method(
                    Some("org.freedesktop.FileManager1"),
                    "/org/freedesktop/FileManager1",
                    Some("org.freedesktop.FileManager1"),
                    "ShowItems",
                    &(vec![uri], ""),
                )
                .await?;
            Ok::<_, zbus::Error>(())
        },
        async move {
            async_io::Timer::at(deadline).await;
            Err(zbus::Error::Failure(
                "the file manager did not answer".into(),
            ))
        },
    ))
    .map_err(|why| why.to_string())
}
