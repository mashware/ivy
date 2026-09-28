//! Who finds out that a producer has published, and how it tells the window.
//!
//! **A single watcher for all consoles.** Each one creates a thread, and the notice carries the path
//! that changed, which is all it takes to know who it wakes. Four panes cost one thread.
//!
//! **Each console's directory is watched, not its `panel.json`.** Read at the source of `notify`: the
//! tracking of a path that is not a directory is placed on the inode, is removed when that inode
//! disappears and **is never registered again**. Since the contract recommends publishing by writing to
//! a temporary and renaming it over, watching the file would leave the panel silently frozen the first
//! time someone followed the recommendation. By watching the directory the notice of the first file
//! also arrives, which with the other form cannot even be armed: tracking a path that does not yet
//! exist fails.
//!
//! **And the notice only means «look again».** The crate neither groups nor deduplicates —one write
//! produces several notices, and how many is not determinable— so what is consumed has to be
//! idempotent. A flag is, and with that the debounce comes for free: two notices and twenty give
//! exactly the same work, which is one read in the next frame.
//!
//! **This module knows nothing of windows.** It receives how to wake, just like the bridge, and for the
//! same reason: what is going to be rewritten is the interface framework, and waking someone needs
//! none.
//!
//! Two known limits, written here because whoever runs into them is not going to read the design:
//! - **The paths are fixed when it is armed.** With this milestone's two panes it is enough, but the
//!   day panes can be opened and closed (Phase 2) it has to be rearmed, or the new one would never find
//!   out. Rearming costs about 300 µs, and that is not what matters: **between releasing the old one and
//!   arming the new one nothing arrives**, so a publication that falls right there is lost forever.
//! - **The path comparison is textual**, and that works because the Linux backend keeps the path just
//!   as it is given. The macOS one delivers resolved paths (`/var/…` → `/private/var/…`), so there it
//!   would match nothing, silently. It is a matter for the milestone that compiles for the first time on
//!   macOS.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
use std::sync::atomic::AtomicUsize;

