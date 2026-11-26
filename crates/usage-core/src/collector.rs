//! Background ownership of local Claude history and read-only usage snapshots.

mod scan;
mod summary;

use crate::Provider;
use crate::confined::{self, Binding, Discovery, Root};
use crate::control::{Consent, Control};
use crate::ledger::{Error as LedgerError, Ledger};
use crate::pricing::Catalog;
use chrono::{NaiveDate, Utc};
use std::collections::VecDeque;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_COMMANDS: usize = 8;
const QUERY_INTERVAL: Duration = Duration::from_secs(1);
const SWEEP_INTERVAL: Duration = Duration::from_secs(20);
const IDLE_POLL: Duration = Duration::from_millis(200);
const DIRTY_SOURCES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DateRange {
    pub start_ms: i64,
    pub end_ms: i64,
}

impl DateRange {
    fn valid(self) -> bool {
        let Some(first) = chrono::DateTime::from_timestamp_millis(self.start_ms) else {
            return false;
        };
        let Some(last) = chrono::DateTime::from_timestamp_millis(self.end_ms.saturating_sub(1))
        else {
            return false;
        };
        self.start_ms < self.end_ms
            && (0..366).contains(
                &last
                    .with_timezone(&chrono::Local)
                    .date_naive()
                    .signed_duration_since(first.with_timezone(&chrono::Local).date_naive())
                    .num_days(),
            )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
    #[default]
    Disabled,
    Importing,
    Ready,
    Empty,
    Partial,
    Limit,
    WaitingToClear,
    Unsupported,
    Incompatible,
    Error,
    Stale,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cost {
    pub usd: f64,
    pub partial: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub uncached_input: u128,
    pub cache_read: u128,
    pub cache_write: u128,
    pub output: u128,
    pub reasoning_output: u128,
    pub reasoning_output_known: bool,
    pub tokens: u128,
    pub tokens_known: bool,
    pub uncached_input_known: bool,
    pub cache_read_known: bool,
    pub cache_write_known: bool,
    pub output_known: bool,
    pub partial: bool,
    pub estimate: Option<Cost>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Granularity {
    Hour,
    #[default]
    Day,
    Month,
}

#[derive(Clone, Debug, PartialEq)]
struct Request {
    range: DateRange,
    provider: Option<Provider>,
    model: Option<String>,
    granularity: Granularity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bucket {
    pub label: String,
    pub totals: Totals,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub totals: Totals,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub provider: Provider,
    pub label: String,
    pub totals: Totals,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub status: Status,
    pub enabled: bool,
    pub roots: Vec<PathBuf>,
    pub codex_roots: Vec<PathBuf>,
    pub codex_enabled: bool,
    pub codex_last_success_at_ms: Option<i64>,
    pub range: Option<DateRange>,
    pub provider: Option<Provider>,
    pub model_filter: Option<String>,
    pub granularity: Granularity,
    pub buckets: Vec<Bucket>,
    pub last_success_at_ms: Option<i64>,
    pub pricing_version: String,
    pub pricing_source: String,
    pub has_history: bool,
    pub totals: Totals,
    pub daily: Vec<Day>,
    pub models: Vec<Model>,
}

enum Command {
    Enable(Vec<PathBuf>, Option<[u8; 16]>, Provider),
    Stop,
    Clear,
}

#[derive(Default)]
struct Mailbox {
    generation: u64,
    cancel: Arc<AtomicBool>,
    commands: VecDeque<Command>,
    range: Option<Request>,
    reply: Option<(Snapshot, Option<[u8; 16]>)>,
    observed_clear: Option<[u8; 16]>,
    finish: bool,
}

impl Mailbox {
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.reply = None;
    }
}

#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    changed: Condvar,
}

pub struct Service {
    shared: Arc<Shared>,
}

impl Service {
    /// Start one local worker. A fresh view neither creates a store nor discovers provider files.
    pub fn new(cache_dir: PathBuf, repaint: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Shared::default());
        let work = Arc::clone(&shared);
        let handler = thread::Builder::new()
            .name("ivy-usage".into())
            .spawn(move || Worker::new(cache_dir).run(&work, repaint));
        if handler.is_err() {
            let mut mailbox = shared.mailbox.lock().unwrap();
            mailbox.reply = Some((
                Snapshot {
                    status: Status::Error,
                    ..Snapshot::default()
                },
                None,
            ));
            mailbox.finish = true;
        }
        Self { shared }
    }

    pub fn enable(&self, roots: Vec<PathBuf>) -> bool {
        self.command(|mailbox| Command::Enable(roots, mailbox.observed_clear, Provider::Claude))
    }

    pub fn enable_codex(&self, roots: Vec<PathBuf>) -> bool {
        self.command(|mailbox| Command::Enable(roots, mailbox.observed_clear, Provider::Codex))
    }

    pub fn stop(&self) -> bool {
        self.command(|_| Command::Stop)
    }

    pub fn clear(&self) -> bool {
        self.command(|_| Command::Clear)
    }

    fn command(&self, make: impl FnOnce(&Mailbox) -> Command) -> bool {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.finish || mailbox.commands.len() >= MAX_COMMANDS {
            return false;
        }
        mailbox.invalidate();
        let command = make(&mailbox);
        mailbox.commands.push_back(command);
        self.shared.changed.notify_one();
        true
    }

    pub fn request(&self, range: DateRange) {
        self.request_comparison(range, None, None, Granularity::Day);
    }

    pub fn request_comparison(
        &self,
        range: DateRange,
        provider: Option<Provider>,
        model: Option<String>,
        granularity: Granularity,
    ) {
        if !range.valid()
            || granularity == Granularity::Hour
                && range.end_ms.saturating_sub(range.start_ms) > 8 * 86_400_000
        {
            return;
        }
        let request = Some(Request {
            range,
            provider,
            model,
            granularity,
        });
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.range != request && !mailbox.finish {
            mailbox.invalidate();
            mailbox.range = request;
            self.shared.changed.notify_one();
        }
    }

    pub fn poll(&self) -> Option<Snapshot> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        let (reply, generation) = mailbox.reply.take()?;
        mailbox.observed_clear = generation;
        self.shared.changed.notify_one();
        Some(reply)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        mailbox.invalidate();
        mailbox.finish = true;
        self.shared.changed.notify_one();
    }
}

