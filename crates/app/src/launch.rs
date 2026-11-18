//! A console reserves its place immediately; slow Linux scope setup finishes off the frame thread.
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use term_core::{Backlog, Ending, ForegroundScope, Pty, Size};

static LAUNCHER: OnceLock<PathBuf> = OnceLock::new();
type Spawned = (Pty, Receiver<Vec<u8>>);

/// Explicit opt-in by the Ivy executable, never by a library consumer's test runner.
pub fn enable(launcher: PathBuf) {
    let _ = LAUNCHER.set(launcher);
}

pub(crate) struct Process {
    live: Option<Pty>,
    terminal: term_core::Termination,
    cost: std::time::Duration,
    pub(crate) context: Option<eframe::egui::Context>,
    pending: Option<Pending>,
    input: Vec<u8>,
    input_since: Option<std::time::Instant>,
    refused: bool,
    refusals: u64,
    size: Size,
    pub(crate) failure: Option<String>,
    pub(crate) dismissed: bool,
}

struct Pending {
    result: Receiver<term_core::Result<Spawned>>,
    _output_open: Sender<Vec<u8>>,
    reads: crate::control::ReadActivity,
}

impl From<Pty> for Process {
    fn from(live: Pty) -> Self {
        Self {
            live: Some(live),
            terminal: term_core::Termination::Pending,
            cost: std::time::Duration::ZERO,
            context: None,
            pending: None,
            input: Vec::new(),
            input_since: None,
            refused: false,
            refusals: 0,
            size: crate::INITIAL_GRID,
            failure: None,
            dismissed: false,
        }
    }
}

pub(crate) struct Request<'a> {
    pub program: &'a str,
    pub args: &'a [&'a str],
    pub size: Size,
    pub env: &'a [(&'a str, Option<&'a OsStr>)],
    pub directory: Option<&'a Path>,
    pub fallback: bool,
}

