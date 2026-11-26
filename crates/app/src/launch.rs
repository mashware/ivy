//! A console reserves its place immediately; slow Linux scope setup finishes off the frame thread.
use std::ffi::OsStr;
#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
#[cfg(any(target_os = "linux", test))]
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use term_core::{Backlog, Ending, ForegroundScope, Pty, Size};

static LAUNCHER: OnceLock<PathBuf> = OnceLock::new();
const PENDING_INPUT_CEILING: usize = 256 * 1024;
type Spawned = (Pty, Receiver<Vec<u8>>);

#[derive(Debug)]
pub(crate) struct StartupFailure {
    pub program: String,
    pub reason: String,
    source: term_core::Error,
}

impl std::fmt::Display for StartupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.program, self.reason)
    }
}

impl std::error::Error for StartupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

struct Ready {
    spawned: Spawned,
    startup: Vec<u8>,
}

#[cfg(any(target_os = "linux", test))]
fn prepare(spawned: Spawned) -> Ready {
    let mut startup = Vec::new();
    // term-core holds at most eight 8192-byte reads. Capture that finite FIFO prefix on the
    // worker, never drain a growing producer or copy its prefix on the drawing thread.
    for chunk in spawned.1.try_iter().take(8) {
        startup.extend(chunk);
    }
    Ready { spawned, startup }
}

#[cfg(test)]
thread_local! {
    static TEST_LAUNCHER: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    pub(crate) static TEST_SPAWN_PROGRAM: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "linux")]
fn launcher() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(path) = TEST_LAUNCHER.with(|launcher| launcher.borrow().clone()) {
        return Some(path);
    }
    LAUNCHER.get().cloned()
}

/// Explicit opt-in by the Ivy executable, never by a library consumer's test runner.
pub fn enable(launcher: PathBuf) {
    let _ = LAUNCHER.set(launcher);
}

pub(crate) struct Process {
    // Revoke quota publication before waiting for PTY teardown.
    #[cfg(target_os = "linux")]
    quota_lease: Option<crate::usage::QuotaLease>,
    live: Option<Pty>,
    terminal: term_core::Termination,
    cost: std::time::Duration,
    pub(crate) context: Option<eframe::egui::Context>,
    pending: Option<Pending>,
    input: Vec<u8>,
    startup: usize,
    input_since: Option<std::time::Instant>,
    refused: bool,
    refusals: u64,
    pub(crate) failure: Option<String>,
    pub(crate) startup_failure: Option<StartupFailure>,
    delivery_failed: bool,
    pub(crate) dismissed: bool,
}

struct Pending {
    result: Receiver<term_core::Result<Ready>>,
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
            startup: 0,
            input_since: None,
            refused: false,
            refusals: 0,
            failure: None,
            startup_failure: None,
            delivery_failed: false,
            dismissed: false,
            #[cfg(target_os = "linux")]
            quota_lease: None,
        }
    }
}

