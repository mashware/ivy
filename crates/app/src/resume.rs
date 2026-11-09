//! Fixed client launches and a revocable bridge to the pane's off-frame session service.
use crate::{control, opening, session};
use panel_control::session::{Client, Identity, SessionHandler};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Route {
    handler: Option<Arc<Mutex<Option<SessionHandler>>>>,
}

impl Route {
    pub(crate) fn callback(&mut self) -> SessionHandler {
        let handler = Arc::clone(self.handler.get_or_insert_with(Default::default));
        Arc::new(move |peer, request| {
            let callback = handler
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            match callback {
                Some(callback) => callback(peer, request),
                None => serde_json::json!({"ok":false,"error":"disabled"}),
            }
        })
    }

    pub(crate) fn set(&self, callback: Option<SessionHandler>) {
        if let Some(handler) = &self.handler {
            *handler.lock().unwrap_or_else(|error| error.into_inner()) = callback;
        }
    }
}

pub(crate) fn helper_path() -> Option<std::path::PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let name = if cfg!(windows) {
        "ivy-session.exe"
    } else {
        "ivy-session"
    };
    let path = executable.with_file_name(name);
    if executable_file(&path) {
        return Some(path);
    }
    #[cfg(test)]
    if executable.parent()?.file_name()? == "deps" {
        let path = executable.parent()?.parent()?.join(name);
        if executable_file(&path) {
            return Some(path);
        }
    }
    None
}