use notify::event::{AccessKind, AccessMode, ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
// **The *trait* comes in unnamed**, and it is needed: `backend.watch(..)` is its, and its name clashes
// with this module's type now that it stopped being named in Spanish.
use notify::Watcher as _;

/// «This console's directory has changed: look again.»
///
/// It is shared by the watcher thread, which raises it, with the frame, which collects it. It carries
/// neither the event nor how many there were, on purpose: the only thing the window needs to know is
/// whether it has to reread.
///
/// **The memory order is relaxed and it is not an oversight**: no data is published through this flag.
/// What has to be read travels through the file system and is written by another process, so there is
/// nothing to acquire or release — asking for a stronger order would assert a synchronization that does
/// not exist.
#[derive(Clone)]
pub struct Flag(Arc<AtomicBool>);

impl Flag {
    /// A flag already raised, which is how they are born.
    ///
    /// **And it is not `Default`**: a `Default` that is `true` reads backwards at the place where it is
    /// used. Being born raised covers a real gap —the watcher is armed when the window already has its
    /// consoles open, so a lightning-fast producer may have published before anyone looked at its
    /// directory— and it costs one read in the first frame.
    pub fn raised() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    /// Raises it. That it already was is not a rare case: it is the normal one.
    pub fn raise(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Collects it: says whether it was raised and leaves it lowered.
    ///
    /// **Lowering it before reading and not after** is what keeps a notice that arrived *during* the
    /// read from being lost: it raises the flag again and the next frame comes back. The other way
    /// around, the half-written file would be read and the flag warning about the other half would be
    /// lowered.
    pub fn take(&self) -> bool {
        self.0.swap(false, Ordering::Relaxed)
    }
}

/// A watched console: where it publishes and who has to be notified.
struct Watched {
    /// The directory, which is what is really tracked. It is kept to recognize the notice that it has
    /// stopped existing, which arrives with **its** path and not with the file's.
    dir: PathBuf,
    /// The exact path of the `panel.json`. It is kept ready-made because it is compared on every notice,
    /// and the notices arrive several at a time per write.
    file: PathBuf,
    flag: Flag,
}

/// The window's watcher.
pub struct Watcher {
    /// **It is kept for its lifetime, not for its interface**: dropping it asks its thread to end.
    /// That ending is asynchronous, so [`Self::active`] rejects anything that still arrives. `None`
    /// means there is nothing to watch, arming failed, or this generation is being replaced; which
    /// failures require fallback is told by [`Watcher::is_blind`].
    _backend: Option<RecommendedWatcher>,
    /// Whether the watcher has stopped being trustworthy. It shares it with its thread, because **almost
    /// all the ways of going blind happen after arming it** and only the thread sees them.
    blind: Arc<AtomicBool>,
    /// Whether notices still belong to a live watcher generation.
    ///
    /// `notify` ends its backend thread asynchronously. This gate is lowered before asking it to
    /// stop, so a queued event cannot turn an intentional pane retirement into lost tracking.
    active: Arc<AtomicBool>,
    /// How many times this generation changed from active to stopped before replacement or
    /// intentional directory retirement.
    #[cfg(test)]
    retired: Arc<AtomicUsize>,
    /// The watched directories and whether any was already gone when this generation stopped.
    /// Test-only because it is an ordering oracle, not application state.
    #[cfg(test)]
    watched_dirs: Vec<PathBuf>,
    #[cfg(test)]
    retired_after_directory_loss: Arc<AtomicBool>,
}

impl Watcher {
    /// Arms the watcher over each console's directory.
    ///
    /// `wake` is how the window is brought out of rest, and it arrives as a function for the same reason
    /// as with the bridge: **this module does not depend on the interface framework**, which is exactly
    /// what is going to be rewritten. It is called from the watcher thread, so it cannot panic.
    ///
    /// **Not being able to arm it does not prevent opening the window**, just as not being able to
    /// create a console's directory does not prevent opening it: it is said through the error output and
    /// it carries on in the degraded mode that [`Watcher::is_blind`] describes.
    pub fn new(wake: impl Fn() + Send + 'static, consoles: &[(&Path, Flag)]) -> Self {
        let blind = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(true));
        #[cfg(test)]
        let watched_dirs = consoles.iter().map(|(dir, _)| dir.to_path_buf()).collect();
        #[cfg(test)]
        let retired_after_directory_loss = Arc::new(AtomicBool::new(false));

        // With no consoles with a directory there is nothing to track, and setting up an instance of the
        // system watcher and its thread to look at nothing would be paying for doing nothing. **And it is
        // not being blind**: there is no panel that can go stale.
        if consoles.is_empty() {
            return Self {
                _backend: None,
                blind,
                active,
                #[cfg(test)]
                retired: Arc::new(AtomicUsize::new(0)),
                #[cfg(test)]
                watched_dirs,
                #[cfg(test)]
                retired_after_directory_loss,
            };
        }

        let watchlist = consoles
            .iter()
            .map(|(dir, flag)| Watched {
                dir: dir.to_path_buf(),
                file: dir.join(panel_contract::PANEL_FILE),
                flag: flag.clone(),
            })
            .collect();

        let handler = handler(wake, watchlist, Arc::clone(&blind), Arc::clone(&active));
        let mut backend = match notify::recommended_watcher(handler) {
            Ok(backend) => backend,
            Err(failure) => {
                eprintln!("could not arm the panel watcher ({failure})");
                blind.store(true, Ordering::Relaxed);
                return Self {
                    _backend: None,
                    blind,
                    active,
                    #[cfg(test)]
                    retired: Arc::new(AtomicUsize::new(0)),
                    #[cfg(test)]
                    watched_dirs,
                    #[cfg(test)]
                    retired_after_directory_loss,
                };
            }
        };

        for (dir, _) in consoles {
            if let Err(failure) = backend.watch(dir, RecursiveMode::NonRecursive) {
                // The **whole** watcher is given up on and not just this console. A half watcher would be
                // a window in which one panel finds out and the one next to it does not, with nothing to
                // tell them apart on looking at it; the reread fallback is correct for all of them.
                eprintln!("could not watch {} ({failure})", dir.display());
                blind.store(true, Ordering::Relaxed);
                return Self {
                    _backend: None,
                    blind,
                    active,
                    #[cfg(test)]
                    retired: Arc::new(AtomicUsize::new(0)),
                    #[cfg(test)]
                    watched_dirs,
                    #[cfg(test)]
                    retired_after_directory_loss,
                };
            }
        }

        Self {
            _backend: Some(backend),
            blind,
            active,
            #[cfg(test)]
            retired: Arc::new(AtomicUsize::new(0)),
            #[cfg(test)]
            watched_dirs,
            #[cfg(test)]
            retired_after_directory_loss,
        }
    }

    /// Stops this watcher before the window removes a directory intentionally.
    ///
    /// The handler is made inert before the backend is dropped because `notify` stops its thread
    /// asynchronously. Rearming raises every survivor's reread flag, so a publication in the short
    /// interval before the replacement is installed cannot leave a panel stale.
    pub fn stop_before_retirement(&mut self) {
        let _was_active = self.active.swap(false, Ordering::SeqCst);
        self._backend = None;
        #[cfg(test)]
        if _was_active {
            if self.watched_dirs.iter().any(|dir| !dir.exists()) {
                self.retired_after_directory_loss
                    .store(true, Ordering::Relaxed);
            }
            self.retired.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Whether there is no longer anyone to notify, and therefore the files have to be looked at as was
    /// done before this watcher existed.
    ///
    /// **Almost all the ways of going blind happen with the window already open**, not on arming it:
    /// that the user deletes from their own console the directory that was being watched —the tracking
    /// is lost with the inode and is not re-registered—, or that the watcher delivers a failure, after
    /// which its state is unknown. On arming, only what the system does not give fails: the kernel limits
    /// how many watchers a user can have at once (`fs.inotify.max_user_instances`), and there is nothing
    /// to be done about that.
    ///
    /// Without this fallback the panel would stay **frozen forever and silently**, showing something
    /// plausible from an hour ago, which is exactly what this milestone comes to avoid.
    pub fn is_blind(&self) -> bool {
        self.blind.load(Ordering::Relaxed)
    }

    /// Keeps the retired watcher's verdict observable after the window replaces it.
    #[cfg(test)]
    pub(crate) fn blindness_witness(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.blind)
    }

    /// Keeps intentional shutdown observable after the window replaces this watcher.
    #[cfg(test)]
    pub(crate) fn retirement_witness(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.retired)
    }

    /// Says whether this generation was stopped only after one of its directories disappeared.
    #[cfg(test)]
    pub(crate) fn late_retirement_witness(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.retired_after_directory_loss)
    }
}