struct Writer {
    ledger: Ledger,
    _lifecycle: File,
    _owner: File,
    recovery: bool,
    codex_ready: bool,
}

struct SweepRoot {
    provider: Provider,
    opened: Root,
    discovery: Discovery,
    key: [u8; 32],
    dirty: VecDeque<PathBuf>,
}

#[derive(Default)]
struct Sweep {
    next_root: usize,
    current: Option<SweepRoot>,
    scan: Option<scan::Scan>,
    resume: Option<scan::Resume>,
    next_resume: Option<scan::Resume>,
    providers: [SweepProgress; 2],
    complete: bool,
}

#[derive(Default)]
struct SweepProgress {
    discovered: u64,
    partial: bool,
    more: bool,
    reported: bool,
}

const PROVIDERS: [Provider; 2] = [Provider::Claude, Provider::Codex];

impl Sweep {
    fn more(&self) -> bool {
        self.providers.iter().any(|state| state.more)
    }

    fn partial(&self) -> bool {
        self.providers.iter().any(|state| state.partial)
    }

    fn step(
        &mut self,
        writer: &mut Writer,
        control: &Control,
        consent: &Consent,
    ) -> Result<(), scan::Error> {
        if self.complete {
            return Ok(());
        }
        if self.current.is_none() {
            let claude = if consent.enabled {
                consent.roots.as_slice()
            } else {
                &[]
            };
            let codex = if consent.codex_enabled {
                consent.codex_roots.as_slice()
            } else {
                &[]
            };
            let Some((binding, provider)) = claude
                .iter()
                .map(|root| (root, Provider::Claude))
                .chain(codex.iter().map(|root| (root, Provider::Codex)))
                .nth(self.next_root)
            else {
                self.complete = true;
                return Ok(());
            };
            self.next_root += 1;
            let opened = match Root::open(binding.clone()) {
                Ok(root) => root,
                Err(_) => {
                    self.providers[provider as usize].partial = true;
                    return Ok(());
                }
            };
            let discovery = match opened.discover() {
                Ok(discovery) => discovery,
                Err(_) => {
                    self.providers[provider as usize].partial = true;
                    return Ok(());
                }
            };
            let salt = writer.ledger.info()?.identity_salt;
            self.current = Some(SweepRoot {
                provider,
                opened,
                discovery,
                key: scan::root_key(binding, &salt),
                dirty: VecDeque::new(),
            });
        }

        let root = self.current.as_mut().expect("initialized above");
        if let Some(active) = &mut self.scan {
            match active.step(&root.opened, &mut writer.ledger) {
                Ok(scan::Progress::Work) => {}
                Ok(scan::Progress::Unchanged) => self.scan = None,
                Ok(scan::Progress::Publish) => {
                    let _consent = control.approve_read(consent)?;
                    let active = self.scan.take().expect("active source required to publish");
                    let (more, resume) = active.publish(&root.opened, &mut writer.ledger)?;
                    self.providers[root.provider as usize].more |= more;
                    if resume.is_some() {
                        self.next_resume = resume;
                    }
                }
                Err(error) => return Err(error),
            }
            return Ok(());
        }

        // Keep a bounded queue and resume the discovery cursor when room opens.
        for _ in 0..DIRTY_SOURCES {
            if root.dirty.len() == DIRTY_SOURCES || root.discovery.finished() {
                break;
            }
            if let Some(path) = root.discovery.next(1) {
                if path.extension().is_some_and(|extension| extension == "zst") {
                    if root.provider == Provider::Codex {
                        self.providers[Provider::Codex as usize].partial = true;
                    }
                } else if accepts_source(root.provider, &path) && !root.dirty.contains(&path) {
                    root.dirty.push_back(path);
                }
            }
        }
        if let Some(path) = root.dirty.pop_front() {
            self.providers[root.provider as usize].discovered += 1;
            let salt = writer.ledger.info()?.identity_salt;
            self.scan = Some(scan::Scan::new(
                &root.opened,
                path,
                root.key,
                salt,
                &writer.ledger,
                self.resume.as_ref(),
                root.provider,
            )?);
        } else if root.discovery.finished() {
            self.providers[root.provider as usize].partial |= root.discovery.partial;
            self.current = None;
        }
        Ok(())
    }
}

