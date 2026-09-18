//! A pane's end of the control channel: what it holds, and where what arrives is applied.
//!
//! **The one decision in this module is *where*, and it is the whole of it.** A document that comes
//! over the channel is applied exactly where the file's refresh already happens — inside
//! `Pane::refresh_its_panel`, which the painting calls before it tiles, and which the pass that
//! measures the panes out of sight calls too.
//!
//! **Draining earlier would have been the careful-looking mistake.** A single apply point in the
//! frame's logic, before the interface pass, under a declared span, is what every instinct suggests;
//! it also *creates* the one window this application does not have today. The panel is re-read
//! inside the painting and the mouse is attended after it, in the same pass, so a press cannot land
//! on a panel newer than the one that was painted — and the button routing leans on exactly that,
//! measuring a press against the previous frame's rectangles on purpose. A drain in the logic would
//! put a new document between the paint and the press, and then a mechanism would have to be built
//! to survive a window that was opened for nothing.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::SystemTime;

use eframe::egui;
use panel_contract::PanelDir;
use panel_control::{Answer, Channel, Delivery, Side as WireSide, What};

use crate::measure::Control;
use crate::painter::CellMetrics;
use crate::panel::{PanelState, Released};
use crate::tab_bar::set_the_tab_bar_aside;
use crate::tiling::{Side, can_split};
use crate::{Pane, Window, the_users_shell};

/// How many deliveries a pane's inbox may hold before a publication is refused.
///
/// **A ceiling on the queue, and it is what keeps memory bounded when nobody drains it.** The window
/// applies deliveries while it runs a pass; a window that is minimised or occluded runs no pass at
/// all, so a producer parked against it used to pile up a document per connection every time its
/// reply timed out — each up to 256 KiB — with nothing reading. With the queue bounded the extra
/// publication comes back refused, the producer is told instead of the window growing, and the queue
/// drains as soon as a pass runs. Eight is one per connection the pane serves.
const MAX_PENDING: usize = 8;

/// What a pane keeps of its channel.
pub struct PaneControl {
    /// Kept for its destructor: dropping it is what asks this pane's threads to end.
    _channel: Channel,
    /// What its connections have handed over and nobody has applied yet.
    ///
    /// **Per pane and not per window, because the pane *is* the address.** A connection reaches one
    /// endpoint and one endpoint belongs to one pane, so routing by an identifier carried in the
    /// message would be deriving a second time something the transport already decided — and it
    /// would put on the wire the very field the design refuses to have on it.
    inbox: Receiver<Delivery>,
    /// Where the window is told that there is something to pick up.
    ///
    /// **The same closure the channel hands its threads, kept here for a second reason**: what a
    /// request needs the tree for is moved aside during a painting — the frame's drain has already
    /// run by then — and without a wake nobody would ask for the pass that answers it, so an agent
    /// parked against a window at rest would wait out its five seconds and be told the window did
    /// not answer. It is the truth of the moment, not the request's outcome.
    wake: Arc<dyn Fn() + Send + Sync>,
    /// Requests that are not the panel's to apply, because they need the tree.
    ///
    /// **Filled where the file's refresh happens and emptied by the window's own pass**, which is the
    /// only place a pane can be opened, read or closed.
    waiting_on_the_window: Vec<Delivery>,
    /// What this pane's channel has done over the whole run.
    ///
    /// **Counted here and read once, rather than summed every frame.** A per-frame sum would have to
    /// be collected from both the painted and the unpainted path, and a path that forgot to
    /// contribute would publish a number that is smaller than the truth — which **D13** rates worse
    /// than no number.
    did: Control,
}

impl PaneControl {
    /// Binds this pane's endpoint and starts listening, or says why it could not.
    ///
    /// **A pane with no channel is a complete pane.** It publishes by file, exactly as every pane
    /// did before this milestone, which is the same decision a pane whose directory could not be
    /// created already gets.
    pub fn open(dir: &PanelDir, wake: impl Fn() + Send + Sync + 'static) -> Option<Self> {
        let (sender, inbox) = sync_channel(MAX_PENDING);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        match Channel::open(dir.path(), dir.id(), sender, Arc::clone(&wake)) {
            Ok(channel) => Some(Self {
                _channel: channel,
                inbox,
                wake,
                waiting_on_the_window: Vec::new(),
                did: Control::default(),
            }),
            // **Binding nothing on a platform that cannot assert who may connect is policy, not a
            // fault, so it is not reported.** Said once per pane at every start it reads as a broken
            // terminal, and the person reading it has nothing to do about it: the platform gets its
            // channel when F2.7 can put a permission on the endpoint.
            Err(failure) if failure.kind() == std::io::ErrorKind::Unsupported => None,
            Err(failure) => {
                eprintln!(
                    "console {}: it goes without a control channel, its endpoint could not be bound ({failure})",
                    dir.id()
                );
                None
            }
        }
    }

