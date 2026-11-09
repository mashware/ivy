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
    let chain = scope.inspect(peer.pid(), peer.birth())?;
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
            if process.group != chain.foreground_group {
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
    if check_mode && !codex_interactive(process)? {
        return Err(denied());
    }
    let lifetime = Arc::new(process.lifetime()?);
    if !peer.is_alive() || !scope.is_open() {
        return Err(denied());
    }
    Ok(Root {
        process: process.clone(),
        client: program.client,
        lifetime,
        check_mode,
    })
}

impl Root {
    pub(super) fn current(&self, scope: &ForegroundScope) -> bool {
        scope
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
    use term_core::Argument::{Literal as L, Uuid};
    process.matches_arguments(&[
        &[],
        &[L("--no-daemon")],
        &[L("resume"), Uuid],
        &[L("resume"), L("--no-daemon"), Uuid],
        &[L("--no-daemon"), L("resume"), Uuid],
    ])
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