struct Worker {
    control: Control,
    consent: Consent,
    writer: Option<Writer>,
    sweep: Sweep,
    next_sweep: Instant,
    next_query: Instant,
    last_generation: u64,
    last: Option<Snapshot>,
    // The worker drops file locks off-frame after the mailbox is read or replaced.
    reply_guard: Option<File>,
    pending_action: Option<Command>,
    failure: Option<Status>,
    last_store: Option<[u8; 32]>,
}

impl Worker {
    fn new(cache_dir: PathBuf) -> Self {
        Self {
            control: Control::new(cache_dir),
            consent: Consent::default(),
            writer: None,
            sweep: Sweep::default(),
            next_sweep: Instant::now(),
            next_query: Instant::now(),
            last_generation: u64::MAX,
            last: None,
            reply_guard: None,
            pending_action: None,
            failure: None,
            last_store: None,
        }
    }

    fn run(mut self, shared: &Shared, repaint: impl Fn()) {
        loop {
            let (generation, range, cancel) = {
                let mut mailbox = shared.mailbox.lock().unwrap();
                if mailbox.finish {
                    return;
                }
                if mailbox.reply.is_none() {
                    self.reply_guard = None;
                }
                if self.pending_action.is_none() {
                    self.pending_action = mailbox.commands.pop_front();
                }
                (
                    mailbox.generation,
                    mailbox.range.clone(),
                    Arc::clone(&mailbox.cancel),
                )
            };

            let now = Instant::now();
            let new_request = self.last_generation != generation;
            if new_request {
                self.last_generation = generation;
                self.next_query = now;
                self.last = None;
            }

            let delay = match self.tick(shared, generation, range, cancel, &repaint) {
                Ok(delay) => delay,
                Err(error) => {
                    self.writer = None;
                    let status = if error.kind() == io::ErrorKind::Unsupported {
                        Status::Unsupported
                    } else {
                        Status::Error
                    };
                    self.deliver_status(shared, generation, status, &repaint);
                    IDLE_POLL
                }
            };
            if !delay.is_zero() {
                let mailbox = shared.mailbox.lock().unwrap();
                if !mailbox.finish && mailbox.generation == generation {
                    drop(shared.changed.wait_timeout(mailbox, delay).unwrap());
                }
            }
        }
    }