impl Process {
    pub(crate) fn start(
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
        reads: crate::control::ReadActivity,
        request: Request<'_>,
    ) -> term_core::Result<(Self, Receiver<Vec<u8>>)> {
        #[cfg(target_os = "linux")]
        if let Some(launcher) = LAUNCHER.get() {
            let launcher = launcher.clone();
            let program = request.program.to_owned();
            let args: Vec<String> = request.args.iter().map(|arg| (*arg).to_owned()).collect();
            let env: Vec<(String, Option<OsString>)> = request
                .env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.map(OsStr::to_owned)))
                .collect();
            let directory = request.directory.map(Path::to_owned);
            let size = request.size;
            let fallback = request.fallback;
            let (sender, result) = channel();
            let (output_open, output) = channel();
            std::thread::Builder::new()
                .name("console-launch".into())
                .spawn(move || {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    let borrowed: Vec<(&str, Option<&OsStr>)> = env
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_deref()))
                        .collect();
                    let identity = env
                        .iter()
                        .find(|(key, _)| key == "PANEL_ID")
                        .and_then(|(_, value)| value.as_ref())
                        .map(|v| v.to_string_lossy())
                        .unwrap_or_else(|| "console".into());
                    let spawn = |program, args: &[&str]| {
                        Pty::spawn_with_launcher_in(
                            &launcher,
                            &identity,
                            program,
                            args,
                            size,
                            &borrowed,
                            directory.as_deref(),
                        )
                    };
                    let spawned = match spawn(&program, &args) {
                        Err(error) if fallback => {
                            eprintln!(
                                "'{program}' could not be launched ({error}); opening {}",
                                crate::FALLBACK_SHELL
                            );
                            spawn(crate::FALLBACK_SHELL, &[])
                        }
                        result => result,
                    };
                    // A dropped pane owns no receiver: the undelivered child is shut down here.
                    let _ = sender.send(spawned);
                    wake();
                })?;
            return Ok((
                Self {
                    live: None,
                    terminal: term_core::Termination::Pending,
                    cost: std::time::Duration::ZERO,
                    context: None,
                    pending: Some(Pending {
                        result,
                        _output_open: output_open,
                        reads,
                    }),
                    input: Vec::new(),
                    input_since: None,
                    refused: false,
                    refusals: 0,
                    size,
                    failure: None,
                    dismissed: false,
                },
                output,
            ));
        }
        let _ = (wake, reads);
        let spawn = |program, args: &[&str]| {
            Pty::spawn_with_env_in(program, args, request.size, request.env, request.directory)
        };
        let (pty, output) = match spawn(request.program, request.args) {
            Err(error) if request.fallback => {
                eprintln!(
                    "'{}' could not be launched ({error}); opening {}",
                    request.program,
                    crate::FALLBACK_SHELL
                );
                spawn(crate::FALLBACK_SHELL, &[])?
            }
            result => result?,
        };
        Ok((pty.into(), output))
    }

    pub(crate) fn live(&self) -> Option<&Pty> {
        self.live.as_ref()
    }
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn termination(&self) -> term_core::Termination {
        self.terminal
    }
    pub(crate) fn keeps_open(&self) -> bool {
        self.failure.is_some()
            || self.pending()
            || self.termination() != term_core::Termination::Ordinary
    }
    pub(crate) fn notice(&self) -> Option<&str> {
        if self.dismissed {
            return None;
        }
        if self.failure.is_some() {
            return Some("This console's program could not be started.");
        }
        match self.termination() {
            term_core::Termination::Oom => {
                Some("This console was stopped because of memory pressure.")
            }
            term_core::Termination::Signal => Some("This console was terminated by a signal."),
            _ => None,
        }
    }
    pub(crate) fn foreground_scope(&self) -> std::io::Result<ForegroundScope> {
        self.live
            .as_ref()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?
            .foreground_scope()
    }
    pub(crate) fn exit_success(&mut self) -> Option<bool> {
        self.live.as_mut()?.exit_success()
    }
    pub(crate) fn working_directory(&mut self) -> Option<PathBuf> {
        self.live.as_mut()?.working_directory()
    }
    pub(crate) fn foreground_program(&mut self) -> Option<String> {
        self.live.as_mut()?.foreground_program()
    }
    pub(crate) fn ending(&self) -> Option<Ending> {
        self.live.as_ref()?.ending()
    }
    pub(crate) fn backlog(&self) -> Option<Backlog> {
        if let Some(live) = &self.live {
            return live.backlog();
        }
        self.input_since.map(|since| Backlog {
            bytes: self.input.len(),
            waiting: since.elapsed(),
            gave_up: self.failure.is_some(),
            refused: self.refused,
            refusals: self.refusals,
        })
    }
    pub(crate) fn resize(&mut self, size: Size) -> term_core::Result<()> {
        self.size = size;
        if let Some(live) = &mut self.live {
            live.resize(size)?;
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) -> term_core::Result<()> {
        if let Some(live) = &mut self.live {
            return live.write(bytes);
        }
        if self.pending.is_none() {
            return Err("the console could not be started".into());
        }
        // Match D60: accept one paste whole, refuse subsequent input while the queue is full.
        if self.input.len() >= 256 * 1024 {
            self.refused = true;
            self.refusals += 1;
            return Err(
                "the pending console's input queue is full; input did not reach the program".into(),
            );
        }
        if !bytes.is_empty() {
            self.input_since.get_or_insert_with(std::time::Instant::now);
        }
        self.refused = false;
        self.input.extend_from_slice(bytes);
        Ok(())
    }

    fn poll(&mut self) -> Option<(Receiver<Vec<u8>>, crate::control::ReadActivity)> {
        let result = match self.pending.as_ref()?.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("the console launch worker stopped".into()),
        };
        let pending = self.pending.take()?;
        match result {
            Ok((mut pty, output)) => {
                let install = pty.resize(self.size).and_then(|()| {
                    if self.input.is_empty() {
                        Ok(())
                    } else {
                        pty.write(&self.input)
                    }
                });
                self.input.clear();
                if let Err(error) = install {
                    self.failure = Some(error.to_string());
                }
                self.live = Some(pty);
                Some((output, pending.reads))
            }
            Err(error) => {
                self.failure = Some(error.to_string());
                self.input.clear();
                None
            }
        }
    }
}