    /// Applies everything waiting, and answers whoever is waiting for an answer.
    ///
    /// **The queue is bounded at [`MAX_PENDING`], and the earlier shape that left it unbounded was
    /// wrong for a reason review found.** The argument for leaving it open was that a pane serves a
    /// fixed, small number of connections and each one blocks on its own reply, so what is ordinarily
    /// waiting is one request per connection. What that missed is a caller that *gave up*: its thread
    /// goes back to reading with its delivery still queued, and a window that is minimised runs no
    /// pass to drain it — so a producer parked against it piled up one document per connection per
    /// timeout, unbounded. With `sync_channel` the send applies backpressure (or the listener refuses,
    /// see its `publish`), and memory stays capped.
    ///
    /// **A caller that gave up still gets its document applied when a pass runs**, and the wording on
    /// the other side is what makes that honest: the reply a timed-out caller reads says the window
    /// did not answer in time, never that nothing was published.
    pub fn apply(&mut self, state: &mut PanelState, now: SystemTime) {
        while let Ok(delivery) = self.inbox.try_recv() {
            match delivery.what {
                What::Publish { source, panel } => {
                    let answer = match state.deliver(delivery.connection, &source, panel, now) {
                        Ok(()) => {
                            self.did.applied += 1;
                            Answer::Taken
                        }
                        Err(why) => {
                            self.did.refused += 1;
                            Answer::Refused(why)
                        }
                    };
                    // The send cannot block — the answer channel takes one value — and it fails only
                    // when the caller is already gone, which changes nothing about what is on screen.
                    let _ = delivery.answer.send(answer);
                }
                What::Closed => {
                    if state.release(delivery.connection) == Released::Given {
                        self.did.released += 1;
                    }
                    // **And the window is told that this connection is over**, because the handles it
                    // was given are the window's to forget: left behind, they would name consoles the
                    // connection no longer has for as long as the window lives.
                    self.waiting_on_the_window.push(delivery);
                    (self.wake)();
                }
                // **The three `split_*` verbs are the window's, and a pane has no tree.** They are
                // held here until the window's drain takes them, which is where the handles, the
                // focus and the revocation live — and the window is woken for it, because the drain
                // of this very pass has already run: without that, a request arriving while the
                // window is at rest would be answered by no pass at all.
                What::Open { .. } | What::Read { .. } | What::Close { .. } => {
                    self.waiting_on_the_window.push(delivery);
                    (self.wake)();
                }
            }
        }
    }

    /// Takes what only the window can answer, leaving the panel's own deliveries applied.
    ///
    /// **A second queue and not a second thread**: the pane's inbox is drained where the file's
    /// refresh already happens, and the window walks its panes once per pass afterwards. What the
    /// window cannot do from inside a painting is act on the tree, so these wait for it there.
    pub fn take_waiting_on_the_window(&mut self) -> Vec<Delivery> {
        std::mem::take(&mut self.waiting_on_the_window)
    }

    /// Puts a delivery in the window's queue, **for a test about the window rather than the socket**.
    ///
    /// The channel's own path is covered where the channel lives; what a test needs here is a
    /// request already in the window's hands, without paying for a connection and its deadline.
    #[cfg(test)]
    pub(crate) fn push_for_the_tests(&mut self, delivery: Delivery) {
        self.waiting_on_the_window.push(delivery);
    }
}

impl PaneControl {
    /// What this pane's channel has done, for the report.
    pub fn did(&self) -> Control {
        self.did
    }
}

/// What a connection opened, as the window remembers it.
#[derive(Debug)]
struct Opened {
    /// The pane whose channel the request came in on.
    origin: usize,
    /// The connection that opened it, which is what the handle is scoped by.
    connection: u64,
    /// The pane it opened, by name.
    pane: usize,
}

/// Every console an agent has opened this run, by the opaque handle it was given.
///
/// **The handle is sixteen random bytes and not an index**, and that is the security property rather
/// than a style: `PANEL_ID` is in the environment of every descendant and any sibling of this user
/// reads it out of `/proc/<pid>/environ`, so a name is not a credential — and a small integer is
/// reachable by a typo. A handle also remembers **who** asked, so a second connection cannot name a
/// console it did not open even if it overheard the handle (**AC14**).
#[derive(Debug, Default)]
pub struct OpenedByAgents {
    handles: HashMap<String, Opened>,
}