    fn tick(
        &mut self,
        shared: &Shared,
        generation: u64,
        range: Option<Request>,
        cancel: Arc<AtomicBool>,
        repaint: &impl Fn(),
    ) -> io::Result<Duration> {
        if !confined::supported() {
            self.deliver_status(shared, generation, Status::Unsupported, repaint);
            self.pending_action = None;
            return Ok(IDLE_POLL);
        }
        if self.control.pending_clear()? {
            if matches!(self.pending_action, Some(Command::Enable(..))) {
                self.pending_action = None;
            }
            shared
                .mailbox
                .lock()
                .unwrap()
                .commands
                .retain(|command| !matches!(command, Command::Enable(..)));
            if self.writer.is_some()
                || self.last_store.is_some()
                || self.reply_guard.is_some()
                || self.last.as_ref().is_some_and(|last| last.has_history)
            {
                self.discard(shared);
            }
            match self.control.complete_clear() {
                Ok(()) => {
                    self.consent = self.control.read()?;
                    self.last = None;
                    self.deliver_status(shared, generation, Status::Disabled, repaint);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.deliver_status(shared, generation, Status::WaitingToClear, repaint)
                }
                Err(error) => return Err(error),
            }
            return Ok(IDLE_POLL);
        }
        if let Some(command) = self.pending_action.take() {
            let result = match &command {
                Command::Enable(paths, observed, provider) => {
                    self.control
                        .enable_provider(paths.clone(), *observed, *provider)
                }
                Command::Stop => self.control.stop().map(|()| true),
                Command::Clear => self.control.begin_clear().map(|()| true),
            };
            match result {
                Ok(false) => self.next_query = Instant::now(),
                Ok(true) => {
                    if matches!(command, Command::Clear) {
                        self.discard(shared);
                        self.deliver_status(shared, generation, Status::WaitingToClear, repaint);
                        return Ok(Duration::ZERO);
                    }
                    self.failure = None;
                    self.next_query = Instant::now();
                    if !shared.mailbox.lock().unwrap().commands.is_empty() {
                        return Ok(Duration::ZERO);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.pending_action = Some(command);
                    return Ok(IDLE_POLL);
                }
                Err(error) => {
                    self.failure = Some(if error.kind() == io::ErrorKind::Unsupported {
                        Status::Unsupported
                    } else {
                        Status::Error
                    });
                    self.next_query = Instant::now();
                }
            }
        }

        let consent = self.control.read()?;
        if consent != self.consent {
            self.writer = None;
            self.sweep = Sweep::default();
            self.consent = consent;
            self.last = None;
            self.last_store = None;
            self.next_query = Instant::now();
            self.next_sweep = Instant::now();
        }
        let mut busy = false;
        if (self.consent.enabled || self.consent.codex_enabled)
            && (self.failure != Some(Status::Limit)
                || self.writer.as_ref().is_some_and(|writer| writer.recovery))
        {
            busy = self.collect()?;
        }
        if Instant::now() >= self.next_query {
            self.next_query = Instant::now() + QUERY_INTERVAL;
            self.read_and_deliver(shared, generation, range, cancel, repaint)?;
        }
        Ok(if busy { Duration::ZERO } else { IDLE_POLL })
    }

    fn collect(&mut self) -> io::Result<bool> {
        if self.writer.is_none() {
            let owner = match self.control.owner() {
                Ok(lock) => lock,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) => return Err(error),
            };
            let lifecycle = match self.control.reader(true) {
                Ok(lock) => lock,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) => return Err(error),
            };
            let _consent = match self.control.approve_read(&self.consent) {
                Ok(lock) => lock,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) => return Err(error),
            };
            let path = self.control.database(true)?;
            let ledger = match Ledger::open_writer(&path) {
                Ok(ledger) => ledger,
                Err(error) => {
                    self.failure = Some(ledger_status(&error));
                    return Ok(false);
                }
            };
            self.writer = Some(Writer {
                ledger,
                _lifecycle: lifecycle,
                _owner: owner,
                recovery: true,
                codex_ready: false,
            });
        }

        let writer = self.writer.as_mut().expect("worker owns the collector");
        if writer.recovery {
            match writer.ledger.recover_slice() {
                Ok(done) => writer.recovery = !done,
                Err(error) => {
                    self.failure = Some(ledger_status(&error));
                    self.writer = None;
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        if self.consent.codex_enabled && !writer.codex_ready {
            let _consent = match self.control.approve_read(&self.consent) {
                Ok(lock) => lock,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) => return Err(error),
            };
            if let Err(error) = writer.ledger.upgrade_for_codex() {
                self.failure = Some(ledger_status(&error));
                return Ok(false);
            }
            writer.codex_ready = true;
        }

        if self.sweep.complete {
            if Instant::now() < self.next_sweep {
                return match writer.ledger.reap_slice() {
                    Ok(done) => Ok(!done),
                    Err(error) => {
                        self.failure = Some(ledger_status(&error));
                        Ok(false)
                    }
                };
            }
            self.sweep = Sweep {
                resume: self.sweep.next_resume.take(),
                ..Sweep::default()
            };
        }
        if let Err(error) = self.sweep.step(writer, &self.control, &self.consent) {
            writer.recovery = true;
            self.sweep.scan = None;
            self.sweep.resume = None;
            self.sweep.next_resume = None;
            if let Some(root) = &self.sweep.current {
                self.sweep.providers[root.provider as usize].partial = true;
            }
            match error {
                scan::Error::Io(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                scan::Error::Io(_) => {}
                scan::Error::Ledger(error) => self.failure = Some(ledger_status(&error)),
            }
            self.next_query = Instant::now();
            return Ok(false);
        }
        if self.sweep.complete {
            self.sweep.resume = None;
            if !self.sweep.partial() && !self.sweep.more() && self.failure == Some(Status::Stale) {
                self.failure = None;
            }
            self.next_sweep = if self.sweep.more() {
                Instant::now() + IDLE_POLL
            } else {
                Instant::now() + SWEEP_INTERVAL
            };
            self.next_query = Instant::now();
        }
        Ok(!self.sweep.complete)
    }

    fn read_and_deliver(
        &mut self,
        shared: &Shared,
        generation: u64,
        range: Option<Request>,
        cancel: Arc<AtomicBool>,
        repaint: &impl Fn(),
    ) -> io::Result<()> {
        let fallback =
            self.failure
                .unwrap_or(if self.consent.enabled || self.consent.codex_enabled {
                    Status::Importing
                } else {
                    Status::Disabled
                });
        let Some(request) = range else {
            self.deliver_status(shared, generation, fallback, repaint);
            return Ok(());
        };
        let range = request.range;
        let (guard, path) = match self
            .control
            .reader(false)
            .and_then(|guard| self.control.database(false).map(|path| (guard, path)))
        {
            Ok(pair) => pair,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.last_store = None;
                self.deliver_status(shared, generation, fallback, repaint);
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        };
        let result = (|| {
            let mut ledger = Ledger::open_reader(&path)?;
            let store = ledger.info()?.store_id;
            if self.last_store.is_some_and(|previous| previous != store) {
                self.last = None;
            }
            self.last_store = Some(store);
            summary::read(&mut ledger, &request, cancel)
        })();
        let mut snapshot = match result {
            Ok(summary) => {
                let mut snapshot = summary.snapshot;
                let enabled = [self.consent.enabled, self.consent.codex_enabled];
                let mut source_partial = false;
                let mut more = false;
                for provider in PROVIDERS {
                    let i = provider as usize;
                    let state = &mut self.sweep.providers[i];
                    let partial = state.partial
                        || self.sweep.complete
                            && enabled[i]
                            && summary.source_counts[i] > state.discovered;
                    if request.provider.is_none_or(|selected| selected == provider) {
                        source_partial |= partial;
                        more |= state.more;
                    }
                    if self.sweep.complete
                        && enabled[i]
                        && !state.reported
                        && !state.more
                        && !partial
                        && self.failure.is_none()
                        && let Some(writer) = self.writer.as_mut()
                        && let Ok(_consent) = self.control.approve_read(&self.consent)
                    {
                        let now = Utc::now().timestamp_millis();
                        match writer.ledger.mark_provider_refresh_success(provider, now) {
                            Ok(()) => {
                                match provider {
                                    Provider::Claude => snapshot.last_success_at_ms = Some(now),
                                    Provider::Codex => {
                                        snapshot.codex_last_success_at_ms = Some(now)
                                    }
                                }
                                state.reported = true;
                            }
                            Err(error) => self.failure = Some(ledger_status(&error)),
                        }
                    }
                }
                let selected_enabled = request.provider.map_or(
                    self.consent.enabled || self.consent.codex_enabled,
                    |provider| enabled[provider as usize],
                );
                let importing =
                    selected_enabled && self.writer.is_some() && (!self.sweep.complete || more);
                let missing_refresh = match request.provider {
                    Some(Provider::Claude) => snapshot.last_success_at_ms.is_none(),
                    Some(Provider::Codex) => snapshot.codex_last_success_at_ms.is_none(),
                    None => {
                        (self.consent.enabled && snapshot.last_success_at_ms.is_none())
                            || (self.consent.codex_enabled
                                && snapshot.codex_last_success_at_ms.is_none())
                    }
                };
                let coverage_pending =
                    selected_enabled && (importing || self.writer.is_none() || missing_refresh);
                let partial = source_partial || summary.partial || coverage_pending;
                snapshot.totals.partial |= partial;
                if let Some(cost) = &mut snapshot.totals.estimate {
                    cost.partial |= snapshot.totals.partial;
                }
                snapshot.status = self.failure.unwrap_or(if !selected_enabled {
                    Status::Disabled
                } else if importing {
                    Status::Importing
                } else if partial {
                    Status::Partial
                } else if !summary.has_events {
                    Status::Empty
                } else {
                    Status::Ready
                });
                snapshot
            }
            Err(LedgerError::Cancelled) => return Ok(()),
            Err(error) => {
                let mut snapshot = self
                    .last
                    .clone()
                    .filter(|previous| {
                        previous.range == Some(range)
                            && previous.provider == request.provider
                            && previous.model_filter == request.model
                            && previous.granularity == request.granularity
                    })
                    .unwrap_or_default();
                snapshot.status = self.failure.unwrap_or(if snapshot.range.is_some() {
                    Status::Stale
                } else {
                    ledger_status(&error)
                });
                snapshot
            }
        };
        snapshot.enabled = self.consent.enabled;
        snapshot.roots = selected_paths(&self.consent.roots);
        snapshot.codex_enabled = self.consent.codex_enabled;
        snapshot.codex_roots = selected_paths(&self.consent.codex_roots);
        self.deliver(shared, generation, snapshot, Some(guard), repaint);
        Ok(())
    }

    fn discard(&mut self, shared: &Shared) {
        self.writer = None;
        self.sweep = Sweep::default();
        self.consent = Consent::default();
        self.last_store = None;
        self.failure = None;
        self.last = None;
        // A snapshot still in the mailbox is revoked while the lock is held; a
        // consumed snapshot has already been delivered before clear can finish.
        shared.mailbox.lock().unwrap().reply = None;
        self.reply_guard = None;
    }

    fn deliver_status(
        &mut self,
        shared: &Shared,
        generation: u64,
        status: Status,
        repaint: &impl Fn(),
    ) {
        self.deliver(
            shared,
            generation,
            Snapshot {
                status,
                enabled: self.consent.enabled,
                roots: selected_paths(&self.consent.roots),
                codex_enabled: self.consent.codex_enabled,
                codex_roots: selected_paths(&self.consent.codex_roots),
                pricing_version: Catalog::bundled().version.to_string(),
                pricing_source: Catalog::bundled().source.to_string(),
                ..Snapshot::default()
            },
            None,
            repaint,
        );
    }

    fn deliver(
        &mut self,
        shared: &Shared,
        generation: u64,
        snapshot: Snapshot,
        guard: Option<File>,
        repaint: &impl Fn(),
    ) {
        if self.last.as_ref() == Some(&snapshot) {
            return;
        }
        let mut mailbox = shared.mailbox.lock().unwrap();
        if mailbox.generation == generation && !mailbox.finish {
            self.last = Some(snapshot.clone());
            mailbox.reply = Some((snapshot, self.consent.clear_generation));
            self.reply_guard = guard;
            drop(mailbox);
            repaint();
        }
    }
}

fn ledger_status(error: &LedgerError) -> Status {
    match error {
        LedgerError::FutureVersion | LedgerError::Invalid => Status::Incompatible,
        LedgerError::Limit => Status::Limit,
        LedgerError::Deadline | LedgerError::Cancelled => Status::Stale,
        LedgerError::Sql(error)
            if matches!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
            ) =>
        {
            Status::Stale
        }
        LedgerError::Sql(_) | LedgerError::Io(_) | LedgerError::Missing => Status::Error,
    }
}