pub(crate) fn complete(pane: &mut crate::Pane) {
    let Some((output, reads)) = pane.pty.poll() else {
        if pane.pty.failure.is_some() && pane.restoration.resuming {
            pane.restoration.failed = true;
            pane.restoration.resuming = false;
            pane.restoration.fresh_shell_required = true;
        }
        return;
    };
    let wake = pane.restoration.wake.clone();
    pane.pty_output = crate::pty_output::PtyOutput::new(output, move || {
        if let Some(wake) = &wake {
            wake();
        }
    });
    pane.restoration.scope_unavailable = false;
    if pane.control.is_some() {
        return;
    }
    let callback = pane.restoration.route.callback();
    let inspector = pane
        .pty
        .foreground_scope()
        .map(crate::panel::provenance_route::inspector)
        .unwrap_or_else(|_| crate::panel::provenance_route::unavailable());
    pane.control = pane.panel.as_ref().and_then(|dir| {
        let wake = pane.restoration.wake.clone();
        crate::control::open_with_authority(
            dir,
            &pane.panel_state,
            reads,
            Some(callback),
            pane.pty
                .foreground_scope()
                .ok()
                .map(crate::panel::overlay::Authority::new),
            inspector,
            move || {
                if let Some(wake) = &wake {
                    wake();
                }
            },
        )
    });
    pane.restoration.scope_unavailable = false;
}

pub(crate) fn silences(process: &Process) -> crate::pty_output::Silences {
    process
        .live()
        .map(crate::pty_output::Silences::of)
        .unwrap_or_default()
}

impl crate::Pane {
    /// Applies the size the last UI pass measured.
    ///
    /// **The order between the two halves is not indifferent: the grid first and the program
    /// after.** As soon as the program knows the new size it starts sending bytes meant for it, and
    /// if the grid were still the old one they would be interpreted against the wrong dimensions.
    pub(crate) fn resize(&mut self) {
        crate::pty_output::resize(self);
    }

    /// Whether the program finished **well** and nothing of its own is left to show.
    ///
    /// **It waits for the queue to be empty**: dying does not erase what the program wrote before
    /// doing so, and giving the pane up for finished with a queue pending would eat precisely the
    /// end, which is the part that was being looked at.
    ///
    /// **What «finished» means is a decision and it lives outside this file**, in the module that
    /// owns the signal it rests on: see [`crate::pty_output::is_finished`]. **The ending is asked of the
    /// pseudoterminal** and not of anything the bridge keeps, so it does not depend on the pump
    /// having run this frame — which the queue, the other half of the question, does by definition.
    pub(crate) fn is_finished(&self, queue_left: bool) -> bool {
        crate::resume::finished(self, queue_left)
    }
}

#[derive(Default)]
pub(crate) struct Cost {
    pub calls: usize,
    pub total: std::time::Duration,
}

pub(crate) fn refresh(pane: &mut crate::Pane) {
    let before = std::time::Instant::now();
    complete(pane);
    pane.pty.terminal = pane
        .pty
        .live
        .as_ref()
        .map_or(term_core::Termination::Pending, Pty::termination);
    if !pane.pty_output.is_alive()
        && pane.pty.termination() == term_core::Termination::Pending
        && let Some(ctx) = &pane.pty.context
    {
        ctx.request_repaint_after(std::time::Duration::from_millis(20));
    }
    pane.pty.cost += before.elapsed();
}

pub(crate) fn take_cost(process: &mut Process) -> std::time::Duration {
    std::mem::take(&mut process.cost)
}

#[cfg(test)]
mod tests;