/// What runs on the watcher thread each time a notice arrives.
///
/// It is separate and returns the function **so that it can be invoked from a test with a notice put in
/// by hand**: that the notice really arrives from the file system can be tested, but it is slow and
/// depends on the kernel. What matters here —who each notice wakes— does not need the kernel.
fn handler(
    wake: impl Fn() + Send + 'static,
    watchlist: Vec<Watched>,
    blind: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
) -> impl FnMut(notify::Result<notify::Event>) + Send + 'static {
    move |notice| {
        if !active.load(Ordering::SeqCst) {
            return;
        }
        let event = match notice {
            Ok(event) => event,
            // After a failure **the watcher's state is unknown**, so it switches to the fallback instead
            // of going on trusting notices that may no longer arrive.
            Err(failure) => {
                return go_blind(&blind, &wake, &format!("the watcher failed ({failure})"));
            }
        };

        // The only notice that means «you have missed things», and it comes **with no paths**: there is
        // no one to hand it to, so everything is looked at. It leaves no one blind —the overflowed queue
        // is of an instant, not a breakdown— but the notice that was lost behind it does not come back.
        if event.need_rescan() {
            for watched in &watchlist {
                watched.flag.raise();
            }
            wake();
            return;
        }

        // The watched directory has stopped existing, and with it the tracking: `notify` deletes it from
        // its maps and **never registers it again**. Here the `panel.json`'s path does not arrive, but
        // the directory's, so without this the notice would be discarded and no one would find out that
        // from now on nothing arrives.
        if lost_a_watched_dir(&event, &watchlist) {
            return go_blind(
                &blind,
                &wake,
                "the directory that was being watched has stopped existing",
            );
        }

        if is_nothing_new(&event.kind) {
            return;
        }

        if raise_matching(&event.paths, &watchlist) {
            // **Requesting the frame is the central piece of this milestone, not a precaution.** The
            // window repaints only when asked to, and the only request there was, was tied to the
            // pseudoterminal producing: with the window at rest the flag would be raised and the next
            // frame would never arrive.
            wake();
        }
    }
}