fn accepts_source(provider: Provider, path: &std::path::Path) -> bool {
    let rollout = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"));
    match provider {
        Provider::Claude => !rollout,
        Provider::Codex => rollout,
    }
}

fn selected_paths(roots: &[Binding]) -> Vec<PathBuf> {
    roots.iter().map(|root| root.path.clone()).collect()
}

#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::DirBuilderExt;
    use std::path::Path;

    const RECORD: &str = r#"{"type":"assistant","timestamp":"2025-12-01T00:00:00Z","requestId":"req_test","message":{"id":"msg_test","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"cache_read_input_tokens":30,"cache_creation_input_tokens":10,"cache_creation":{"ephemeral_5m_input_tokens":10,"ephemeral_1h_input_tokens":0},"output_tokens":20}}}"#;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let mut suffix = [0_u8; 8];
            getrandom::fill(&mut suffix).unwrap();
            let base = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("ivy-usage-{:016x}", u64::from_le_bytes(suffix)));
            #[cfg(unix)]
            fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
            #[cfg(windows)]
            fs::create_dir(&base).unwrap();
            Self(base)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn await_snapshot(service: &Service, good: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut last = None;
        while Instant::now() < deadline {
            if let Some(snapshot) = service.poll() {
                if good(&snapshot) {
                    return snapshot;
                }
                last = Some(snapshot);
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("expected collector state, got {last:?}");
    }

    fn address(base: &Path) -> (PathBuf, PathBuf) {
        let root = base.join("selected");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("conversation.jsonl"),
            format!("{RECORD}\n{RECORD}\n"),
        )
        .unwrap();
        (root, base.join("control"))
    }

    fn range() -> DateRange {
        DateRange {
            start_ms: 1_764_460_800_000,
            end_ms: 1_764_806_400_000,
        }
    }

    #[test]
    fn compressed_rollout_marks_only_codex_coverage_partial() {
        let fixture = Fixture::new();
        let (root, control) = address(&fixture.0);
        std::fs::write(
            root.join("rollout-synthetic.jsonl.zst"),
            b"not a readable rollout",
        )
        .unwrap();
        let service = Service::new(control, || {});
        service.request(range());
        assert!(service.enable(vec![root.clone()]));
        let claude = await_snapshot(&service, |state| state.status == Status::Ready);
        assert!(!claude.totals.partial);
        assert!(service.enable_codex(vec![root]));
        let combined = await_snapshot(&service, |state| {
            state.codex_enabled && state.status == Status::Partial
        });
        assert_eq!(combined.totals.tokens, 160);
        service.request_comparison(range(), Some(Provider::Claude), None, Granularity::Day);
        let only_claude =
            await_snapshot(&service, |state| state.provider == Some(Provider::Claude));
        assert!(!only_claude.totals.partial);
    }

    #[test]
    fn codex_rollouts_require_separate_consent_and_compare_with_retained_claude() {
        let fixture = Fixture::new();
        let (root, control) = address(&fixture.0);
        std::fs::write(
            root.join("rollout-synthetic.jsonl"),
            concat!(
                r#"{"type":"session_meta","payload":{"id":"fixture-session"}}"#, "\n",
                r#"{"type":"turn_context","payload":{"model":"gpt-5.3-codex"}}"#, "\n",
                r#"{"type":"event_msg","timestamp":"2025-12-01T10:00:00Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"reasoning_output_tokens":5},"total_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"reasoning_output_tokens":5}}}}"#, "\n",
            ),
        ).unwrap();
        let service = Service::new(control, || {});
        service.request(range());
        assert!(service.enable(vec![root.clone()]));
        let claude = await_snapshot(&service, |state| state.last_success_at_ms.is_some());
        assert_eq!(claude.totals.tokens, 160);
        assert!(!claude.codex_enabled);

        assert!(service.enable_codex(vec![root]));
        let both = await_snapshot(&service, |state| state.totals.tokens == 280);
        assert!(
            both.models
                .iter()
                .any(|m| m.provider == Provider::Codex && m.label == "gpt-5.3-codex")
        );
        service.request_comparison(
            range(),
            Some(Provider::Codex),
            Some("gpt-5.3-codex".into()),
            Granularity::Hour,
        );
        let codex = await_snapshot(&service, |state| state.provider == Some(Provider::Codex));
        assert_eq!(codex.totals.tokens, 120);
        assert_eq!(codex.buckets.len(), 1);
    }

    #[test]
    fn enables_explicitly_retains_after_stop_and_clears_with_follower() {
        let fixture = Fixture::new();
        let (root, control) = address(&fixture.0);
        let service = Service::new(control.clone(), || {});
        service.request(range());
        await_snapshot(&service, |state| state.status == Status::Disabled);
        assert!(!control.exists());

        assert!(service.enable(vec![root.clone()]));
        let found = await_snapshot(&service, |state| {
            state.totals.tokens == 160 && state.last_success_at_ms.is_some()
        });
        assert_eq!(found.roots, std::slice::from_ref(&root));
        assert!(found.has_history);
        assert!(found.totals.tokens_known);
        assert!(found.totals.uncached_input_known);
        assert!(found.totals.cache_read_known);
        assert!(found.totals.cache_write_known);
        assert!(found.totals.output_known);
        assert_eq!(found.daily.len(), 1);

        let follower = Service::new(control.clone(), || {});
        follower.request(range());
        let following = await_snapshot(&follower, |state| state.totals.tokens == 160);
        assert_eq!(following.last_success_at_ms, found.last_success_at_ms);
        assert_eq!(following.status, Status::Partial);
        assert!(following.totals.partial);
        assert!(following.totals.estimate.unwrap().partial);

        assert!(service.stop());
        let stopped = await_snapshot(&service, |state| state.status == Status::Disabled);
        assert_eq!(stopped.totals.tokens, 160);
        assert!(!stopped.enabled);

        assert!(follower.clear());
        let empty = await_snapshot(&follower, |state| {
            state.status == Status::Disabled && !state.has_history
        });
        assert!(empty.roots.is_empty());
        assert!(!control.join("clear.pending").exists());
        assert!(!control.join("ledger").exists());
        assert!(root.join("conversation.jsonl").exists());
        await_snapshot(&service, |state| {
            state.status == Status::Disabled && !state.has_history
        });
    }

    #[test]
    fn interrupted_clear_is_resumed_before_enable_or_open() {
        let fixture = Fixture::new();
        let (root, control_path) = address(&fixture.0);
        let control = Control::new(control_path.clone());
        control.set_enabled(vec![root], None).unwrap();
        let owner = control.owner().unwrap();
        let reader = control.reader(true).unwrap();
        let db = control.database(true).unwrap();
        drop(Ledger::open_writer(&db).unwrap());
        control.begin_clear().unwrap();

        let service = Service::new(control_path.clone(), || {});
        service.request(range());
        await_snapshot(&service, |state| state.status == Status::WaitingToClear);
        assert!(db.exists());
        drop(reader);
        drop(owner);
        await_snapshot(&service, |state| state.status == Status::Disabled);
        assert!(!db.exists());
        assert!(!control_path.join("clear.pending").exists());
    }

    #[test]
    fn preclear_enable_cannot_reopen_a_cleared_ledger() {
        let fixture = Fixture::new();
        let (root, path) = address(&fixture.0);
        let control = Control::new(path.clone());
        assert!(control.set_enabled(vec![root.clone()], None).unwrap());
        let lease = control.reader(true).unwrap();
        let db = control.database(true).unwrap();
        drop(Ledger::open_writer(&db).unwrap());
        control.begin_clear().unwrap();
        assert!(!control.set_enabled(vec![root.clone()], None).unwrap());
        assert_eq!(
            control.complete_clear().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(lease);
        control.complete_clear().unwrap();

        assert!(!control.set_enabled(vec![root.clone()], None).unwrap());
        let _lease = control.reader(false).unwrap();
        assert_eq!(
            control.database(false).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(!db.parent().unwrap().exists());
        let consent = control.read().unwrap();
        assert!(consent.clear_generation.is_some());
        assert!(!consent.enabled);
        assert!(
            control
                .set_enabled(vec![root], consent.clear_generation)
                .unwrap()
        );
    }

    #[test]
    fn missing_source_retains_published_counts_and_timestamp_after_restart() {
        let fixture = Fixture::new();
        let (root, control) = address(&fixture.0);
        let service = Service::new(control.clone(), || {});
        service.request(range());
        service.enable(vec![root.clone()]);
        let imported = await_snapshot(&service, |state| {
            state.status == Status::Ready && state.last_success_at_ms.is_some()
        });
        drop(service);
        let previous_owner = Control::new(control.clone());
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match previous_owner.owner() {
                Ok(lease) => {
                    drop(lease);
                    break;
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("previous collector did not release its owner lock: {error}"),
            }
        }
        fs::remove_file(root.join("conversation.jsonl")).unwrap();

        let restarted = Service::new(control, || {});
        restarted.request(range());
        let retained = await_snapshot(&restarted, |state| state.status == Status::Partial);
        assert_eq!(retained.totals.tokens, 160);
        assert!(retained.totals.partial);
        assert_eq!(retained.last_success_at_ms, imported.last_success_at_ms);
    }

    #[test]
    fn owner_death_unlocks_collection_and_abandons_the_unpublished_candidate() {
        const CHILD_PATH: &str = "IVY_USAGE_CRASH_TEST_CHILD";
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            let fixture = PathBuf::from(path);
            let control = Control::new(fixture.join("control"));
            let _owner = control.owner().unwrap();
            let _lifecycle = control.reader(true).unwrap();
            let mut db = Ledger::open_writer(&control.database(true).unwrap()).unwrap();
            let stale_source = [18; 32];
            let stale = RECORD.replace("req_test", "req_abandoned");
            let crate::claude::Record::Observation(row) = crate::claude::parse(
                stale.as_bytes(),
                stale_source,
                &db.info().unwrap().identity_salt,
            ) else {
                panic!("crash fixture is not an observation");
            };
            db.begin(stale_source).unwrap();
            db.stage(&[row]).unwrap();
            fs::write(fixture.join("candidate-ready"), b"ready").unwrap();
            thread::park_timeout(Duration::from_secs(30));
            return;
        }

        let fixture = Fixture::new();
        let (root, cache) = address(&fixture.0);
        let control = Control::new(cache.clone());
        control.set_enabled(vec![root], None).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "collector::tests::owner_death_unlocks_collection_and_abandons_the_unpublished_candidate",
            ])
            .env(CHILD_PATH, &fixture.0)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let ready = fixture.0.join("candidate-ready");
        let deadline = Instant::now() + Duration::from_secs(8);
        while !ready.exists() && child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(ready.exists(), "child did not stage a candidate");

        let service = Service::new(cache, || {});
        service.request(range());
        let recovered = await_snapshot(&service, |state| {
            state.status == Status::Ready && state.last_success_at_ms.is_some()
        });
        assert_eq!(recovered.totals.tokens, 160);
    }
}