#[cfg(test)]
impl Process {
    /// A console whose launch failed with what was typed still owed: its writer has given up for
    /// good, which is what a test needs that cannot wait for a real writer to fail.
    pub(crate) fn deaf() -> Self {
        Self {
            live: None,
            terminal: term_core::Termination::Pending,
            cost: std::time::Duration::ZERO,
            context: None,
            pending: None,
            input: b"typed".to_vec(),
            startup: 0,
            input_since: Some(std::time::Instant::now()),
            refused: false,
            refusals: 0,
            failure: Some("the program could not be started".into()),
            startup_failure: None,
            delivery_failed: false,
            dismissed: false,
            #[cfg(target_os = "linux")]
            quota_lease: None,
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
        Self::start_configured(
            wake,
            reads,
            request,
            #[cfg(target_os = "linux")]
            None,
        )
    }

    pub(crate) fn start_configured(
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
        reads: crate::control::ReadActivity,
        request: Request<'_>,
        #[cfg(target_os = "linux")] quota: Option<crate::usage::QuotaStartup>,
    ) -> term_core::Result<(Self, Receiver<Vec<u8>>)> {
        #[cfg(target_os = "linux")]
        let scope_launcher = launcher();
        #[cfg(target_os = "linux")]
        if scope_launcher.is_some() || quota.is_some() {
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
            let lease = quota.as_ref().map(crate::usage::QuotaStartup::lease);
            let (mut process, output) = start_worker(wake, reads, move || {
                if let Some(startup) = &quota
                    && let Err(error) = startup.verify()
                {
                    startup.fail();
                    return Err(Box::new(error));
                }
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
                let spawn = |program, args: &[&str]| match scope_launcher.as_deref() {
                    Some(launcher) => Pty::spawn_with_launcher_in(
                        launcher,
                        &identity,
                        program,
                        args,
                        size,
                        &borrowed,
                        directory.as_deref(),
                    ),
                    None => {
                        Pty::spawn_with_env_in(program, args, size, &borrowed, directory.as_deref())
                    }
                };
                let result = match spawn(&program, &args) {
                    Err(error)
                        if fallback && program != crate::shell_integration::fallback_shell() =>
                    {
                        let fallback = crate::shell_integration::fallback_shell();
                        crate::pty_output::say(&format!(
                            "'{program}' could not be launched ({error}); opening {fallback}"
                        ));
                        spawn(fallback, &[])
                    }
                    result => result,
                };
                if let Some(startup) = quota {
                    match &result {
                        Ok((pty, _)) => startup.bind(pty),
                        Err(_) => startup.fail(),
                    }
                }
                result
            })?;
            process.quota_lease = lease;
            return Ok((process, output));
        }
        let _ = (wake, reads);
        let spawn = |program: &str, args: &[&str]| {
            #[cfg(test)]
            let replacement = TEST_SPAWN_PROGRAM.with(|value| value.borrow().clone());
            #[cfg(test)]
            let program = replacement.as_deref().unwrap_or(program);
            Pty::spawn_with_env_in(program, args, request.size, request.env, request.directory)
                .map_err(|error| -> term_core::Error {
                    Box::new(StartupFailure {
                        program: program.to_owned(),
                        reason: error.to_string(),
                        source: error,
                    })
                })
        };
        let (pty, output) = match spawn(request.program, request.args) {
            Err(error)
                if request.fallback
                    && request.program != crate::shell_integration::fallback_shell() =>
            {
                let fallback = crate::shell_integration::fallback_shell();
                crate::pty_output::say(&format!(
                    "'{}' could not be launched ({error}); opening {fallback}",
                    request.program,
                ));
                spawn(fallback, &[])?
            }
            result => result?,
        };
        Ok((pty.into(), output))
    }

    pub(crate) fn failed(error: StartupFailure) -> (Self, Receiver<Vec<u8>>) {
        let (sender, output) = std::sync::mpsc::channel();
        drop(sender);
        let process = Self {
            live: None,
            terminal: term_core::Termination::Ordinary,
            cost: std::time::Duration::ZERO,
            context: None,
            pending: None,
            input: Vec::new(),
            startup: 0,
            input_since: None,
            refused: false,
            refusals: 0,
            failure: Some(error.to_string()),
            startup_failure: Some(error),
            delivery_failed: false,
            dismissed: false,
            #[cfg(target_os = "linux")]
            quota_lease: None,
        };
        (process, output)
    }

    pub(crate) fn live(&self) -> Option<&Pty> {
        self.live.as_ref()
    }
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn startup_pending(&self) -> bool {
        self.startup != 0
    }
    pub(crate) fn interpreted(&mut self, bytes: usize) {
        self.startup = self.startup.saturating_sub(bytes);
    }
    pub(crate) fn termination(&self) -> term_core::Termination {
        self.terminal
    }
    pub(crate) fn keeps_open(&self) -> bool {
        self.failure.is_some()
            || self.delivery_failed
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
            _ if self.delivery_failed => {
                Some("What you typed while this console was starting did not reach the program.")
            }
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
    pub(crate) fn can_queue_reply(&self) -> bool {
        self.live.as_ref().is_some_and(|live| {
            if self.startup_pending() || !self.input.is_empty() {
                !self.refused && self.input.len() < PENDING_INPUT_CEILING
            } else {
                live.can_queue_reply()
            }
        })
    }
    pub(crate) fn resize(&mut self, size: Size) -> term_core::Result<()> {
        if !self.startup_pending()
            && let Some(live) = &mut self.live
        {
            live.resize(size)?;
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) -> term_core::Result<()> {
        if !self.startup_pending()
            && self.input.is_empty()
            && let Some(live) = &mut self.live
        {
            return live.write(bytes);
        }
        if self.pending.is_none() && self.live.is_none() {
            return Err("the console could not be started".into());
        }
        // Match D60: accept one paste whole, refuse subsequent input while the queue is full.
        if self.input.len() >= PENDING_INPUT_CEILING {
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

    fn poll(&mut self) -> Option<(Receiver<Vec<u8>>, Vec<u8>, crate::control::ReadActivity)> {
        let result = match self.pending.as_ref()?.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("the console launch worker stopped".into()),
        };
        let pending = self.pending.take()?;
        match result {
            Ok(Ready {
                spawned: (pty, output),
                startup,
            }) => {
                self.live = Some(pty);
                self.startup = startup.len();
                Some((output, startup, pending.reads))
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
    let Some((output, startup, reads)) = pane.pty.poll() else {
        if pane.pty.failure.is_some() && pane.restoration.resuming {
            pane.restoration.failed = true;
            pane.restoration.resuming = false;
            pane.restoration.fresh_shell_required = true;
        }
        return;
    };
    let wake = pane.restoration.wake.clone();
    pane.pty_output = crate::pty_output::PtyOutput::seeded(output, startup, move || {
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
            pane.pty
                .context
                .as_ref()
                .and_then(crate::control::control_hwnd),
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
    pane.pty.terminal = pane.pty.live.as_ref().map_or_else(
        || {
            if pane.pty.pending() {
                term_core::Termination::Pending
            } else {
                term_core::Termination::Ordinary
            }
        },
        Pty::termination,
    );
    if pane.pty.live.is_some()
        && !pane.pty_output.is_alive()
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

pub(crate) fn finish_input(pane: &mut crate::Pane) {
    if pane.pty.startup_pending() || pane.pty.live.is_none() || pane.pty.input.is_empty() {
        return;
    }
    let before = std::time::Instant::now();
    let input = std::mem::take(&mut pane.pty.input);
    if let Err(error) = pane
        .pty
        .live
        .as_mut()
        .expect("installed console")
        .write(&input)
    {
        pane.pty.delivery_failed = true;
        crate::pty_output::say(&format!(
            "pending console input could not be delivered: {error}"
        ));
    }
    pane.pty.input_since = None;
    pane.pty.cost += before.elapsed();
}

#[cfg(any(target_os = "linux", test))]
struct CompletionWake(std::sync::Arc<dyn Fn() + Send + Sync>);
#[cfg(any(target_os = "linux", test))]
impl Drop for CompletionWake {
    fn drop(&mut self) {
        (self.0)();
    }
}

#[cfg(any(target_os = "linux", test))]
fn start_worker(
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    reads: crate::control::ReadActivity,
    operation: impl FnOnce() -> term_core::Result<Spawned> + Send + 'static,
) -> term_core::Result<(Process, Receiver<Vec<u8>>)> {
    let (sender, result) = channel();
    let (output_open, output) = channel();
    std::thread::Builder::new()
        .name("console-launch".into())
        .spawn(move || {
            let _completed = CompletionWake(wake);
            // On receiver cancellation the undelivered child is shut down on this worker.
            let deliver = sender;
            let _ = deliver.send(operation().map(prepare));
        })?;
    Ok((
        Process {
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
            startup: 0,
            input_since: None,
            refused: false,
            refusals: 0,
            failure: None,
            startup_failure: None,
            delivery_failed: false,
            dismissed: false,
            #[cfg(target_os = "linux")]
            quota_lease: None,
        },
        output,
    ))
}

#[cfg(test)]
pub(crate) mod tests;