impl OpenedByAgents {
    /// Remembers a handle that was just issued for a pane.
    ///
    /// **The handle is made by the caller**, because the caller is the one that must fail before
    /// anything is opened when the system gives no randomness: issuing it here, after the console
    /// exists, would leave a live console behind a refusal that says nothing was opened.
    fn record(&mut self, handle: String, origin: usize, connection: u64, pane: usize) {
        self.handles.insert(
            handle,
            Opened {
                origin,
                connection,
                pane,
            },
        );
    }

    /// The pane a handle names, **if this connection is the one that opened it**.
    ///
    /// **Whether the person has since taken the console over is asked of the pane and not kept
    /// here.** The write sites are what decide it and a pane is what they hold, so a second copy in
    /// this map would be a second source for one fact — and the copy that got forgotten would be
    /// the one this map answers with.
    fn reachable(&mut self, origin: usize, connection: u64, handle: &str) -> Result<usize, String> {
        let Some(opened) = self.handles.get(handle) else {
            return Err("no console of yours has that handle".to_owned());
        };
        if opened.origin != origin || opened.connection != connection {
            return Err("that handle belongs to another connection".to_owned());
        }
        Ok(opened.pane)
    }

    /// Forgets the handles that named a pane that is gone.
    fn forget(&mut self, pane: usize) {
        self.handles.retain(|_, opened| opened.pane != pane);
    }

    /// Forgets everything one connection was given, when it goes.
    fn forget_the_connection(&mut self, origin: usize, connection: u64) {
        self.handles
            .retain(|_, opened| !(opened.origin == origin && opened.connection == connection));
    }

    /// Drops every handle naming a console that is not among the living.
    ///
    /// **It runs once per pass, and that is what keeps the map honest without every close path
    /// having to remember**: a pane can go by the chord, by its tab, or by the tree collapsing it,
    /// and a `forget` call in each of those is a rule three places have to keep in step — the shape
    /// that leaves the map naming a console nobody has.
    fn keep_only(&mut self, living: &[usize]) {
        self.handles
            .retain(|_, opened| living.contains(&opened.pane));
    }
}

/// The sentence a handle carries once the person has taken the console over, and the one the pane's
/// own row shows. **One string for both**, so the agent and the person read the same thing.
pub(crate) const HANDED_BACK: &str = "the person typed into this console, so it went back to them";

/// Sixteen random bytes, or nothing when the system will not give them.
fn fresh_handle() -> Option<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The wire's side, as the tiling names it.
fn the_side(side: WireSide) -> Side {
    match side {
        WireSide::Left => Side::Left,
        WireSide::Right => Side::Right,
        WireSide::Top => Side::Top,
        WireSide::Bottom => Side::Bottom,
    }
}

impl Pane {
    /// Marks this console as the person's again, **the instant a byte of their input reaches it**.
    ///
    /// **The condition is that the write happened**, not that a key arrived: what ends an agent's
    /// access is the person typing *into* this console, and a program that swallowed the bytes does
    /// not change whose it is. It is set once, with the first byte, so the sentence an agent reads is
    /// the one from the moment the console was taken.
    pub(crate) fn hand_it_back_to_the_person(&mut self) {
        if self.taken_over.is_none() {
            self.taken_over = Some(HANDED_BACK.to_owned());
            // **And the panel's own row says it too, on a console an agent opened**, which is what
            // the person reads: the flag alone would leave a console that quietly stopped being an
            // agent's with nothing on screen saying so (**AC15**). Whether there is such a row to
            // publish is the panel's own question and it answers it — on every other console the row
            // would announce the end of an access nobody ever had, and it would be the application
            // making a panel appear where no producer published one (**D17**).
            self.panel_state.the_person_took_it_over(HANDED_BACK);
        }
    }
}

/// The three things opening a console needs from the interface pass, as one argument.
///
/// **One argument because the geometry is one thing**: where the window is and how wide a cell is
/// are not two decisions a caller makes, and a signature that took them apart grew past what
/// `clippy` reads as a reasonable number of parameters.
struct OnScreen<'a> {
    ctx: &'a egui::Context,
    whole_window: egui::Rect,
    metrics: &'a CellMetrics,
}