fn executable_file(path: &std::path::Path) -> bool {
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

pub(crate) fn arguments(identity: &Identity) -> Vec<String> {
    let prefix: &[&str] = match identity.client {
        Client::Codex => &["resume", "--no-daemon"],
        Client::Claude => &["--resume"],
        Client::Opencode => &["--session"],
    };
    prefix
        .iter()
        .map(|arg| (*arg).to_owned())
        .chain(std::iter::once(identity.session_id.clone()))
        .collect()
}

pub(crate) fn try_open(
    ctx: &eframe::egui::Context,
    leaf: &session::StoredLeaf,
    name: usize,
    reads: control::ReadActivity,
) -> term_core::Result<crate::Pane> {
    try_open_with(ctx, leaf, name, reads, crate::ai_sessions::resolve_launch)
}

fn try_open_with(
    ctx: &eframe::egui::Context,
    leaf: &session::StoredLeaf,
    name: usize,
    reads: control::ReadActivity,
    resolve: impl FnOnce(Client) -> std::io::Result<crate::ai_sessions::ResolvedLaunch>,
) -> term_core::Result<crate::Pane> {
    let identity = leaf.resume.as_ref().ok_or("no saved client identity")?;
    let directory = leaf
        .cwd
        .as_deref()
        .filter(|path| path.is_dir())
        .ok_or("saved client directory is unavailable")?;
    let resolved = resolve(identity.client)?;
    let program = resolved
        .program
        .to_str()
        .ok_or("client executable path is not text")?;
    let mut args = resolved.prefix_args;
    args.extend(arguments(identity));
    let mut pane = opening::launch_exact(
        ctx,
        opening::Launch {
            program,
            args: &args,
            directory: Some(directory),
            path: Some(&resolved.path),
        },
        crate::panel_or_none(panel_contract::PanelDir::create()),
        name,
        reads,
    )?;
    pane.restoration.directory = leaf.cwd.clone();
    pane.restoration.resume = leaf.resume.clone();
    pane.restoration.resuming = true;
    Ok(pane)
}

pub(crate) fn set_capture(pane: &mut crate::Pane, enabled: bool) {
    if !enabled {
        if pane.restoration.sessions.take().is_some() || pane.restoration.scope_unavailable {
            pane.restoration.route.set(None);
            pane.restoration.session_revision = 0;
            pane.restoration.session_status = None;
            pane.restoration.scope_unavailable = false;
        }
        return;
    }
    if pane.restoration.sessions.is_some() || pane.restoration.scope_unavailable {
        return;
    }
    let Some(wake) = &pane.restoration.wake else {
        return;
    };
    match pane.pty.foreground_scope() {
        Ok(scope) => {
            let handle = crate::ai_sessions::Handle::new(scope, Arc::clone(wake));
            pane.restoration.route.set(Some(handle.callback()));
            handle.set_enabled(true);
            pane.restoration.sessions = Some(handle);
        }
        Err(_) => {
            pane.restoration.scope_unavailable = true;
            pane.restoration.session_status = Some(crate::ai_sessions::Status::Unsupported);
        }
    }
}

pub(crate) fn capture(
    pane: &mut crate::Pane,
    final_snapshot: Option<crate::ai_sessions::Snapshot>,
) {
    if pane.restoration.failed {
        return;
    }
    let snapshot = final_snapshot.or_else(|| {
        pane.restoration
            .sessions
            .as_ref()?
            .snapshot_if_changed(pane.restoration.session_revision)
    });
    let Some(snapshot) = snapshot else {
        return;
    };
    pane.restoration.session_revision = snapshot.revision;
    pane.restoration.session_status = Some(snapshot.status);
    if pane.restoration.resuming
        && (!snapshot.sampled || (!pane.restoration.interacted && snapshot.identity.is_none()))
    {
        return;
    }
    if pane.restoration.resuming
        && !pane.restoration.interacted
        && pane.restoration.resume.as_ref() != snapshot.identity.as_ref()
        && snapshot.identity.is_some()
    {
        pane.restoration.failed = true;
        pane.restoration.resuming = false;
        return;
    }
    pane.restoration.session_directory = if snapshot.identity.is_some() {
        snapshot.cwd
    } else {
        None
    };
    pane.restoration.resume = snapshot.identity;
    if snapshot.sampled {
        pane.restoration.resuming = false;
    }
}

pub(crate) fn finished(pane: &crate::Pane, queue_left: bool) -> bool {
    !pane.restoration.failed
        && !pane.restoration.resuming
        && crate::pty_output::is_finished(pane.pty_output.is_alive(), queue_left, pane.pty.ending())
}

pub(crate) fn inspect_restored_exit(pane: &mut crate::Pane) {
    if !pane.restoration.resuming {
        return;
    }
    let Some(success) = pane.pty.exit_success() else {
        return;
    };
    if success {
        pane.restoration.resuming = false;
        pane.restoration.resume = None;
        if let Some(wake) = &pane.restoration.wake {
            wake();
        }
        return;
    }
    let mut nothing = Vec::new();
    let pickup =
        pane.pty_output
            .pick_up(&mut nothing, 0, crate::pty_output::Silences::of(&pane.pty));
    if pane.pty_output.is_alive() || pickup.left_a_queue {
        return;
    }
    pane.restoration.failed = true;
    pane.restoration.resuming = false;
    if let Some(wake) = &pane.restoration.wake {
        wake();
    }
}

/// Preserve the failed client's output until an explicit user action starts a fresh shell.
pub(crate) fn take_over(pane: &mut crate::Pane) -> term_core::Result<()> {
    if pane.restoration.failed && pane.pty.exit_success().is_some() {
        let wake = pane.restoration.wake.as_ref().map(Arc::clone);
        let mut identity: Vec<_> = crate::identity_of(pane.panel.as_ref())
            .into_iter()
            .collect();
        let helper = helper_path();
        identity.push((
            "IVY_SESSION_HELPER",
            helper.as_deref().map(std::path::Path::as_os_str),
        ));
        let directory = pane
            .restoration
            .directory
            .as_deref()
            .filter(|path| path.is_dir());
        let size = pane.screen.size();
        let (pty, output) = term_core::Pty::spawn_with_env_in(
            &crate::the_users_shell(),
            &[],
            size,
            &identity,
            directory,
        )?;
        pane.restoration.route.set(None);
        pane.restoration.sessions = None;
        pane.restoration.session_revision = 0;
        pane.restoration.session_status = None;
        pane.restoration.scope_unavailable = false;
        pane.pty = pty;
        pane.pty_output = crate::pty_output::PtyOutput::new(output, move || {
            if let Some(wake) = &wake {
                wake();
            }
        });
        pane.screen = term_core::Screen::new(size);
        pane.signals.reset_for_new_shell(&mut pane.screen);
        pane.painted_console = None;
        pane.received.clear();
        pane.held = None;
        pane.search = Default::default();
        pane.pending_wheel = Default::default();
    }
    pane.restoration.take_over();
    Ok(())
}

pub(crate) fn dismiss_failure(pane: &mut crate::Pane) {
    if let Err(error) = take_over(pane) {
        eprintln!("A fresh console could not be opened: {error}");
    }
}

pub(crate) fn write_keys(pane: &mut crate::Pane) -> term_core::Result<()> {
    if pane.restoration.failed {
        take_over(pane)?;
    }
    pane.pty.write(&pane.keystrokes)
}

pub(crate) fn write(pane: &mut crate::Pane, bytes: &[u8]) -> term_core::Result<()> {
    if pane.restoration.failed {
        take_over(pane)?;
    }
    pane.pty.write(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    const ID: &str = "11111111-1111-4111-8111-111111111111";

    fn saved(client: Client, directory: PathBuf) -> session::StoredLeaf {
        session::StoredLeaf {
            panel_width: 30,
            folded: false,
            label: None,
            cwd: Some(directory),
            resume: Some(
                Identity::new(
                    client,
                    if client == Client::Opencode {
                        "ses_Exact123"
                    } else {
                        ID
                    },
                )
                .unwrap(),
            ),
        }
    }

    #[test]
    fn saved_identity_is_typed_and_lower_modes_remove_it() {
        let document = serde_json::json!({"v":2,"tabs":[{"root":{"leaf":{
            "panel_width":30,"folded":false,"cwd":"/tmp/project",
            "resume":{"client":"codex","session_id":ID}
        }}}]});
        let parsed = session::interpret(&document.to_string()).unwrap();
        let session::StoredNode::Leaf(leaf) = &parsed.tabs()[0].root else {
            panic!("leaf");
        };
        assert_eq!(leaf.resume.as_ref().unwrap().session_id, ID);
        let directories = parsed.clone().without_sessions();
        let session::StoredNode::Leaf(leaf) = &directories.tabs()[0].root else {
            panic!("leaf");
        };
        assert!(leaf.resume.is_none());
        assert_eq!(
            leaf.cwd.as_deref(),
            Some(std::path::Path::new("/tmp/project"))
        );
        let layout = parsed.without_directories();
        let session::StoredNode::Leaf(leaf) = &layout.tabs()[0].root else {
            panic!("leaf");
        };
        assert!(leaf.resume.is_none() && leaf.cwd.is_none());
        for resume in [
            serde_json::json!({"client":"sh","session_id":ID}),
            serde_json::json!({"client":"codex","session_id":"--continue"}),
            serde_json::json!({"client":"claude","session_id":ID,"argv":["danger"]}),
            serde_json::json!({"client":"opencode","session_id":"ses_a;touch forbidden"}),
        ] {
            let mut invalid = document.clone();
            invalid["tabs"][0]["root"]["leaf"]["resume"] = resume;
            assert!(session::interpret(&invalid.to_string()).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn every_client_reaches_the_child_with_only_fixed_arguments_and_a_structured_cwd() {
        use std::os::unix::fs::PermissionsExt;
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "ivy-resume-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let directory = root.join("directory with spaces;$literal");
        std::fs::create_dir(&directory).unwrap();
        let binary = root.join("client");
        let output = root.join("arguments");
        std::fs::write(
            &binary,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > '{}'\nexec /bin/cat\n",
                output.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        for client in [Client::Codex, Client::Claude, Client::Opencode] {
            let leaf = saved(client, directory.clone());
            let pane = try_open_with(&Default::default(), &leaf, 0, Default::default(), |_| {
                Ok(crate::ai_sessions::ResolvedLaunch {
                    program: binary.clone(),
                    prefix_args: vec![],
                    path: "/usr/bin:/bin".into(),
                })
            })
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let expected_args: Vec<&str> = match client {
                Client::Codex => vec!["resume", "--no-daemon", ID],
                Client::Claude => vec!["--resume", ID],
                Client::Opencode => vec!["--session", "ses_Exact123"],
            };
            let expected = std::iter::once(directory.to_str().unwrap())
                .chain(expected_args)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            while std::fs::read_to_string(&output).ok().as_deref() != Some(&expected)
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(2));
            }
            let actual = std::fs::read_to_string(&output).unwrap();
            assert_eq!(actual, expected);
            assert!(pane.keystrokes.is_empty());
            assert_eq!(pane.restoration.resume, leaf.resume);
            drop(pane);
            std::fs::remove_file(&output).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn failed_resume_retains_output_until_explicit_fresh_shell_takeover() {
        let leaf = saved(Client::Claude, std::env::temp_dir());
        let mut pane = try_open_with(&Default::default(), &leaf, 0, Default::default(), |_| {
            Ok(crate::ai_sessions::ResolvedLaunch {
                program: "/bin/sh".into(),
                prefix_args: vec![
                    "-c".into(),
                    "printf 'resume failed\\r\\n'; exit 7".into(),
                    "fixture".into(),
                ],
                path: "/usr/bin:/bin".into(),
            })
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            pane.pty_output.pick_up(
                &mut output,
                65536,
                crate::pty_output::Silences::of(&pane.pty),
            );
            inspect_restored_exit(&mut pane);
            if pane.restoration.failed {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(pane.restoration.failed);
        assert_eq!(pane.pty.exit_success(), Some(false));
        assert_eq!(pane.restoration.resume, leaf.resume);
        assert!(String::from_utf8_lossy(&output).contains("resume failed"));
        assert!(
            !finished(&pane, false),
            "a failed resume must keep its window open"
        );
        pane.screen.advance(b"\x1b[?2004h\x1b[?1h");
        assert!(pane.screen.bracketed_paste());
        take_over(&mut pane).unwrap();
        assert!(!pane.restoration.failed);
        assert!(pane.restoration.resume.is_none());
        assert!(!pane.screen.bracketed_paste());
        assert!(!pane.screen.application_cursor_keys());
        assert!(
            pane.pty.exit_success().is_none(),
            "the fresh shell is alive"
        );
        pane.pty.write(b"exit\r").unwrap();
    }

    #[test]
    fn attach_without_a_sample_cannot_replace_an_initial_resume_target() {
        let mut pane = crate::Pane::launching(&Default::default(), "/bin/cat", 0).unwrap();
        let identity = saved(Client::Claude, std::env::temp_dir()).resume;
        pane.restoration.resume = identity.clone();
        pane.restoration.resuming = true;
        capture(
            &mut pane,
            Some(crate::ai_sessions::Snapshot {
                identity: None,
                cwd: None,
                revision: 1,
                status: crate::ai_sessions::Status::Registered,
                sampled: false,
            }),
        );
        assert_eq!(pane.restoration.resume, identity);
        assert!(pane.restoration.resuming);
        capture(
            &mut pane,
            Some(crate::ai_sessions::Snapshot {
                identity: identity.clone(),
                cwd: Some(std::env::temp_dir()),
                revision: 2,
                status: crate::ai_sessions::Status::Registered,
                sampled: true,
            }),
        );
        assert_eq!(pane.restoration.resume, identity);
        assert!(!pane.restoration.resuming);
    }

    #[test]
    fn missing_client_directory_keeps_the_target_visible_until_takeover() {
        let missing =
            std::env::temp_dir().join(format!("ivy-missing-ai-directory-{}", std::process::id()));
        let leaf = saved(Client::Claude, missing);
        let mut pane = crate::restoration::open_leaf(
            &Default::default(),
            &leaf,
            "/bin/sh",
            0,
            Default::default(),
        )
        .unwrap();
        assert!(pane.restoration.failed);
        assert_eq!(pane.restoration.resume, leaf.resume);
        assert_eq!(
            crate::painter::notices::what_the_silence_is(&pane),
            Some(crate::painter::notices::Silence::RestoreDirectory)
        );
        pane.restoration.take_over();
        assert!(pane.restoration.resume.is_none());
        assert!(pane.restoration.directory.is_none());
    }
}
