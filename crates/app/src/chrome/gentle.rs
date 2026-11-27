//! Explicit launch of the installed Gentle client in a separate console.
use crate::{OpeningLayout, Tab, Window, opening};
use eframe::egui;
use std::path::{Path, PathBuf};

fn resolve() -> term_core::Result<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    #[cfg(not(windows))]
    let roots = std::env::var_os("HOME")
        .map(PathBuf::from)
        .into_iter()
        .filter(|home| home.is_absolute())
        .map(|home| home.join(".gentle-shell/bin"))
        .collect::<Vec<_>>();
    #[cfg(windows)]
    let roots = [
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("gentle-shell/bin")),
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|p| p.join(".gentle-shell-bundle/bin")),
    ]
    .into_iter()
    .flatten()
    .filter(|path| path.is_absolute())
    .collect::<Vec<_>>();
    discover(std::env::split_paths(&path).chain(roots))
}

fn discover(directories: impl IntoIterator<Item = PathBuf>) -> term_core::Result<PathBuf> {
    for directory in directories.into_iter().filter(|p| p.is_absolute()) {
        #[cfg(windows)]
        let names = ["gentle-shell.exe", "gentle-shell.cmd"];
        #[cfg(not(windows))]
        let names = ["gentle-shell"];
        for name in names {
            let candidate = directory.join(name);
            if executable(&candidate) {
                #[cfg(windows)]
                if name.ends_with(".cmd") {
                    return Err("This Gentle Shell installation uses a Windows .cmd launcher, which Ivy cannot start safely. Run it in a normal console instead.".into());
                }
                return Ok(candidate);
            }
        }
    }
    Err("Gentle Shell was not found in an absolute PATH directory or its bundled installation. Install Gentle Shell and try again.".into())
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub(super) fn open(window: &mut Window, ctx: &egui::Context) {
    if !window.gentle_enabled {
        return;
    }
    let started = std::time::Instant::now();
    let focus = window.active().focus;
    let directory = crate::viewer::console_directory(window, focus);
    let name = window.next_pane_name;
    let tab_name = window.next_name;
    window.next_pane_name += 1;
    window.next_name += 1;
    let reads = window.control.read_activity();
    let result = resolve()
        .and_then(|program| launch(ctx, &program, directory.as_deref(), name, reads.clone()));
    let pane = result
        .unwrap_or_else(|error| opening::failed_pane(ctx, name, reads, "gentle-shell", error));
    let layout = OpeningLayout {
        columns: 1,
        rows: 1,
    };
    window.bring_to_front(Ok(Tab::with_panes(vec![pane], &layout, tab_name)), ctx);
    window.timed_separately += started.elapsed();
}

fn launch(
    ctx: &egui::Context,
    program: &std::path::Path,
    directory: Option<&std::path::Path>,
    name: usize,
    reads: crate::control::ReadActivity,
) -> term_core::Result<crate::Pane> {
    let program = program
        .to_str()
        .ok_or("Gentle Shell's executable path is not valid text")?;
    let directory = directory
        .filter(|path| path.is_dir())
        .ok_or("The active console's working directory is unavailable")?;
    opening::launch_exact(
        ctx,
        opening::Launch {
            program,
            args: &[],
            directory: Some(directory),
            path: None,
        },
        crate::panel_or_none(panel_contract::PanelDir::create()),
        name,
        reads,
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn missing_client_produces_a_visible_failure() {
        let ctx = egui::Context::default();
        let reads = crate::control::ReadActivity::default();
        let missing = "/nonexistent-ivy-gentle-client";
        let error = launch(
            &ctx,
            Path::new(missing),
            Some(std::env::temp_dir().as_path()),
            0,
            reads.clone(),
        )
        .err()
        .unwrap();
        let pane = opening::failed_pane(&ctx, 0, reads, missing, error);
        assert_eq!(
            pane.pty.notice(),
            Some("This console's program could not be started.")
        );
        assert!(
            pane.pty
                .startup_failure
                .as_ref()
                .unwrap()
                .program
                .contains(missing)
        );
        assert!(pane.pty.keeps_open());
    }

    #[test]
    fn client_starts_in_the_given_directory_without_arguments() {
        let root = std::env::temp_dir().join(format!(
            "ivy-gentle-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let directory = root.join("project with spaces;literal");
        std::fs::create_dir(&directory).unwrap();
        let program = root.join("gentle-fixture");
        let output = root.join("observed");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$#\" > '{}'\nexec /bin/cat\n",
                output.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let pane = launch(
            &egui::Context::default(),
            &program,
            Some(&directory),
            0,
            Default::default(),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let expected = format!("{}\n0\n", directory.display());
        while std::fs::read_to_string(&output).ok().as_deref() != Some(&expected)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(std::fs::read_to_string(&output).unwrap(), expected);
        drop(pane);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovery_ignores_empty_and_relative_directories_and_finds_absolute_install() {
        let root = std::env::current_dir().unwrap().join(format!(
            ".ivy-gentle-discover-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let binary = root.join("gentle-shell");
        std::fs::write(&binary, "fixture").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let relative = root.file_name().unwrap().into();
        assert!(discover([PathBuf::new(), PathBuf::from("."), relative]).is_err());
        assert_eq!(
            discover([PathBuf::new(), PathBuf::from("."), root.clone()]).unwrap(),
            binary
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn absolute_executable_is_selected_and_cmd_is_reported() {
        let root = std::env::temp_dir().join(format!(
            "ivy-gentle-discover-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let cmd = root.join("gentle-shell.cmd");
        std::fs::write(&cmd, "fixture").unwrap();
        let error = discover([PathBuf::new(), PathBuf::from("."), root.clone()]).unwrap_err();
        assert!(error.to_string().contains(".cmd"));
        let executable = root.join("gentle-shell.exe");
        std::fs::write(&executable, "fixture").unwrap();
        assert_eq!(
            discover([PathBuf::new(), PathBuf::from("."), root.clone()]).unwrap(),
            executable
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