impl Window {
    /// Answers what the open connections left for the tree: opening, reading and closing consoles.
    ///
    /// **It runs in the interface pass**, which is where the geometry a split needs exists, and it
    /// drains a queue the panes filled while they were painted — so a request lands in the frame
    /// after it arrived, which is nothing beside the five seconds its caller waits.
    pub(crate) fn answer_the_windows_requests(
        &mut self,
        ctx: &egui::Context,
        whole_window: egui::Rect,
        metrics: &CellMetrics,
    ) {
        // **What this costs is declared, and it is not zeal**: opening a console is a `fork`/`exec`
        // and a graft inside the interface pass, and the frame's cost is split by subtraction — so
        // whatever is not declared here is published as the framework's cost, the figure D3 judges
        // egui by (**D13**). It goes to `timed_separately`, which is the window's own work and is
        // already subtracted, exactly where the chord's split puts the same two jobs.
        let before = std::time::Instant::now();
        // **Handles that name a console nobody is holding are dropped here, once per pass.** A pane
        // can go by the chord, by a tab, or by the tree collapsing it, and every one of those paths
        // would otherwise have to remember to prune: keeping the two in step by hand is exactly how a
        // map comes to name a console that is gone.
        let living: Vec<usize> = self.all_panes().map(|pane| pane.name).collect();
        self.opened_by_agents.keep_only(&living);

        let mut waiting: Vec<(usize, Delivery)> = Vec::new();
        for pane in self.all_panes_mut() {
            let name = pane.name;
            let Some(control) = pane.control.as_mut() else {
                continue;
            };
            for delivery in control.take_waiting_on_the_window() {
                waiting.push((name, delivery));
            }
        }

        for (origin, delivery) in waiting {
            let answer = match delivery.what {
                What::Open {
                    side,
                    program,
                    args,
                } => self
                    .opening_for_a_connection(
                        origin,
                        delivery.connection,
                        side,
                        program,
                        args,
                        &OnScreen {
                            ctx,
                            whole_window,
                            metrics,
                        },
                    )
                    .map_or_else(Answer::Refused, Answer::Opened),
                What::Read { pane, scrollback } => {
                    self.reading_for_a_connection(origin, delivery.connection, &pane, scrollback)
                }
                What::Close { pane } => {
                    self.closing_for_a_connection(origin, delivery.connection, &pane, ctx)
                }
                // **A closed connection takes its handles with it**, which is what D44 promises and
                // what keeps the map bounded: an agent that opens consoles and dies must not leave
                // names behind for as long as the window lives.
                What::Closed => {
                    self.opened_by_agents
                        .forget_the_connection(origin, delivery.connection);
                    // Nothing waits on a release —the thread that sends it is on its way out— so
                    // there is nobody to answer and nothing to say.
                    continue;
                }
                What::Publish { .. } => {
                    Answer::Refused("that request is not the window's".to_owned())
                }
            };
            let _ = delivery.answer.send(answer);
        }
        self.timed_separately += before.elapsed();
    }

    /// Opens a console beside the one that asked, **without moving the focus**.
    ///
    /// **The focus staying put is the whole difference** from the chord's split, and the reason is
    /// whose gesture it was: whoever presses the chord is opening a console to write in, and an agent
    /// is opening one to read (**AC12**). It also refuses before anything is spent when the pane
    /// cannot give both children their minimum (**AC13**).
    fn opening_for_a_connection(
        &mut self,
        origin: usize,
        connection: u64,
        side: WireSide,
        program: Option<String>,
        args: Vec<String>,
        on_screen: &OnScreen<'_>,
    ) -> Result<Option<String>, String> {
        let OnScreen {
            ctx,
            whole_window,
            metrics,
        } = *on_screen;
        let side = the_side(side);
        let cut_along = side.cut_along();
        let (_, area) = set_the_tab_bar_aside(whole_window, self.tabs.len(), metrics);
        let gone = || "the console that asked is gone".to_owned();

        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.panes().iter().any(|pane| pane.name == origin))
            .ok_or_else(gone)?;
        let at = self.tabs[tab_index]
            .panes()
            .iter()
            .position(|pane| pane.name == origin)
            .ok_or_else(gone)?;
        let its_area = self.tabs[tab_index].root.place(area, metrics.width()).parts[at];
        if !can_split(its_area, cut_along, metrics, self.headers) {
            return Err(
                "the console cannot give both children their minimum; make the window bigger"
                    .to_owned(),
            );
        }

        // **The handle is asked for before the console exists**, and that order is not taste: a
        // `getrandom` that fails after the graft would leave a live console behind an answer that
        // says nothing was opened. Failing here spends nothing and opens nothing.
        let handle = match fresh_handle() {
            Some(handle) => handle,
            None => {
                return Err("the system gave no randomness to name the new console with".to_owned());
            }
        };