/// Switches to the degraded mode, and says it **just once**.
///
/// Once is the right thing and it is not stinginess: the watcher's read loop delivers the failure and
/// carries on, so a failure that persists would be an unbounded stream of lines through the error
/// output. And it is written without being able to panic, because this runs on the watcher thread: with
/// the error output through a pipe with no reader, an `eprintln!` would kill the thread and nothing more
/// would arrive.
fn go_blind(blind: &AtomicBool, wake: &impl Fn(), reason: &str) {
    if !blind.swap(true, Ordering::Relaxed) {
        let _ = writeln!(
            std::io::stderr(),
            "the panels will be reread on every frame: {reason}"
        );
        wake();
    }
}

/// Whether the notice says that a watched directory is no longer where it was.
///
/// The two ways of losing it —deleting it and moving it— arrive with the directory's own path.
fn lost_a_watched_dir(event: &notify::Event, watchlist: &[Watched]) -> bool {
    let lost = matches!(
        event.kind,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From))
    );

    lost && watchlist
        .iter()
        .any(|watched| event.paths.contains(&watched.dir))
}

/// The two notices that do **not** count as a new publication.
///
/// Discarding them makes no difference in what is seen; it makes a difference in how many frames
/// publishing costs, which is what this milestone pays to find out.
fn is_nothing_new(kind: &EventKind) -> bool {
    match kind {
        // **Closing after writing is publishing**: it cannot be ours, because here it is only read, and
        // it is the only signal that arrives when the producer has already finished writing.
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => false,
        // **Opening the file to read it also produces a notice**, and whoever opens it is this very
        // application: without filtering it, every read requests a frame that reads again. It is not
        // theoretical — with a `panel.json` that is not understood, and which is therefore reopened on
        // every attempt, the window would never fall asleep: **measured, 123 reads in two seconds versus
        // 2**.
        EventKind::Access(_) => true,
        // **The echo of a rename.** A rename over —what the contract recommends— delivers the
        // destination and **right after** a second notice with origin and destination together, which
        // carries the same path and says nothing the first did not. Read at the source: that second one
        // never arrives without the first ahead of it. Discarding it brings the **frames per publication
        // down from 1.88 to 1** on the path the contract recommends.
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => true,
        _ => false,
    }
}

