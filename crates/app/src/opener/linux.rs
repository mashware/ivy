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
    match command_for(call, show_items)? {
        Launch::Run(command) => wait(command),
        Launch::Done => Ok(()),
        Launch::Folder { command, why } => {
            let (Call::Open(path) | Call::Reveal(path)) = call;
            let line = super::could_not("select", path, &format!("{why}; opening its folder"));
            eprintln!("{line}");
            wait(command).map_err(|failed| format!("{failed}, after the file manager: {why}"))
        }
    }
}

/// What a click runs once the file manager's bus has been asked, if it had to be.
pub(super) enum Launch {
    /// This command, for the file itself.
    Run(Command),
    /// The file manager showed the file; nothing to run.
    Done,
    /// The file manager could not show it (`why`), so this command opens its folder.
    Folder { command: Command, why: String },
}

/// The command for `call`: an open runs `xdg-open` on the file; a reveal asks `bus` to show the
/// file selected and, when that fails or times out, opens its folder — never the file itself, so a
/// reveal that failed does not become an open.
pub(super) fn command_for(
    call: &Call,
    bus: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Launch, String> {
    let path = match call {
        Call::Open(path) => return Ok(Launch::Run(opener(path))),
        Call::Reveal(path) => path,
    };
    let uri = url::Url::from_file_path(path).map_err(|()| "not an absolute path".to_owned())?;
    let Err(why) = bus(uri.as_str()) else {
        return Ok(Launch::Done);
    };
    let folder = path.parent().ok_or("it has no folder")?;
    // `xdg-open` types what it gets: a folder swapped for a link to a page would open a browser.
    let still_a_folder = std::fs::symlink_metadata(folder).is_ok_and(|found| found.is_dir());
    if !still_a_folder {
        return Err(format!("{why}, and its folder is no longer a folder"));
    }
    Ok(Launch::Folder {
        command: opener(folder),
        why,
    })
}

/// `xdg-open` with `path` as its one argument: no shell reads it, and an absolute path never
/// starts with a dash an option parser could take. What it and the application print is not the
/// terminal's: a failure is said once, by the opener.
pub(super) fn opener(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Run `command` and wait for it, so it is reaped; a launcher that exits unhappy says so.
pub(super) fn wait(mut command: Command) -> Result<(), String> {
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
