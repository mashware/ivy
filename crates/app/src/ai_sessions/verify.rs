use super::programs::Program;
use panel_control::session::{Client, Peer};
use std::io;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use term_core::{ForegroundScope, NativeProcess};

pub(super) struct Root {
    pub(super) process: NativeProcess,
    pub(super) client: Client,
    pub(super) lifetime: Arc<OwnedFd>,
    check_mode: bool,
}

pub(super) fn root(scope: &ForegroundScope, peer: &Peer, programs: &[Program]) -> io::Result<Root> {
    if !peer.is_alive() {
        return Err(denied());
    }
    let chain = scope.inspect_descendant(peer.pid(), peer.birth())?;
    let mut selected: Option<(&Program, &NativeProcess, bool)> = None;
    for process in &chain.processes {
        for program in programs {
            let native = process.executable == program.native;
            let wrapper = program
                .wrapper
                .as_ref()
                .is_some_and(|(interpreter, script)| {
                    process.executable == *interpreter
                        && process.runs_script(script).unwrap_or(false)
                });
            if !native && !wrapper {
                continue;
            }
            if !process.foreground {
                return Err(denied());
            }
            if let Some((previous, previous_process, previous_wrapper)) = selected {
                // The supported npm wrapper's direct native child is one invocation; another
                // recognized client anywhere below it is a nested invocation, even of the same CLI.
                if !previous_wrapper
                    || wrapper
                    || previous.client != program.client
                    || process.parent != previous_process.pid
                {
                    return Err(denied());
                }
            }
            selected = Some((program, process, wrapper));
        }
    }
    let (program, process, wrapper) = selected.ok_or_else(denied)?;
    if wrapper {
        return Err(denied());
    }
    let check_mode = program.client == Client::Codex;
    #[cfg(test)]
    let check_mode = check_mode && !program.fixture;
    let lifetime = Arc::new(process.lifetime()?);
    let root = Root {
        process: process.clone(),
        client: program.client,
        lifetime,
        check_mode,
    };
    if !peer.is_alive() || !root.current(scope) {
        return Err(denied());
    }
    Ok(root)
}

impl Root {
    pub(super) fn working_directory(
        &self,
        scope: &ForegroundScope,
    ) -> io::Result<std::path::PathBuf> {
        if !self.current(scope) {
            return Err(denied());
        }
        let directory = self.process.working_directory()?;
        let text = directory.to_str().ok_or_else(denied)?;
        if panel_control::session::validate_cwd(text).is_err() || !self.current(scope) {
            return Err(denied());
        }
        Ok(directory)
    }

    pub(super) fn current(&self, scope: &ForegroundScope) -> bool {
        use std::os::fd::AsRawFd;
        let mut watched = libc::pollfd {
            fd: self.lifetime.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: this root owns the exact pidfd; the observation never waits.
        (unsafe { libc::poll(&mut watched, 1, 0) == 0 })
            && scope
                .inspect(self.process.pid, self.process.birth)
                .is_ok_and(|chain| {
                    chain
                        .processes
                        .last()
                        .is_some_and(|process| process.executable == self.process.executable)
                })
            && (!self.check_mode || codex_interactive(&self.process).unwrap_or(false))
    }
}

fn codex_interactive(process: &NativeProcess) -> io::Result<bool> {
    let shapes = codex_shapes();
    let shapes: Vec<_> = shapes.iter().map(Vec::as_slice).collect();
    process.matches_arguments(&shapes)
}

fn codex_shapes() -> Vec<Vec<term_core::Argument<'static>>> {
    use term_core::Argument::{Literal as L, Opaque, Uuid};
    let mut shapes = vec![
        vec![],
        vec![L("--no-daemon")],
        vec![L("resume"), Uuid],
        vec![L("resume"), L("--no-daemon"), Uuid],
        vec![L("--no-daemon"), L("resume"), Uuid],
    ];
    // Codex keeps --no-daemon authoritative with config overrides; their values are opaque.
    for pairs in 1..=4 {
        for aliases in 0..1 << pairs {
            let mut shape = vec![L("--no-daemon")];
            for index in 0..pairs {
                shape.push(L(if aliases & (1 << index) == 0 {
                    "-c"
                } else {
                    "--config"
                }));
                shape.push(Opaque { max_bytes: 8192 });
            }
            shapes.push(shape.clone());
            shape.extend([L("resume"), Uuid]);
            shapes.push(shape);
        }
    }
    shapes
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