/// Raises the flag of each console whose `panel.json` appears in the notice, and says whether any was
/// raised.
///
/// A notice carries several paths and a single write causes several notices. What is **not** looked at
/// is what kind of event it is: creating, writing and renaming over all end in the same read.
fn raise_matching(paths: &[PathBuf], watchlist: &[Watched]) -> bool {
    let mut any_raised = false;

    for watched in watchlist {
        if paths.contains(&watched.file) {
            watched.flag.raise();
            any_raised = true;
        }
    }

    any_raised
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::Event;
    use notify::event::{CreateKind, DataChange, RemoveKind};
    use std::sync::atomic::AtomicUsize;

    /// How many times the window has been woken.
    ///
    /// **Counting is stronger than asking the framework whether it has a repaint pending**, which is
    /// what the first version of these tests did: that forced leaving the context at rest first —four
    /// empty passes— and asserted over egui's internal bookkeeping instead of over what this module
    /// does.
    #[derive(Clone, Default)]
    struct Wakeups(Arc<AtomicUsize>);

    impl Wakeups {
        fn count(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }

        fn wake_fn(&self) -> impl Fn() + Send + 'static {
            let count = Arc::clone(&self.0);
            move || {
                count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Two consoles with their directory, and the real handler mounted over them.
    ///
    /// The handler is returned and not the loose pieces **on purpose**: what has to be pinned down is
    /// what runs on the watcher thread. A test over the dispatcher alone would prove that the dispatcher
    /// works, not that anyone calls it nor that anyone is woken.
    struct Harness {
        handler: Box<dyn FnMut(notify::Result<Event>)>,
        consoles: [Console; 2],
        wakeups: Wakeups,
        blind: Arc<AtomicBool>,
        active: Arc<AtomicBool>,
    }

    struct Console {
        dir: PathBuf,
        flag: Flag,
    }

    impl Console {
        fn panel(&self) -> PathBuf {
            self.dir.join(panel_contract::PANEL_FILE)
        }
    }

    fn two_consoles() -> Harness {
        let consoles = [
            Console {
                dir: PathBuf::from("/does-not-matter/one"),
                flag: Flag::raised(),
            },
            Console {
                dir: PathBuf::from("/does-not-matter/other"),
                flag: Flag::raised(),
            },
        ];

        let wakeups = Wakeups::default();
        let blind = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(true));
        let handler = handler(
            wakeups.wake_fn(),
            consoles
                .iter()
                .map(|console| Watched {
                    dir: console.dir.clone(),
                    file: console.panel(),
                    flag: console.flag.clone(),
                })
                .collect(),
            Arc::clone(&blind),
            Arc::clone(&active),
        );

        // The flags are born raised so that the first frame looks; what these tests measure is what
        // happens **from there on**, so they are collected before starting.
        for console in &consoles {
            console.flag.take();
        }

        Harness {
            handler: Box::new(handler),
            consoles,
            wakeups,
            blind,
            active,
        }
    }

    /// The notice that something has been written to a path.
    fn written_to(path: PathBuf) -> notify::Result<Event> {
        Ok(Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any))).add_path(path))
    }

    #[test]
    fn a_notice_raises_the_flag_of_its_console_and_not_that_of_the_other() {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].panel()));

        assert!(
            harness.consoles[0].flag.take(),
            "the console whose panel changed has not found out"
        );
        assert!(
            !harness.consoles[1].flag.take(),
            "the console next to it will reread its file without anyone having touched it"
        );
    }

    #[test]
    fn a_notice_wakes_the_window() {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].panel()));

        assert_eq!(
            harness.wakeups.count(),
            1,
            "the flag was raised and no one woke the window: the panel would not move"
        );
    }

    /// **The contract's own temporary file is the case to filter, not an oddity.** Whoever publishes as
    /// the contract recommends writes `panel.json.tmp` in that same directory and renames it over:
    /// without filtering, every publication would cause an extra read of the file that is not yet the
    /// good one.
    fn a_neighbor_raises_nothing(neighbor: &str) {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].dir.join(neighbor)));

        assert!(
            !harness.consoles[0].flag.take(),
            "«{neighbor}» made its console's panel reread"
        );
        assert!(
            !harness.consoles[1].flag.take(),
            "«{neighbor}» woke the one next to it"
        );
        assert_eq!(
            harness.wakeups.count(),
            0,
            "«{neighbor}» brought the window out of rest to do nothing"
        );
    }

    #[test]
    fn the_atomic_write_temporary_raises_nothing() {
        a_neighbor_raises_nothing("panel.json.tmp");
    }

    #[test]
    fn any_file_of_the_directory_raises_nothing() {
        a_neighbor_raises_nothing("notes.txt");
    }

    /// **A notice carries several paths**, and the crate does not promise which: if only the first were
    /// looked at, a publication would be lost depending on which order it arrives in.
    #[test]
    fn a_notice_with_several_paths_looks_at_all_of_them() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Create(CreateKind::File))
            .add_path(harness.consoles[0].dir.join("panel.json.tmp"))
            .add_path(harness.consoles[0].panel())));

        assert!(
            harness.consoles[0].flag.take(),
            "the panel.json came in the notice and no one saw it"
        );
    }

    /// **Publishing well cannot cost two frames.** A rename over —what the contract recommends—
    /// delivers the destination and **right after** a second notice with origin and destination
    /// together: the same path again, and nothing new to say. Without discarding it, every
    /// well-made publication asks for **1.88 frames** instead of one, measured in the real application.
    #[test]
    fn the_echo_of_a_rename_does_not_ask_for_a_second_frame() {
        let mut harness = two_consoles();
        let (temp, panel) = (
            harness.consoles[0].dir.join("panel.json.tmp"),
            harness.consoles[0].panel(),
        );

        // The two notices of the rename, in the order in which they arrive.
        (harness.handler)(Ok(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::To,
        )))
        .add_path(panel.clone())));
        (harness.handler)(Ok(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::Both,
        )))
        .add_path(temp)
        .add_path(panel)));

        assert!(
            harness.consoles[0].flag.take(),
            "the rename over is a publication and it has not been seen"
        );
        assert_eq!(
            harness.wakeups.count(),
            1,
            "a single publication asked for two frames"
        );
    }

    /// **The application cannot hear itself read.** The system watcher also notifies that someone has
    /// opened the file, and whoever opens it is this very application: without filtering it, reading
    /// raises the flag, the flag asks for a frame and the frame reads again. With a `panel.json` that is
    /// not understood —which is why it is reopened every time— that is a window that never falls asleep,
    /// with its disk read per frame and nothing on screen to explain it.
    #[test]
    fn opening_the_file_to_read_it_raises_no_flag() {
        let mut harness = two_consoles();

        for ours in [
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            EventKind::Access(AccessKind::Read),
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
        ] {
            (harness.handler)(Ok(Event::new(ours).add_path(harness.consoles[0].panel())));

            assert!(
                !harness.consoles[0].flag.take(),
                "{ours:?}: reading the file is counted as a publication and the read feeds itself back"
            );
        }

        assert_eq!(harness.wakeups.count(), 0);
    }

    /// And the other half, which is the one that keeps the above from being fixed too far: **closing
    /// after writing is publishing**. It is the only signal that arrives when the producer that does not
    /// rename has already finished, and without it the panel would be left with what was read
    /// half-written.
    #[test]
    fn closing_after_writing_does_raise_the_flag() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Access(AccessKind::Close(
            AccessMode::Write,
        )))
        .add_path(harness.consoles[0].panel())));

        assert!(
            harness.consoles[0].flag.take(),
            "the producer finished writing and the panel does not find out"
        );
    }

    /// **The only notice that means «you have missed things» came with no paths and was thrown away.**
    /// It is what the system sends when it overflows its notice queue: behind it goes a real change that
    /// will now never arrive, so everything has to be looked at again.
    #[test]
    fn the_notice_that_notices_have_been_lost_makes_all_of_them_look() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(
            Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan)
        ));

        assert!(
            harness.consoles[0].flag.take() && harness.consoles[1].flag.take(),
            "notices were lost and no console looks at its file again"
        );
        assert_eq!(
            harness.wakeups.count(),
            1,
            "and they have to be gone and looked at"
        );
        assert!(
            !harness.blind.load(Ordering::Relaxed),
            "an overflowed queue is of an instant, not a breakdown: there is no need to degrade over it"
        );
    }

    /// **That the user deletes the directory from their own console is a project scenario**, not a
    /// hypothesis: it has a shell inside. The tracking is lost with the inode and is never re-registered,
    /// and the notice arrives with the directory's path, not with the panel's — so without looking at it
    /// it would be discarded and no one would know that from there on nothing arrives.
    #[test]
    fn losing_the_watched_directory_leaves_the_watcher_blind() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Remove(RemoveKind::Folder))
            .add_path(harness.consoles[0].dir.clone())));

        assert!(
            harness.blind.load(Ordering::Relaxed),
            "the tracking was lost and the window will go on believing that someone notifies it"
        );
        assert_eq!(
            harness.wakeups.count(),
            1,
            "a wake is needed for the fallback to start, or it does not start until something else happens"
        );
    }

    /// `notify` may deliver an event after its backend has been dropped because backend shutdown is
    /// asynchronous. Once this watcher generation is retired, that queued event belongs to nobody
    /// and must not turn an intentional close into lost tracking.
    #[test]
    fn a_directory_loss_queued_after_retirement_is_ignored() {
        let mut harness = two_consoles();
        harness.active.store(false, Ordering::SeqCst);

        (harness.handler)(Ok(Event::new(EventKind::Remove(RemoveKind::Folder))
            .add_path(harness.consoles[0].dir.clone())));

        assert!(
            !harness.blind.load(Ordering::Relaxed),
            "a queued notice from a retired watcher declared lost tracking"
        );
        assert_eq!(
            harness.wakeups.count(),
            0,
            "a queued notice from a retired watcher woke the window"
        );
    }

    /// Deleting the `panel.json` leaves no one blind: the directory is still watched and the producer
    /// can publish again. It is the half that distinguishes the finding above from throwing in the
    /// towel.
    #[test]
    fn deleting_the_panel_leaves_no_one_blind_but_does_make_it_look() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(
            Event::new(EventKind::Remove(RemoveKind::File)).add_path(harness.consoles[0].panel())
        ));

        assert!(
            !harness.blind.load(Ordering::Relaxed),
            "the directory is still there and the watcher gave itself up for dead"
        );
        assert!(
            harness.consoles[0].flag.take(),
            "the panel disappeared and its console does not look again"
        );
    }

    /// A watcher failure **cannot bring down its thread**, and it leaves its state unknown: from there
    /// on one cannot go on trusting notices that may no longer arrive.
    #[test]
    fn a_failure_leaves_the_watcher_blind_without_panicking() {
        let mut harness = two_consoles();

        (harness.handler)(Err(notify::Error::generic("the notices ran out")));

        assert!(
            harness.blind.load(Ordering::Relaxed),
            "the watcher failed and the window will go on waiting for notices from it"
        );
    }

    /// And the failure that repeats **cannot turn into a stream through the error output**: the
    /// watcher's read loop delivers the failure and carries on, so a persistent one would arrive on
    /// every round.
    #[test]
    fn a_failure_that_repeats_wakes_only_once() {
        let mut harness = two_consoles();

        for _ in 0..10 {
            (harness.handler)(Err(notify::Error::generic("that's ten now")));
        }

        assert_eq!(
            harness.wakeups.count(),
            1,
            "a failure that repeats wakes the window on every round"
        );
    }

    /// **Without a watcher, the window has to know it.** It is what triggers the fallback of rereading on
    /// every frame, and without that the milestone would worsen what there was instead of improving it.
    #[test]
    fn an_armed_watcher_is_not_blind() {
        let dir = panel_contract::PanelDir::create().expect("create the directory");

        let watcher = Watcher::new(|| {}, &[(dir.path(), Flag::raised())]);

        assert!(
            !watcher.is_blind(),
            "the watcher armed over a directory that exists and declares itself blind"
        );
    }

    /// A pane close removes its directory on purpose. Stopping that watcher generation must make
    /// even an event already queued on its backend inert.
    #[test]
    fn retiring_a_directory_before_removing_it_does_not_report_lost_tracking() {
        let dir = panel_contract::PanelDir::create().expect("create the directory");
        let path = dir.path().to_path_buf();
        let mut watcher = Watcher::new(|| {}, &[(path.as_path(), Flag::raised())]);
        let active = Arc::clone(&watcher.active);

        watcher.stop_before_retirement();
        assert!(
            !active.load(Ordering::SeqCst),
            "the retiring watcher still admits queued notices"
        );
        drop(dir);

        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            !watcher.is_blind(),
            "an intentionally retired directory was reported as lost tracking"
        );
        assert_eq!(
            watcher.retired.load(Ordering::Relaxed),
            1,
            "the watcher did not record the retirement"
        );
    }

    /// And the other way around: a directory that does not exist **cannot** be watched, and that is no
    /// excuse to stop saying that it is blind.
    ///
    /// The path that does not exist hangs off a freshly created directory **on purpose**: a fixed path
    /// inside the system's temporary would be a bet that no one has ever created it on this machine, and
    /// the day someone did this test would fail forever pointing at the code.
    #[test]
    fn a_directory_that_does_not_exist_leaves_the_watcher_blind() {
        let dir = panel_contract::PanelDir::create().expect("create the directory");
        let missing = dir.path().join("does-not-exist");

        let watcher = Watcher::new(|| {}, &[(missing.as_path(), Flag::raised())]);

        assert!(
            watcher.is_blind(),
            "nothing could be watched and the window will believe that someone notifies it"
        );
    }

    /// **With no consoles with a directory there is nothing to watch, and that is not being blind.**
    /// Arming the system watcher and its thread to look at nothing would be paying for doing nothing, and
    /// declaring oneself blind would make the window reread on every frame some files that do not exist.
    #[test]
    fn with_no_consoles_nothing_is_armed_and_it_is_not_blind() {
        let watcher = Watcher::new(|| {}, &[]);

        assert!(watcher._backend.is_none(), "it armed to look at nothing");
        assert!(!watcher.is_blind(), "there is no panel that goes stale");
    }
}
