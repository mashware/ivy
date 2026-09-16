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

use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::SystemTime;

use panel_contract::PanelDir;
use panel_control::{Answer, Channel, Delivery, What};

use crate::measure::Control;
use crate::panel::{PanelState, Released};

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
        match Channel::open(dir.path(), dir.id(), sender, Arc::new(wake)) {
            Ok(channel) => Some(Self {
                _channel: channel,
                inbox,
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
                }
            }
        }
    }
}

impl PaneControl {
    /// What this pane's channel has done, for the report.
    pub fn did(&self) -> Control {
        self.did
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
}