        let name = self.next_pane_name;
        let opened = Pane::launching_with_args(
            ctx,
            program.as_deref().unwrap_or(&the_users_shell()),
            &args,
            name,
        );
        // **The name is spent even if the program never starts**, as the chord's split also does:
        // what opens afterwards must not be called the same as what never got to exist.
        self.next_pane_name += 1;
        let mut new = match opened {
            Ok(new) => new,
            Err(failure) => return Err(format!("the console could not be opened ({failure})")),
        };
        // **A console whose panel directory could not be created is not addressable**, and the answer
        // says so with a null handle instead of inventing one: the console exists and the person can
        // use it, but this channel has no identity to hand back for it.
        let addressable = new.panel.is_some();
        // **And its panel is told an agent opened this console, before it is grafted**: from here on
        // it may carry the application's row saying the person took it back (**AC15**), which no
        // console the person opened themselves has anything to say about. It follows `addressable`,
        // because a console this channel cannot name is not one it can lose either.
        if addressable {
            new.panel_state.an_agent_opened_it();
        }

        // A `Some` here is the target having been lost and the console handed back; nothing has been
        // split and it is said, because the console that was built dies on being dropped.
        if self.tabs[tab_index].root.split(origin, side, new).is_some() {
            return Err("the console that asked was not found when splitting".to_owned());
        }
        // **And the focus is left alone.** `graft_the_new_one` moves it for the chord; here the
        // person did not ask for anything, so their keyboard stays where it was.
        self.rearm_the_watcher(ctx);
        if !addressable {
            return Ok(None);
        }
        self.opened_by_agents
            .record(handle.clone(), origin, connection, name);
        Ok(Some(handle))
    }

    /// The console a handle names, **if this connection opened it and the person has not taken it
    /// over**, with the refusal as the caller is to read it.
    ///
    /// **One place resolves a handle**, so reading and closing cannot drift into answering
    /// differently about the same console: a revoked handle answers with the reason and not with a
    /// bare error (**AC15**), which is what lets an agent tell «the person took it» from «you never
    /// had it».
    fn a_console_this_connection_owns(
        &mut self,
        origin: usize,
        connection: u64,
        handle: &str,
    ) -> Result<usize, String> {
        let pane_name = self
            .opened_by_agents
            .reachable(origin, connection, handle)?;
        let Some(pane) = self.all_panes().find(|pane| pane.name == pane_name) else {
            return Err("that console is gone".to_owned());
        };
        match &pane.taken_over {
            Some(why) => Err(why.clone()),
            None => Ok(pane_name),
        }
    }

    /// What a console this connection opened is showing.
    fn reading_for_a_connection(
        &mut self,
        origin: usize,
        connection: u64,
        handle: &str,
        scrollback: bool,
    ) -> Answer {
        let pane_name = match self.a_console_this_connection_owns(origin, connection, handle) {
            Ok(name) => name,
            Err(why) => return Answer::Refused(why),
        };
        let Some(pane) = self.all_panes().find(|pane| pane.name == pane_name) else {
            return Answer::Refused("that console is gone".to_owned());
        };
        // **The visible screen unless the scrollback was asked for**, which is the contract's default
        // and not a policy: the small read is what an agent usually wants, and the whole history is a
        // lot of bytes to hand over by accident.
        let text = if scrollback {
            pane.screen.scrollback_text()
        } else {
            pane.screen.to_text()
        };
        Answer::Text(text)
    }

    /// Closes a console this connection opened.
    fn closing_for_a_connection(
        &mut self,
        origin: usize,
        connection: u64,
        handle: &str,
        ctx: &egui::Context,
    ) -> Answer {
        let pane_name = match self.a_console_this_connection_owns(origin, connection, handle) {
            Ok(name) => name,
            Err(why) => return Answer::Refused(why),
        };
        match self.close_a_pane_called(pane_name, ctx) {
            Ok(()) => {
                self.opened_by_agents.forget(pane_name);
                Answer::Taken
            }
            Err(why) => Answer::Refused(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_control::{Answered, Producer};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{Receiver, SyncSender, channel, sync_channel};
    use std::time::{Duration, Instant};

    /// A pane's worth of the channel, plus the panel it drives.
    struct Driven {
        dir: PanelDir,
        control: PaneControl,
        state: PanelState,
        woken: Arc<AtomicUsize>,
    }

    fn a_driven_pane() -> Driven {
        let dir = PanelDir::create().expect("create the panel directory");
        let woken = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&woken);
        let control = PaneControl::open(&dir, move || {
            counting.fetch_add(1, Ordering::Relaxed);
        })
        .expect("bind the channel");

        Driven {
            dir,
            control,
            state: PanelState::default(),
            woken,
        }
    }

    /// A producer publishing from a thread of its own **and staying connected afterwards**.
    ///
    /// **It exists because the obvious shape of these tests is a disguise.** A thread that owns its
    /// producer and is joined drops the connection when it ends, so the hold is released before the
    /// assertion runs — and a test for «a second source is refused» then passes a build with no
    /// holder rule at all, because by the time the second one publishes the first is gone. Here the
    /// thread parks until it is told to go, which is what a real agent does: it stays connected.
    struct Publishing {
        answered: Receiver<Answered>,
        go: Option<SyncSender<()>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Publishing {
        fn start(mut producer: Producer, source: &str, document: &str) -> Self {
            let (answering, answered) = channel();
            let (go, told) = sync_channel(0);
            let source = source.to_owned();
            let document = document.to_owned();
            let thread = std::thread::spawn(move || {
                let _ = answering.send(
                    producer
                        .publish(&source, &document)
                        .expect("the publication reached the pane"),
                );
                // Parked here, the connection is still open — which is the whole point.
                let _ = told.recv();
            });

            Self {
                answered,
                go: Some(go),
                thread: Some(thread),
            }
        }

        fn answer(&self) -> Answered {
            self.answered
                .recv_timeout(Duration::from_secs(5))
                .expect("the pane answered the publication")
        }

        /// Closes the connection and waits for the thread, so the release is on its way.
        fn disconnect(&mut self) {
            drop(self.go.take());
            if let Some(thread) = self.thread.take() {
                thread.join().expect("the producer's thread finished");
            }
        }

        /// The same, but **a thread that panicked is not a second panic**.
        ///
        /// `disconnect` is called from the destructor, and a producer thread that panics — exactly
        /// what happens when the channel regresses, which is what these tests exist for — would turn
        /// an unwind into an abort: the whole `cargo test` process dies with no test name and no
        /// assertion message. The explicit `disconnect` keeps the `expect`, because whoever calls it
        /// is asserting the thread finished; the destructor cannot.
        fn shut(&mut self) {
            drop(self.go.take());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    impl Drop for Publishing {
        fn drop(&mut self) {
            self.shut();
        }
    }

    impl Driven {
        fn producer(&self) -> Producer {
            Producer::of(self.dir.path(), self.dir.id()).expect("connect as a producer")
        }

        /// Starts a producer that publishes and stays connected, and runs frames until the pane has
        /// taken what it sent.
        fn publishing(&mut self, source: &str, document: &str) -> Publishing {
            let publishing = Publishing::start(self.producer(), source, document);
            self.frames_until(|state| state.held_by().is_some());
            publishing
        }

        /// Runs the apply point until the panel changes, the way frames would.
        ///
        /// **It is a loop and not one call because the publication is on another thread**: waiting
        /// for a fixed sleep would be a test that passes on a fast machine, which in this suite has
        /// already meant a test that covers nothing.
        fn frames_until(&mut self, what: impl Fn(&PanelState) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                self.control.apply(&mut self.state, SystemTime::now());
                if what(&self.state) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("the panel never got there");
        }

        /// Runs the apply point for a while, for the cases where what is expected is that **nothing**
        /// changes: there is no state to wait for, so the wait is the assertion's premise.
        fn frames_for(&mut self, how_long: Duration) {
            let deadline = Instant::now() + how_long;
            while Instant::now() < deadline {
                self.control.apply(&mut self.state, SystemTime::now());
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    /// **AC8, and the one sentence this delivery promises**: what an agent publishes is what the
    /// next pass paints, with no file written anywhere.
    #[test]
    fn what_an_agent_publishes_is_on_the_panel_and_the_file_was_never_written() {
        let mut pane = a_driven_pane();

        let publishing =
            pane.publishing("flow", "{\"phase\":\"build\",\"lines\":[\"two of four\"]}");

        assert_eq!(publishing.answer(), Answered::Ok);
        assert_eq!(
            pane.state
                .last_understood()
                .expect("there is a panel")
                .lines[0]
                .text,
            "two of four"
        );
        assert!(
            !pane.dir.path().join(panel_contract::PANEL_FILE).exists(),
            "publishing over the channel wrote the file"
        );
    }

    /// **AC11**: a second source is refused, and the refusal names who is holding it — not «busy»,
    /// which would leave the caller unable to tell somebody else's connection from its own.
    #[test]
    fn a_second_source_is_refused_by_the_name_of_the_one_holding_it() {
        let mut pane = a_driven_pane();
        let _holding = pane.publishing("flow", "{\"lines\":[\"the one holding it\"]}");

        let intruder = Publishing::start(pane.producer(), "ci", "{\"lines\":[\"mine now\"]}");
        pane.frames_for(Duration::from_millis(200));

        let Answered::Failed(why) = intruder.answer() else {
            panic!("the second source was allowed to publish");
        };
        assert!(
            why.contains("flow"),
            "the refusal does not name the holder: {why}"
        );
        assert_eq!(pane.state.held_by(), Some("flow"));
        assert_eq!(
            pane.state
                .last_understood()
                .expect("there is a panel")
                .lines[0]
                .text,
            "the one holding it",
            "the refused document reached the screen anyway"
        );
    }

    /// **A file publication made while the panel was held is not lost.** The notice the watcher
    /// raised is handed back by the refusal, so the pass that ends the hold reads it — in that same
    /// pass, because the release is applied before the file is consulted. Without it the edit would
    /// be gone for good: a file's date does not change again on its own, so nothing would ever go
    /// back for it.
    #[test]
    fn a_file_written_while_the_panel_was_held_is_read_when_the_hold_ends() {
        let mut pane = a_driven_pane();
        let mut publishing = pane.publishing("flow", "{\"lines\":[\"the agent\"]}");
        assert_eq!(publishing.answer(), Answered::Ok);

        std::fs::write(
            pane.dir.path().join(panel_contract::PANEL_FILE),
            b"{\"lines\":[\"written during the hold\"]}",
        )
        .expect("publish by file");
        // The refusal happens here, with the hold still standing.
        pane.control.apply(&mut pane.state, SystemTime::now());
        assert_eq!(
            pane.state.refresh(pane.dir.path()),
            crate::panel::Refreshed::Declined
        );

        publishing.disconnect();
        pane.frames_until(|state| state.held_by().is_none());
        pane.state.refresh(pane.dir.path());

        assert_eq!(
            pane.state
                .last_understood()
                .expect("there is a panel")
                .lines[0]
                .text,
            "written during the hold",
            "the publication made during the hold was thrown away"
        );
    }

    /// **AC10**: the hold dies with the connection, what it published stays on screen, and the file
    /// takes over again. Without the release, an agent that crashed would freeze this pane's panel
    /// for the life of the window.
    #[test]
    fn when_the_connection_closes_the_hold_ends_and_what_it_said_stays() {
        let mut pane = a_driven_pane();
        let mut publishing = pane.publishing("flow", "{\"lines\":[\"last word\"]}");
        assert_eq!(publishing.answer(), Answered::Ok);

        publishing.disconnect();
        pane.frames_until(|state| state.held_by().is_none());

        assert_eq!(
            pane.state
                .last_understood()
                .expect("the panel is still there")
                .lines[0]
                .text,
            "last word",
            "what the agent last said was thrown away"
        );
        std::fs::write(
            pane.dir.path().join(panel_contract::PANEL_FILE),
            b"{\"lines\":[\"the file again\"]}",
        )
        .expect("publish by file");
        pane.state.refresh(pane.dir.path());
        assert_eq!(
            pane.state
                .last_understood()
                .expect("the panel is there")
                .lines[0]
                .text,
            "the file again"
        );
    }

    /// **AC9, the half that belongs here**: while a connection holds the pane, a file publication is
    /// not read. What the application *says* about that refusal is asserted where the painting
    /// helpers are, in `panel.rs` — against the rows a person would read, never against the field
    /// the refusal is recorded in.
    #[test]
    fn while_it_is_held_a_file_publication_is_not_read() {
        let mut pane = a_driven_pane();
        let _holding = pane.publishing("flow", "{\"lines\":[\"the agent\"]}");

        std::fs::write(
            pane.dir.path().join(panel_contract::PANEL_FILE),
            b"{\"lines\":[\"the file\"]}",
        )
        .expect("publish by file");
        pane.state.refresh(pane.dir.path());

        assert_eq!(
            pane.state
                .last_understood()
                .expect("there is a panel")
                .lines[0]
                .text,
            "the agent",
            "the file was read over a held panel"
        );
    }

    /// The window is woken when something arrives. Without it, a publication would sit in the inbox
    /// until something else happened to wake the window — which, in a window at rest, is never.
    #[test]
    fn a_publication_wakes_the_window() {
        let mut pane = a_driven_pane();

        let _publishing = pane.publishing("flow", "{\"lines\":[]}");

        assert!(pane.woken.load(Ordering::Relaxed) > 0, "nobody was woken");
    }

    /// **The three numbers the report publishes, all of them, against what actually happened.**
    ///
    /// **Two of them were checked by nobody.** The measurement script reads `applied` in both
    /// directions, but a refusal needs a second source and a release needs a connection to end, and
    /// no batch does either — so both travelled to the report on trust. `released` in particular was
    /// counted before the release was attempted, which made it «connections that closed» under a
    /// label that says «connections that gave their pane back»: **D13** rates that worse than
    /// publishing no number at all.
    ///
    /// The connection that publishes nothing is the case that tells the two apart, and it is not a
    /// contrivance: an agent's editor opens the channel to list the tools long before it has anything
    /// to say.
    #[test]
    fn the_three_numbers_the_report_publishes_count_what_they_say_they_count() {
        let mut pane = a_driven_pane();

        // **The connection that publishes nothing is opened and closed first, and that is what
        // removes the vacuity.** The first version asserted `released == 0` after a fixed 200 ms: on
        // a loaded machine the closure had not arrived and the zero passed even against a build that
        // counted every closure as a release. Here its closure is enqueued before the holder's
        // deliveries, so a wrong build reaches two by the end — no zero anywhere.
        let never_publishing = pane.producer();
        drop(never_publishing);

        let mut holding = pane.publishing("flow", "{\"lines\":[\"mine\"]}");
        assert_eq!(holding.answer(), Answered::Ok);

        let intruder = Publishing::start(pane.producer(), "ci", "{\"lines\":[\"mine now\"]}");
        pane.frames_for(Duration::from_millis(200));
        assert!(matches!(intruder.answer(), Answered::Failed(_)));

        holding.disconnect();
        pane.frames_until(|state| state.held_by().is_none());
        // **Drained past the release**: the counts are only final once every queued delivery has been
        // applied, and asserting on a live frame is the race the first version lost.
        pane.frames_for(Duration::from_millis(200));

        let did = pane.control.did();
        assert_eq!(did.applied, 1, "applied");
        assert_eq!(did.refused, 1, "refused");
        assert_eq!(did.released, 1, "released");
    }

    /// A document delivered has no date of its own, so the application stamps its arrival: without
    /// it a held panel would never go stale and an agent that hung with its connection open would
    /// show the same summary for ever.
    #[test]
    fn a_delivered_document_ages_from_the_moment_it_arrived() {
        let mut pane = a_driven_pane();
        let _publishing = pane.publishing(
            "flow",
            "{\"stale_after_minutes\":1,\"lines\":[\"working\"]}",
        );

        let published = pane.state.last_understood().expect("there is a panel");
        let arrival = published.written_at.expect("it was stamped on arrival");

        assert!(!published.is_stale_at(arrival + Duration::from_secs(30)));
        assert!(published.is_stale_at(arrival + Duration::from_secs(120)));
    }

    /// **A request that needs the tree is not applied here and is not lost.** It waits in the pane's
    /// queue for the window's own pass — the only party with a tree — and the panel is untouched by
    /// it. The reply is not asserted here: no answer is sent, because where the request went is the
    /// whole of what this covers.
    #[test]
    fn a_split_request_waits_for_the_window_instead_of_being_applied() {
        use std::io::Write;

        let mut pane = a_driven_pane();
        let mut client = panel_control::endpoint::connect(pane.dir.path(), pane.dir.id())
            .expect("connect to the pane");
        client
            .write_all(b"{\"tool\":\"split_open\",\"side\":\"right\"}\n")
            .expect("ask for a split");

        pane.frames_for(Duration::from_millis(200));

        let waiting = pane.control.take_waiting_on_the_window();
        assert_eq!(
            waiting.len(),
            1,
            "the request did not reach the window's queue"
        );
        assert!(
            matches!(waiting[0].what, What::Open { .. }),
            "the wrong request was queued: {:?}",
            waiting[0].what
        );
        assert!(pane.state.held_by().is_none(), "a split touched the panel");
    }

    /// **A request that waits for the window wakes it**, and that is the difference between an answer
    /// in the next frame and a caller timing out: the frame's drain has already run when a painting
    /// moves the request aside, so without this wake nobody would ask for the pass that answers it.
    /// Without this test, deleting the wake leaves the whole suite green.
    ///
    /// **The two wakes are told apart by counting**: the arrival's is the listener's, and it has
    /// already happened by the time the delivery is in the inbox; the one under test is the second.
    #[test]
    fn a_request_that_waits_for_the_window_wakes_it() {
        use std::io::Write;

        let mut pane = a_driven_pane();
        let mut client = panel_control::endpoint::connect(pane.dir.path(), pane.dir.id())
            .expect("connect to the pane");
        client
            .write_all(b"{\"tool\":\"split_open\",\"side\":\"right\"}\n")
            .expect("ask for a split");

        // The arrival: the listener hands it over and wakes the window. Waiting for it is what makes
        // the count below the deferral's alone.
        let deadline = Instant::now() + Duration::from_secs(5);
        while pane.woken.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let after_arrival = pane.woken.load(Ordering::Relaxed);
        assert!(after_arrival > 0, "the request never arrived at the pane");

        pane.control.apply(&mut pane.state, SystemTime::now());

        assert!(
            pane.woken.load(Ordering::Relaxed) > after_arrival,
            "a request that waits for the window did not wake it"
        );
    }
}
