//! A pane's side of the channel: who is listening, and what reaches the window.
//!
//! **One acceptor thread per pane, one thread per live connection, and not a runtime anywhere.** The
//! whole concurrency of this server is «wait for somebody, read a line, hand it over, answer», which
//! is what an async runtime would be brought in for and then have nothing to do.
//!
//! **Nothing here knows what a panel looks like on screen, and nothing here decides who may
//! publish.** The hold belongs to the window — it is the only party that can see two connections at
//! once — so a request travels there and comes back as an answer. What this module owns is the
//! bytes and the threads.

use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use interprocess::TryClone;
use interprocess::local_socket::{Stream, prelude::*};
use panel_contract::{Dropped, Panel, Patch};

use crate::endpoint;
use crate::session::{Peer, SessionHandler};
use crate::wire::{AgentSnapshot, Framing, Incoming, Reply, Request, Side, write_reply};

#[derive(Clone, Default)]
struct Services {
    reader: Option<ReadDocument>,
    session: Option<SessionHandler>,
}

/// How long a publication waits for the window to take it.
///
/// **Generous on purpose, and it is not a timeout on the work**: applying a document is a handful of
/// microseconds. What this waits for is the window's *next pass*, which is a different thing — a
/// window that is minimised or occluded does not run one at all, exactly as it does not re-read
/// anybody's `panel.json` either. Five seconds is long enough that no ordinary frame misses it and
/// short enough that an agent parked against a minimised window finds out rather than hanging.
const ANSWER_DEADLINE: Duration = Duration::from_secs(5);

/// How long a connection blocks in `read` before it looks at its stop flag.
///
/// **A receive timeout is what lets the thread end without polling.** The socket is blocking, so an
/// idle connection sleeps in the kernel and is woken the instant a byte arrives — an ordinary
/// publication costs no wake-up of its own beyond the one the data causes — and the timeout only
/// bounds how long a thread with nothing to read takes to notice its pane has closed. Two hundred
/// milliseconds is the ceiling the old poll reached at rest, without the five-millisecond eager pass
/// this replaced, which one publication per second used to buy two hundred wake-ups a second for.
const RECV_TIMEOUT: Duration = Duration::from_millis(200);

/// How long the acceptor waits before retrying a transient failure.
///
/// The accept is blocking, so this is only reached after an error; it keeps a run of errors from
/// spinning a core while [`STUMBLES_ALLOWED`] is counted down.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

/// How many failures in a row an acceptor puts up with before giving its pane's channel up.
///
/// **They are counted in a row and reset by any connection**, because what separates a machine that
/// briefly ran out of descriptors from an endpoint that is gone is exactly whether anything succeeds
/// in between. At the resting pace this is a little over two seconds of trying.
const STUMBLES_ALLOWED: u32 = 10;

/// How many connections one pane serves at a time.
///
/// **A bound on threads, not on trust.** Reaching this endpoint already requires being this user, so
/// this is not a defence: it is what stops a client looping on `connect` — an ordinary bug in an
/// agent — from making unbounded threads inside the window's process. Eight is past anything the
/// design imagines (one agent, occasionally a second taking over) and far below where threads
/// matter.
const MAX_CONNECTIONS: usize = 8;

/// A connection, as the window knows it. Opaque, per pane, never reused.
pub type Connection = u64;

/// Reads the last document committed by this pane without asking the window to run a pass.
pub type ReadDocument = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

/// What a connection asks of the window.
#[derive(Debug)]
pub struct Delivery {
    pub connection: Connection,
    pub what: What,
    /// Where the answer goes.
    ///
    /// **Capacity one, so the window never blocks on it**: it answers once per request and returns,
    /// whether or not anybody is still there. A caller that gave up waiting still gets its document
    /// applied, and the reply it read says so honestly — that the window did not answer in time,
    /// never that nothing was published.
    pub answer: SyncSender<Answer>,
}

/// The things a connection ever says.
#[derive(Debug)]
pub enum What {
    /// Complete independent signal or explicit withdrawal from this connection.
    AgentSignal(Option<AgentSnapshot>),
    /// Publish this, under this source.
    Publish { source: String, panel: Panel },
    /// Compatibility path for callers that still let the pane answer `panel_get` in its pass.
    Get,
    /// Change part of this pane's published document, leaving the rest of it alone.
    ///
    /// **The ops arrive already read**, refused on the connection's own thread for everything a
    /// document is not needed to refuse. What is left for the pane is the one question the document
    /// answers — does this key name a line here — plus what applying the answer costs, which scales
    /// with the document and not with the patch (see `panel_contract::patch`).
    Patch { ops: Patch },
    /// Intentionally retire the session this connection currently holds.
    Retire,
    /// Open a pane beside the caller's own, optionally running a program of the caller's choosing.
    ///
    /// **The pane is the caller's, not a name's**: the connection's own endpoint decided it, which is
    /// why no pane identifier travels beside this. `program` and `args` are the only thing the caller
    /// chooses, and there is no environment — see [`crate::wire`].
    Open {
        side: Side,
        program: Option<String>,
        args: Vec<String>,
    },
    /// Read a pane this connection opened.
    Read { pane: String, scrollback: bool },
    /// Close a pane this connection opened.
    Close { pane: String },
    /// The connection is over. Whatever it held is no longer held.
    ///
    /// **It carries no answer anybody waits for**: the thread that sends it is on its way out, and a
    /// release that could be refused would be a hold nothing releases.
    Closed,
}

/// What the window answers a request with.
#[derive(Debug)]
pub enum Answer {
    /// A publication was applied.
    Taken,
    /// Retirement committed, with the auxiliary consoles detached and preserved.
    Retired { closed: usize, preserved: usize },
    /// A pane was opened; the handle is `None` where the pane has no addressable identity.
    Opened(Option<String>),
    /// What a pane showed.
    Text(String),
    /// Compatibility answer for a [`What::Get`] handled by an older channel owner.
    Document(String),
    /// Refused, with the reason as the caller is to read it.
    Refused(String),
}

/// A pane's listening channel. Ends when it is dropped.
pub struct Channel {
    stop: Arc<AtomicBool>,
    /// How many threads this channel still has alive, acceptor included.
    ///
    /// **The count is production's — the acceptor reads its own handle to cap connections — and the
    /// field is the tests'**, which otherwise could only assert that a thread was asked to end and
    /// never that it did. That is the whole of why it is allowed to look unused from here.
    #[cfg_attr(not(test), allow(dead_code))]
    alive: Arc<AtomicUsize>,
    acceptor: Option<JoinHandle<()>>,
    /// Interrupts this pane's listener to wake the acceptor out of `accept`.
    ///
    /// It owns one end of a private descriptor pair rather than the endpoint's pathname: deleting a
    /// pane's directory cannot make the wake unreachable, and a full backlog cannot delay it.
    stopper: Option<endpoint::AcceptorStopper>,
}

impl Channel {
    /// Binds this pane's endpoint and starts listening.
    ///
    /// `wake` is what tells the window there is something to pick up. It is a closure and not the
    /// interface framework's context for the reason this crate exists at all: the framework is going
    /// to be replaced (D3) and nothing about accepting a connection depends on it.
    pub fn open(
        dir: &Path,
        id: &str,
        to_the_window: SyncSender<Delivery>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        Self::open_with_services(dir, id, to_the_window, None, None, wake)
    }

    /// Binds a pane whose document reads are answered without a window pass.
    pub fn open_with_reader(
        dir: &Path,
        id: &str,
        to_the_window: SyncSender<Delivery>,
        read_document: ReadDocument,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        Self::open_with_services(dir, id, to_the_window, Some(read_document), None, wake)
    }

    /// Adds optional off-frame services while retaining the existing panel ownership protocol.
    pub fn open_with_services(
        dir: &Path,
        id: &str,
        to_the_window: SyncSender<Delivery>,
        read_document: Option<ReadDocument>,
        session_handler: Option<SessionHandler>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let services = Services {
            reader: read_document,
            session: session_handler,
        };
        let listener = endpoint::bind(dir, id)?;
        let (stopper, acceptor) = endpoint::acceptor(listener)?;
        let stop = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicUsize::new(1));

        let acceptor = std::thread::Builder::new()
            .name(format!("panel-control {id}"))
            .spawn({
                let stop = Arc::clone(&stop);
                let alive = Arc::clone(&alive);
                // **The pane's name travels with the thread so what it prints can be acted on.**
                // Standard error is shared by every console, so a line naming none of them tells a
                // person that *a* channel is gone and leaves them to guess which — the same gap
                // **D41** records for the console notices, met here where it is cheap to close.
                let id = id.to_owned();
                move || {
                    accept_until_stopped(
                        &id,
                        &acceptor,
                        &stop,
                        &alive,
                        &to_the_window,
                        &services,
                        &wake,
                    )
                }
            })?;

        Ok(Self {
            stop,
            alive,
            acceptor: Some(acceptor),
            stopper: Some(stopper),
        })
    }

    /// How many of this channel's threads are still running.
    #[cfg(test)]
    pub(crate) fn alive(&self) -> usize {
        self.alive.load(Ordering::Relaxed)
    }
}

impl Drop for Channel {
    /// Asks every thread to end, and **does not wait for any of them**.
    ///
    /// **Not joining is the decision.** An idle connection looks at the flag at least once every
    /// [`RECV_TIMEOUT`]; one parked on the window's answer takes at most [`ANSWER_DEADLINE`] longer,
    /// and one writing a reply to a client that stopped reading a further
    /// [`crate::wire::REPLY_DEADLINE`] on top of that — **the worst case is the sum of the two**, and
    /// saying only the first would be the understatement this file exists to avoid. None of them
    /// outlives its pane beyond that and nothing is leaked; what
    /// joining would buy is a pane close that blocks the window for that long — which, against the
    /// 1.2–1.4 ms **D55** measured for closing a console whose program dies of the hang-up, would
    /// have been nearly all of the close. The threads hold only what they
    /// own — their own channel ends and their own `Arc`s — so there is nothing for a survivor to
    /// touch that the pane has taken away.
    ///
    /// **The release on close is the one delivery that waits past that**, and it is deliberate: the
    /// pane's inbox is bounded, and a connection reporting its close uses a blocking `send` rather
    /// than the `try_send` a publication uses. A full inbox therefore parks that thread until the
    /// window runs a pass or the pane is dropped (which disconnects the receiver and returns the
    /// error). Losing the release instead would freeze the pane's panel for as long as the window
    /// stayed minimised, which is worse than a thread that waits for the pass that ends the hold.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // **Closing the private wake descriptor comes before the handle is dropped**, and it is what
        // ends the acceptor: it cannot fail, resolve a filesystem name another field may already
        // have removed, or wait for room in the endpoint's connection backlog.
        drop(self.stopper.take());
        drop(self.acceptor.take());
    }
}

/// Waits for connections until somebody says to stop.
fn accept_until_stopped(
    id: &str,
    acceptor: &endpoint::Acceptor,
    stop: &Arc<AtomicBool>,
    alive: &Arc<AtomicUsize>,
    to_the_window: &SyncSender<Delivery>,
    services: &Services,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let next = AtomicU64::new(0);
    let mut stumbles = 0_u32;

    // **The poll blocks, so this thread costs nothing while nothing happens.** Its second
    // descriptor belongs only to `Channel`: dropping that end wakes this one even after the public
    // endpoint has been removed and without opening another connection.
    while !stop.load(Ordering::Relaxed) {
        // Abandoned on purpose: this acceptor waits without a deadline because either a connection
        // or dropping its channel-owned wake descriptor ends the wait immediately and without a
        // periodic wake-up.
        match acceptor.wait() {
            Ok(endpoint::AcceptorReady::Stop) => break,
            Ok(endpoint::AcceptorReady::Connection) => {}
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => {
                eprintln!(
                    "console {id}: its control channel stopped waiting for connections \
                     ({failure}{}); it publishes by file from now on",
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
        }
        match acceptor.accept() {
            Ok(stream) => {
                stumbles = 0;
                if stop.load(Ordering::Relaxed) {
                    drop(stream);
                    break;
                }
                serve(
                    stream,
                    next.fetch_add(1, Ordering::Relaxed),
                    stop,
                    alive,
                    to_the_window,
                    services,
                    wake,
                );
            }
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => {}
            Err(_) if stop.load(Ordering::Relaxed) => break,
            Err(failure) => {
                // **Running out of descriptors is not the endpoint being gone**, and the first
                // version of this treated every failure as fatal: a terminal host is exactly who
                // hits `EMFILE`, and one transient stumble would have taken that pane's channel
                // away for the life of the window without a word. So the transients are retried,
                // and only a run of them gives up.
                stumbles += 1;
                if stumbles <= STUMBLES_ALLOWED {
                    std::thread::sleep(ACCEPT_RETRY);
                    continue;
                }
                // **And giving up is said.** Failing to bind prints a line; losing the channel
                // afterwards printing nothing was an asymmetry nobody could have diagnosed.
                eprintln!(
                    "console {id}: its control channel stopped accepting connections \
                     ({failure}{}); it publishes by file from now on",
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
        }
    }

    alive.fetch_sub(1, Ordering::Relaxed);
}

/// Gives a connection its thread, unless this pane already has as many as it serves.
fn serve(
    stream: Stream,
    connection: Connection,
    stop: &Arc<AtomicBool>,
    alive: &Arc<AtomicUsize>,
    to_the_window: &SyncSender<Delivery>,
    services: &Services,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    // The acceptor itself is one of the living, so the comparison is against the cap plus it.
    if alive.load(Ordering::Relaxed) > MAX_CONNECTIONS {
        let mut stream = stream;
        // **The refusal is written with a send timeout**, so a client that is not draining its end
        // cannot park this thread for ever: the block that used to be a hand-rolled write loop is
        // now the socket's own deadline. See [`crate::wire::REPLY_DEADLINE`].
        let _ = stream.set_send_timeout(Some(crate::wire::REPLY_DEADLINE));
        let _ = write_reply(
            &mut stream,
            &Reply::Failed(
                "this pane is already serving as many connections as it serves".to_owned(),
            ),
        );
        return;
    }

    let peer = services
        .session
        .as_ref()
        .map(|_| Peer::capture(&stream).map_err(|_| "verified socket peer unavailable".to_owned()));
    alive.fetch_add(1, Ordering::Relaxed);
    let mut refusable = stream.try_clone().ok();
    let spawned = std::thread::Builder::new()
        .name(format!("panel-control connection {connection}"))
        .spawn({
            let stop = Arc::clone(stop);
            let alive = Arc::clone(alive);
            let to_the_window = to_the_window.clone();
            let services = services.clone();
            let wake = Arc::clone(wake);
            move || {
                converse(
                    stream,
                    connection,
                    &stop,
                    &to_the_window,
                    &services,
                    peer,
                    &wake,
                );
                alive.fetch_sub(1, Ordering::Relaxed);
            }
        });

    // **A thread that could not be started is answered, not dropped.** The path above — the
    // connection cap — writes a refusal, and a caller cannot tell «the machine is out of threads»
    // from «my socket was accepted and then ignored» unless this one does too.
    if let Err(failure) = spawned {
        alive.fetch_sub(1, Ordering::Relaxed);
        eprintln!("a pane's control channel could not serve a connection ({failure})");
        if let Some(stream) = refusable.as_mut() {
            let _ = stream.set_send_timeout(Some(crate::wire::REPLY_DEADLINE));
            let _ = write_reply(
                stream,
                &Reply::Failed(format!(
                    "this pane could not serve the connection ({failure})"
                )),
            );
        }
    }
}

/// Reads one connection until it ends, answering every request.
///
/// **The socket blocks, with a receive timeout**, and that is what makes this thread able to end. An
/// idle agent holding its channel open is the ordinary state of the thing this channel exists for,
/// so a thread parked in `read` with no timeout would be the *common* case of a leak rather than a
/// rare one — and unlike a pseudoterminal's reader (**D32**) there is nothing here that a survivor's
/// next byte would ever wake. A timeout returns control to the loop to look at the stop flag,
/// without the eager five-millisecond polling an earlier version used to catch a request split
/// across reads: a blocking read already returns as soon as the rest of the request arrives.
fn converse(
    stream: Stream,
    connection: Connection,
    stop: &AtomicBool,
    to_the_window: &SyncSender<Delivery>,
    services: &Services,
    peer: Option<Result<Peer, String>>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let mut stream = stream;
    // **A connection whose socket will not take the deadline is refused out loud, not dropped.**
    // Returning here left the producer reading "the pane closed the channel without answering" with
    // nothing said anywhere — the silence this crate's own review calls the one failure a caller
    // cannot act on. The stream is still blocking at this point, which is exactly why the refusal
    // can be written before leaving.
    let armed = stream
        .set_recv_timeout(Some(RECV_TIMEOUT))
        .and(stream.set_send_timeout(Some(crate::wire::REPLY_DEADLINE)));
    if let Err(failure) = armed {
        eprintln!(
            "a pane's control channel could not serve a connection: its socket would not take the \
             deadline ({failure})"
        );
        let _ = write_reply(
            &mut stream,
            &Reply::Failed(format!(
                "this pane could not serve the connection ({failure})"
            )),
        );
        return;
    }
    let mut framing = Framing::default();
    let mut chunk = [0_u8; 4096];
    let mut closing_read_answered = false;
    let mut used_panel_protocol = false;
    let mut saw_session = false;

    'connected: loop {
        let closing = stop.load(Ordering::Relaxed);
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => framing.feed(&chunk[..read]),
            // **The timeout expiring is not a failure**: it is the loop coming back to look at the
            // stop flag, which is the whole reason the timeout is set.
            Err(failure) if failure.kind() == io::ErrorKind::WouldBlock => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }
            // **How a read failed is named, never guessed.** **D38** made the pseudoterminal report
            // its `ErrorKind` *and* the operating system's number for precisely this class, and a
            // channel that ends with the failure thrown away puts the next person back to guessing.
            Err(failure) => {
                eprintln!(
                    "a pane's control channel stopped reading a connection ({}{})",
                    failure.kind(),
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
        }

        // **A read already received when closing wins the pane-lifetime race.** Other requests stop
        // at the flag so a pipelined run cannot hold this thread through all of their deadlines;
        // one `panel_get` read once the flag is up still reaches the weak capability and receives
        // either the admitted document or the exact pane-gone refusal instead of losing its reply
        // at transport shutdown. **Only what this thread still reads is covered**: it leaves after
        // the first read that begins with the flag raised, whether that read returns data or times
        // out. A request that read took and did not answer ends in a clean end of file when nothing
        // else is left unread; on Linux, bytes still unread when the socket closes — a request that
        // arrived after the thread had gone, or the rest of a pipeline longer than one read —
        // answer the client with a reset, and a write after the close gets a broken pipe.
        while let Some(incoming) = framing.take() {
            if stop.load(Ordering::Relaxed) {
                if !is_the_one_closing_read(&incoming, closing_read_answered) {
                    break;
                }
                closing_read_answered = true;
            }
            if let Incoming::Session(request) = incoming {
                saw_session = true;
                let reply = match (request, &services.session, &peer) {
                    (Err(why), _, _) => {
                        serde_json::json!({"ok":false,"error":"invalid_request","message":why})
                    }
                    (Ok(request), Some(handler), Some(Ok(peer))) if peer.is_alive() => {
                        handler(peer.clone(), request)
                    }
                    (Ok(_), Some(_), _) => serde_json::json!({"ok":false,"error":"unauthorized"}),
                    (Ok(_), None, _) => serde_json::json!({"ok":false,"error":"disabled"}),
                };
                if stream
                    .write_all(&crate::session::reply_line(reply))
                    .and_then(|()| stream.flush())
                    .is_err()
                {
                    break 'connected;
                }
                continue;
            }
            // Session helpers never acquire panel ownership, enqueue a close, or wake the UI.
            // Malformed session traffic likewise cannot create an owner to release.
            if matches!(&incoming, Incoming::Request(request) if !matches!(request, Request::NotServed(_)))
            {
                used_panel_protocol = true;
            }
            let (reply, last) = answer(incoming, connection, to_the_window, &services.reader, wake);
            if write_reply(&mut stream, &reply).is_err() || last {
                break 'connected;
            }
        }
        if closing {
            break;
        }
    }

    // **The hold dies with the connection, and this is the only place that says so.** Without it a
    // crashed agent would freeze its pane's panel for ever and the file would never be read again.
    if used_panel_protocol || !saw_session {
        let (answering, _unheard) = sync_channel(1);
        let _ = to_the_window.send(Delivery {
            connection,
            what: What::Closed,
            answer: answering,
        });
        wake();
    }
}

/// Only one read already received is admitted after shutdown starts.
fn is_the_one_closing_read(incoming: &Incoming, already_answered: bool) -> bool {
    !already_answered && matches!(incoming, Incoming::Request(Request::PanelGet))
}

/// Turns one understood line into the reply it deserves, and says whether the connection is over.
fn answer(
    incoming: Incoming,
    connection: Connection,
    to_the_window: &SyncSender<Delivery>,
    read_document: &Option<ReadDocument>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> (Reply, bool) {
    match incoming {
        Incoming::Session(_) => (Reply::Failed("session service unavailable".into()), false),
        Incoming::Request(Request::AgentSignal(snapshot)) => (
            ask_the_window(
                Wants::AgentSignal,
                What::AgentSignal(snapshot),
                connection,
                to_the_window,
                wake,
            ),
            false,
        ),
        Incoming::Request(Request::PanelSet { source, document }) => {
            match panel_contract::parse_document_noting(&document) {
                Ok((panel, dropped)) => (
                    with_what_fell(
                        publish(panel, source, connection, to_the_window, wake),
                        &dropped,
                    ),
                    false,
                ),
                Err(problem) => (Reply::Failed(problem.to_string()), false),
            }
        }
        // **The three `split_*` verbs travel to the window and are scoped there**, which is the only
        // party that can see two connections at once and that owns the tree a pane lives in.
        Incoming::Request(Request::SplitOpen {
            side,
            program,
            args,
        }) => (
            ask_the_window(
                Wants::Opened,
                What::Open {
                    side,
                    program,
                    args,
                },
                connection,
                to_the_window,
                wake,
            ),
            false,
        ),
        Incoming::Request(Request::SplitRead { pane, scrollback }) => (
            ask_the_window(
                Wants::Read,
                What::Read { pane, scrollback },
                connection,
                to_the_window,
                wake,
            ),
            false,
        ),
        Incoming::Request(Request::SplitClose { pane }) => (
            ask_the_window(
                Wants::Taken,
                What::Close { pane },
                connection,
                to_the_window,
                wake,
            ),
            false,
        ),
        // **The read never enters the window queue.** A minimised or occluded window may run no
        // interface pass, and recovering this document is most useful precisely then. The callback
        // owns the pane-lifetime boundary and returns one complete committed version.
        Incoming::Request(Request::PanelGet) => (
            match read_document {
                Some(read_document) => match read_document() {
                    Ok(document) => Reply::Document(document),
                    Err(why) => Reply::Failed(why),
                },
                None => ask_the_window(Wants::Document, What::Get, connection, to_the_window, wake),
            },
            false,
        ),
        // **And so is the patch**, for the same reason and one more: it needs the hold, which lives
        // on the panel state this pane holds and nowhere else.
        Incoming::Request(Request::PanelPatch { ops, dropped }) => (
            with_what_fell(
                ask_the_window(
                    Wants::Taken,
                    What::Patch { ops },
                    connection,
                    to_the_window,
                    wake,
                ),
                &dropped,
            ),
            false,
        ),
        Incoming::Request(Request::PanelRetire) => (
            ask_the_window(
                Wants::Retired,
                What::Retire,
                connection,
                to_the_window,
                wake,
            ),
            false,
        ),
        Incoming::Request(Request::NotServed(tool)) => (
            Reply::Failed(format!("this build does not serve \"{tool}\"")),
            false,
        ),
        Incoming::Malformed(why) => (Reply::Failed(why), false),
        Incoming::Unrecoverable(why) => (Reply::Failed(why), true),
    }
}

/// A taken publication or patch, **told what fell away on the way in** when something did.
///
/// **Only a success carries it.** A refused request was not applied, so what its document would have
/// lost is not news — the refusal is the whole answer. And the window never sees the note: it was
/// read on this thread, where the parse happened, and it is only ever the producer's to read
/// (**D58**).
fn with_what_fell(reply: Reply, dropped: &Dropped) -> Reply {
    match reply {
        Reply::Ok => dropped.sentence().map_or(Reply::Ok, Reply::Lossy),
        other => other,
    }
}

/// Handles a document to the window and waits for it to say what it did with it.
fn publish(
    panel: Panel,
    source: String,
    connection: Connection,
    to_the_window: &SyncSender<Delivery>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> Reply {
    ask_the_window(
        Wants::Taken,
        What::Publish { source, panel },
        connection,
        to_the_window,
        wake,
    )
}

/// Which reply shape the caller on the other side can read.
///
/// **It exists so a mismatched answer cannot be handed over as a success.** The window is one party
/// answering four kinds of request, and a bug there — answering `Taken` to a read — would reach the
/// caller as `{"ok":true}` carrying nothing if this were a plain mapping. With it, that answer is
/// refused and says so (**D30**'s rule applied to a reply).
#[derive(Debug, Clone, Copy)]
enum Wants {
    Taken,
    AgentSignal,
    Retired,
    Opened,
    Read,
    Document,
}

impl Wants {
    fn name(self) -> &'static str {
        match self {
            Self::Taken => "a publication or a close",
            Self::AgentSignal => "an agent signal",
            Self::Retired => "a retirement",
            Self::Opened => "an open",
            Self::Read => "a read",
            Self::Document => "a document read",
        }
    }
}

fn temporary_failure(wants: Wants, reason: &str) -> Reply {
    if matches!(wants, Wants::AgentSignal) {
        Reply::Retryable(reason.to_owned())
    } else {
        Reply::Failed(reason.to_owned())
    }
}

/// Hands one request to the window and waits for it to say what it did with it.
fn ask_the_window(
    wants: Wants,
    what: What,
    connection: Connection,
    to_the_window: &SyncSender<Delivery>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> Reply {
    let (answering, answer) = sync_channel(1);
    // **A full queue is refused, not blocked on**, and that is what keeps memory bounded when the
    // window is not draining — a minimised window runs no pass. The producer is told the window is
    // not taking anything right now; it is not owed an applied document, and saying so beats growing
    // the queue without limit.
    match to_the_window.try_send(Delivery {
        connection,
        what,
        answer: answering,
    }) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            return temporary_failure(wants, "the window is not draining its channel");
        }
        Err(TrySendError::Disconnected(_)) => {
            return Reply::Failed("this pane is gone".to_owned());
        }
    }
    wake();

    match answer.recv_timeout(ANSWER_DEADLINE) {
        Ok(Answer::Refused(why)) => Reply::Failed(why),
        Ok(Answer::Taken) if matches!(wants, Wants::Taken | Wants::AgentSignal) => Reply::Ok,
        Ok(Answer::Retired { closed, preserved }) if matches!(wants, Wants::Retired) => {
            Reply::Retired { closed, preserved }
        }
        Ok(Answer::Opened(handle)) if matches!(wants, Wants::Opened) => Reply::Opened(handle),
        Ok(Answer::Text(text)) if matches!(wants, Wants::Read) => Reply::Text(text),
        Ok(Answer::Document(document)) if matches!(wants, Wants::Document) => {
            Reply::Document(document)
        }
        // **A mismatched answer is refused rather than reshaped**, and only its *name* travels: a
        // window that answered `Text` to a close would otherwise echo the console's screen back over
        // the wire inside an error line, which is the opposite of what a refusal is for.
        Ok(other) => Reply::Failed(format!(
            "the window answered {} to {}",
            match other {
                Answer::Taken => "a taken",
                Answer::Retired { .. } => "a retirement",
                Answer::Opened(_) => "an open",
                Answer::Text(_) => "a read",
                Answer::Document(_) => "a document read",
                Answer::Refused(_) => "a refusal",
            },
            wants.name()
        )),
        // **The wording is what the window did, not what happened to the panel**, and the
        // difference is not pedantry: the window may still take this document afterwards, because
        // giving up here cannot reach into a pass that has already started. Saying «it was not
        // published» would be the lie; saying it was not answered in time is true either way.
        Err(RecvTimeoutError::Timeout) => {
            temporary_failure(wants, "the window did not answer within five seconds")
        }
        // **And a pane that is gone is not a slow one.** Folding the two together invited an agent
        // to retry against something that no longer exists, for ever.
        Err(RecvTimeoutError::Disconnected) => Reply::Failed("this pane is gone".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::PanelDir;
    use std::io::Write;
    use std::sync::mpsc::Receiver;
    use std::time::Instant;

    /// A pane with its channel open, plus what the window would hold.
    struct Pane {
        dir: PanelDir,
        channel: Option<Channel>,
        inbox: Receiver<Delivery>,
        woken: Arc<AtomicUsize>,
    }

    fn a_pane() -> Pane {
        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, inbox) = sync_channel(8);
        let woken = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&woken);
        let channel = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            sender,
            Arc::new(|| Err("nothing has been published on this console yet".to_owned())),
            Arc::new(move || {
                counting.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .expect("open the channel");

        Pane {
            dir,
            channel: Some(channel),
            inbox,
            woken,
        }
    }

    impl Pane {
        fn connect(&self) -> Stream {
            // The acceptor may not have reached its first look yet; a connect against a bound
            // endpoint succeeds all the same, because the backlog is the kernel's.
            endpoint::connect(self.dir.path(), self.dir.id()).expect("connect to the pane")
        }

        /// Waits for the window's inbox to carry something, or gives up.
        fn next_delivery(&self) -> Delivery {
            self.inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the window was handed something")
        }
    }

    fn publication(source: &str, document: &str) -> String {
        format!("{{\"tool\":\"panel_set\",\"source\":\"{source}\",\"document\":{document}}}\n")
    }

    fn read_reply(stream: &mut Stream) -> String {
        let mut reply = Vec::new();
        let mut byte = [0_u8; 1];
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match stream.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    reply.push(byte[0]);
                }
                Err(failure) if failure.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(failure) => panic!("reading the reply failed: {failure}"),
            }
        }
        String::from_utf8(reply).expect("the reply is text")
    }

    #[test]
    fn agent_signal_crosses_the_socket_and_disconnect_is_reported() {
        let pane = a_pane();
        let other = a_pane();
        let mut client = pane.connect();
        client.write_all(b"{\"tool\":\"agent_signal\",\"v\":1,\"client\":\"opencode\",\"snapshot\":{\"session\":\"ses_root\",\"activity\":\"busy\",\"permission\":false,\"question\":true}}\n").unwrap();
        let delivery = pane.next_delivery();
        let connection = delivery.connection;
        assert!(
            matches!(delivery.what, What::AgentSignal(Some(ref snapshot)) if snapshot.session == "ses_root" && snapshot.activity == crate::wire::Activity::Busy && snapshot.question && !snapshot.permission)
        );
        assert!(other.inbox.try_recv().is_err());
        delivery.answer.send(Answer::Taken).unwrap();
        assert!(read_reply(&mut client).contains("true"));
        client
            .write_all(
                b"{\"tool\":\"agent_signal\",\"v\":1,\"client\":\"opencode\",\"snapshot\":null}\n",
            )
            .unwrap();
        let withdrawal = pane.next_delivery();
        assert_eq!(withdrawal.connection, connection);
        assert!(matches!(withdrawal.what, What::AgentSignal(None)));
        withdrawal.answer.send(Answer::Taken).unwrap();
        assert!(read_reply(&mut client).contains("true"));
        drop(client);
        let closed = pane.next_delivery();
        assert_eq!(closed.connection, connection);
        assert!(matches!(closed.what, What::Closed));
    }

    #[test]
    fn agent_signal_queue_full_replies_retryable_over_the_socket_but_panel_does_not() {
        let dir = PanelDir::create().unwrap();
        let (sender, _inbox) = sync_channel::<Delivery>(1);
        let (answer, _) = sync_channel(1);
        sender
            .try_send(Delivery {
                connection: 99,
                what: What::Closed,
                answer,
            })
            .unwrap();
        let _channel = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            sender,
            Arc::new(|| Err("no panel".into())),
            Arc::new(|| {}),
        )
        .unwrap();
        let mut stream = endpoint::connect(dir.path(), dir.id()).unwrap();
        stream
            .write_all(
                b"{\"tool\":\"agent_signal\",\"v\":1,\"client\":\"opencode\",\"snapshot\":null}\n",
            )
            .unwrap();
        let retry: serde_json::Value = serde_json::from_str(&read_reply(&mut stream)).unwrap();
        assert_eq!(retry["ok"], false);
        assert_eq!(retry["retryable"], true);
        stream
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .unwrap();
        let legacy: serde_json::Value = serde_json::from_str(&read_reply(&mut stream)).unwrap();
        assert_eq!(legacy["ok"], false);
        assert!(legacy.get("retryable").is_none());
    }

    #[test]
    fn timeout_classification_is_specific_to_agent_signals() {
        assert!(matches!(
            temporary_failure(Wants::AgentSignal, "timeout"),
            Reply::Retryable(_)
        ));
        assert!(matches!(
            temporary_failure(Wants::Taken, "timeout"),
            Reply::Failed(_)
        ));
    }

    /// The whole of what this delivery promises, end to end: a producer publishes over the channel
    /// and the window is handed the document, with **no file written anywhere**.
    #[test]
    fn what_is_published_over_the_channel_reaches_the_window_and_no_file_is_written() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(publication("flow", "{\"lines\":[\"building\"]}").as_bytes())
            .expect("publish");

        let delivery = pane.next_delivery();
        let What::Publish { source, panel } = delivery.what else {
            panic!("the window was handed something else");
        };
        assert_eq!(source, "flow");
        assert_eq!(panel.lines[0].text, "building");
        assert!(
            !pane.dir.path().join(panel_contract::PANEL_FILE).exists(),
            "publishing over the channel wrote the file"
        );
    }

    /// **A full queue refuses the publication instead of growing without limit.** The window does
    /// not drain while it is minimised or occluded, so a producer parked against it would otherwise
    /// pile up one document per connection per timeout — this is the ceiling the queue is bounded
    /// for.
    ///
    /// **The refusal is reached through the ordinary door and not with a test-only capacity.** The
    /// queue is filled to its bound with plain `try_send`s — a buffered send does not wait — and
    /// then `publish` is called directly, which is the same function a real connection reaches.
    /// Driving eight connections through the socket would cost eight `ANSWER_DEADLINE`s instead.
    #[test]
    fn a_publication_is_refused_when_the_window_is_not_draining() {
        let (to_the_window, _inbox) = sync_channel::<Delivery>(1);
        let (answering, _unheard) = sync_channel::<Answer>(1);
        to_the_window
            .try_send(Delivery {
                connection: 0,
                what: What::Closed,
                answer: answering,
            })
            .expect("fill the queue");

        let panel = panel_contract::parse_document("{\"lines\":[]}").expect("a panel");
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let reply = publish(panel, "flow".to_owned(), 1, &to_the_window, &wake);

        assert!(
            matches!(&reply, Reply::Failed(why) if why.contains("not draining")),
            "a full queue did not refuse the publication: {reply:?}"
        );
    }

    /// A read is available precisely when the bounded write queue cannot make progress.
    #[test]
    fn a_document_is_read_while_the_window_queue_is_full_and_nobody_is_woken() {
        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, inbox) = sync_channel::<Delivery>(1);
        let (answering, _unheard) = sync_channel::<Answer>(1);
        sender
            .try_send(Delivery {
                connection: 7,
                what: What::Closed,
                answer: answering,
            })
            .expect("fill the window queue");
        let woken = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&woken);
        let _channel = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            sender,
            Arc::new(|| Ok(r#"{"lines":[]}"#.to_owned())),
            Arc::new(move || {
                counting.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .expect("open the channel");
        let mut client = endpoint::connect(dir.path(), dir.id()).expect("connect to the pane");

        client
            .write_all(b"{\"tool\":\"panel_get\"}\n")
            .expect("ask for the document");

        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":true,"document":{"lines":[]}}"#
        );
        assert!(
            matches!(
                inbox.try_recv().expect("the queued write remains").what,
                What::Closed
            ),
            "reading drained the window queue"
        );
        assert_eq!(woken.load(Ordering::Relaxed), 0, "reading woke the window");
    }

    /// **The closing-read exception, through the connection's own loop and not only its
    /// predicate.** Three reads arrive in one write — on Linux a write this small is taken by one
    /// read, which is what lets the socket close with nothing left unread and the last reply be a
    /// clean end instead of a reset. The pane closes while the first is being answered. The second
    /// is answered once, here with the refusal a closed pane gives; the third, another read, is not,
    /// because the exception admits one and no more.
    #[test]
    fn one_read_is_answered_after_the_pane_closes_and_no_more() {
        use std::sync::Mutex;
        use std::sync::atomic::AtomicBool;

        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, _inbox) = sync_channel::<Delivery>(8);
        let (entered, first_read_began) = sync_channel::<()>(1);
        let (go, resume) = sync_channel::<()>(1);
        let resume = Mutex::new(resume);
        let answered_once = AtomicBool::new(false);
        let channel = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            sender,
            Arc::new(move || {
                if answered_once.swap(true, Ordering::Relaxed) {
                    return Err("this pane is gone".to_owned());
                }
                // The first read parks until the test has closed the pane under it.
                let _ = entered.send(());
                let _ = resume
                    .lock()
                    .expect("the gate")
                    .recv_timeout(Duration::from_secs(5));
                Ok(r#"{"lines":[]}"#.to_owned())
            }),
            Arc::new(|| {}),
        )
        .expect("open the channel");
        let mut client = endpoint::connect(dir.path(), dir.id()).expect("connect to the pane");
        // So the last look can tell the connection ending from the connection going silent, within
        // a deadline and without hanging the suite.
        client
            .set_recv_timeout(Some(Duration::from_millis(100)))
            .expect("a deadline on the client");

        client
            .write_all(
                b"{\"tool\":\"panel_get\"}\n{\"tool\":\"panel_get\"}\n{\"tool\":\"panel_get\"}\n",
            )
            .expect("send the three reads");
        first_read_began
            .recv_timeout(Duration::from_secs(5))
            .expect("the first read reached the pane");
        drop(channel);
        go.send(()).expect("let the first read finish");

        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":true,"document":{"lines":[]}}"#
        );
        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":false,"error":"this pane is gone"}"#,
            "the one read admitted after the pane closed lost its reply"
        );
        // **The end of the connection itself, not the absence of a reply**: silence that outlasts
        // the deadline would read as "" through `read_reply`, and a connection left open is exactly
        // the defect this look has to catch.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut byte = [0_u8; 1];
        let how_it_ended = loop {
            match client.read(&mut byte) {
                Ok(0) => break "ended",
                Ok(_) => break "a second read after the close was served",
                Err(failure) if failure.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break "the connection stayed open after the pane closed";
                    }
                }
                Err(_) => break "the connection was reset instead of ending",
            }
        };
        assert_eq!(how_it_ended, "ended");
    }

    #[test]
    fn shutdown_admits_one_buffered_document_read_and_no_more() {
        let read = Incoming::Request(Request::PanelGet);
        let write = Incoming::Request(Request::PanelSet {
            source: "test".to_owned(),
            document: r#"{"lines":[]}"#.to_owned(),
        });

        assert!(is_the_one_closing_read(&read, false));
        assert!(!is_the_one_closing_read(&read, true));
        assert!(!is_the_one_closing_read(&write, false));
    }

    /// The original Rust constructor and delivery variants remain a source-compatible fallback.
    #[test]
    fn the_original_channel_api_still_hands_panel_get_to_its_owner() {
        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, inbox) = sync_channel(1);
        let _channel = Channel::open(dir.path(), dir.id(), sender, Arc::new(|| {}))
            .expect("open through the original API");
        let mut client = endpoint::connect(dir.path(), dir.id()).expect("connect to the pane");

        client
            .write_all(b"{\"tool\":\"panel_get\"}\n")
            .expect("ask for the document");
        let delivery = inbox
            .recv_timeout(Duration::from_secs(1))
            .expect("the owner receives the old delivery");
        assert!(matches!(delivery.what, What::Get));
        delivery
            .answer
            .send(Answer::Document(r#"{"lines":["compatible"]}"#.to_owned()))
            .expect("answer through the old variant");

        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":true,"document":{"lines":["compatible"]}}"#
        );
    }

    /// The window's answer is what the caller reads, and the two refusals are told apart. Without
    /// this the channel could answer `ok` to everything and every test above would still pass.
    #[test]
    fn the_window_s_answer_is_what_the_caller_reads() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .expect("publish");
        pane.next_delivery()
            .answer
            .send(Answer::Taken)
            .expect("answer");
        assert_eq!(read_reply(&mut client), "{\"ok\":true}");

        client
            .write_all(publication("ci", "{\"lines\":[]}").as_bytes())
            .expect("publish again");
        pane.next_delivery()
            .answer
            .send(Answer::Refused("held by \"flow\"".to_owned()))
            .expect("answer");
        assert_eq!(
            read_reply(&mut client),
            "{\"ok\":false,\"error\":\"held by \\\"flow\\\"\"}"
        );
    }

    /// **What fell away reaches the producer on a taken reply and on no other**, and the window is
    /// handed the same panel it always was. Through the socket, because the note is read on the
    /// connection's thread and attached after the window answers — a test of the helper alone would
    /// pass with nothing calling it.
    #[test]
    fn a_taken_publication_names_what_it_dropped_and_a_refused_one_does_not() {
        let pane = a_pane();
        let mut client = pane.connect();
        let lossy = r#"{"lines":[{"text":"one","mark":"doing"},"two"]}"#;

        client
            .write_all(publication("flow", lossy).as_bytes())
            .expect("publish");
        let delivery = pane.next_delivery();
        let What::Publish { panel, .. } = &delivery.what else {
            panic!("the window was handed something else");
        };
        assert_eq!(
            panel,
            &panel_contract::parse_document(lossy).expect("a panel")
        );
        delivery.answer.send(Answer::Taken).expect("answer");
        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":true,"dropped":"1 line carries an unknown mark \"doing\""}"#
        );

        client
            .write_all(publication("ci", lossy).as_bytes())
            .expect("publish again");
        pane.next_delivery()
            .answer
            .send(Answer::Refused("held by \"flow\"".to_owned()))
            .expect("answer");
        assert_eq!(
            read_reply(&mut client),
            "{\"ok\":false,\"error\":\"held by \\\"flow\\\"\"}"
        );
    }

    #[test]
    fn a_taken_patch_names_what_its_lines_dropped_and_a_refused_one_does_not() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(
                b"{\"tool\":\"panel_patch\",\"ops\":[{\"op\":\"last\",\"line\":{\"text\":\"x\",\"style\":\"bold\"}}]}\n",
            )
            .expect("patch");
        let delivery = pane.next_delivery();
        assert!(matches!(delivery.what, What::Patch { .. }));
        delivery.answer.send(Answer::Taken).expect("answer");

        assert_eq!(
            read_reply(&mut client),
            r#"{"ok":true,"dropped":"1 line carries an unknown style \"bold\""}"#
        );

        client
            .write_all(
                b"{\"tool\":\"panel_patch\",\"ops\":[{\"op\":\"last\",\"line\":{\"text\":\"y\",\"mark\":\"wip\"}}]}\n",
            )
            .expect("patch again");
        pane.next_delivery()
            .answer
            .send(Answer::Refused("held by \"flow\"".to_owned()))
            .expect("answer");
        assert_eq!(
            read_reply(&mut client),
            "{\"ok\":false,\"error\":\"held by \\\"flow\\\"\"}",
            "a refused patch said what its lines would have lost"
        );
    }

    /// Two connections at once, which is what the holder rule needs to exist at all: served
    /// serially, the second caller could never be heard and the refusal could never happen.
    #[test]
    fn two_connections_are_heard_at_the_same_time() {
        let pane = a_pane();
        let mut first = pane.connect();
        let mut second = pane.connect();

        first
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .expect("the first publishes");
        second
            .write_all(publication("ci", "{\"lines\":[]}").as_bytes())
            .expect("the second publishes");

        let one = pane.next_delivery();
        let other = pane.next_delivery();
        assert_ne!(
            one.connection, other.connection,
            "both arrived on one connection"
        );
    }

    /// **The release is what stops a crashed agent freezing its pane's panel for ever.** It is
    /// asserted on the closing of the socket and not on a tidy goodbye, because a crash does not
    /// send one.
    #[test]
    fn a_connection_that_dies_releases_what_it_held() {
        let pane = a_pane();
        let mut client = pane.connect();
        client
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .expect("publish");
        let published = pane.next_delivery();
        let _ = published.answer.send(Answer::Taken);

        drop(client);

        let closing = pane.next_delivery();
        assert!(matches!(closing.what, What::Closed));
        assert_eq!(closing.connection, published.connection);
    }

    /// **A pane that closes takes its threads with it.** Asserted by watching them end, because
    /// «the flag was raised» is exactly the claim that was true of the pseudoterminal's reader while
    /// it hung the window for a month (**D32**).
    #[test]
    fn closing_the_pane_ends_every_thread_it_had() {
        let mut pane = a_pane();
        let _first = pane.connect();
        let _second = pane.connect();
        let channel = pane.channel.as_ref().expect("the channel is open");
        let deadline = Instant::now() + Duration::from_secs(5);
        while channel.alive() < 3 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(channel.alive(), 3, "acceptor and two connections");
        let watching = Arc::clone(&channel.alive);

        pane.channel = None;

        let deadline = Instant::now() + Duration::from_secs(5);
        while watching.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            watching.load(Ordering::Relaxed),
            0,
            "threads outlived their pane"
        );
    }

    /// The endpoint's path is an address, not the acceptor's lifetime. A pane removes that path as
    /// part of closing, and the channel must still be able to stop the listener it already owns.
    #[test]
    fn closing_after_the_endpoint_path_is_removed_still_ends_the_acceptor() {
        let mut pane = a_pane();
        let _client = pane.connect();
        let watching = Arc::clone(&pane.channel.as_ref().expect("the channel is open").alive);
        let deadline = Instant::now() + Duration::from_millis(500);
        while watching.load(Ordering::Relaxed) < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            watching.load(Ordering::Relaxed),
            2,
            "the acceptor and client"
        );

        let (closed, did_close) = sync_channel(1);
        let channel = pane.channel.take();
        std::thread::spawn(move || {
            drop(pane.dir);
            drop(channel);
            let _ = closed.send(());
        });
        did_close
            .recv_timeout(Duration::from_millis(500))
            .expect("closing the channel itself did not return within 500 ms");

        let deadline = Instant::now() + Duration::from_millis(500);
        while watching.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            watching.load(Ordering::Relaxed),
            0,
            "removing the endpoint path before dropping the channel left its acceptor alive"
        );
    }

    /// A document that is not a panel is refused **without reaching the window**: tolerance is per
    /// field inside a document, never per document.
    #[test]
    fn a_document_that_is_not_a_panel_never_reaches_the_window() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(publication("flow", "{\"no\":\"lines\"}").as_bytes())
            .expect("publish");
        let reply = read_reply(&mut client);

        assert!(reply.contains("\"ok\":false"), "{reply}");
        assert!(
            pane.inbox.try_recv().is_err(),
            "an invalid document reached the window"
        );
    }

    /// The verbs of the frozen format that nobody serves are refused by name without reaching the
    /// window, and the connection survives to be used for what it does serve.
    #[test]
    fn an_unknown_tool_is_refused_by_name_and_the_connection_lives_on() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(b"{\"tool\":\"panel_dance\"}\n")
            .expect("ask for something nobody serves");
        let refusal = read_reply(&mut client);
        assert!(refusal.contains("panel_dance"), "{refusal}");

        client
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .expect("publish afterwards");
        assert!(matches!(pane.next_delivery().what, What::Publish { .. }));
    }

    /// **The three `split_*` verbs travel to the window and its answer comes back to the caller.**
    /// The window here is the test: it takes each delivery, checks what it was asked, and answers —
    /// which is what pins that these no longer take the refusal path of an unknown tool, and that
    /// each verb's reply shape is the one the wire declared.
    #[test]
    fn the_split_verbs_travel_to_the_window_and_its_answer_returns() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(b"{\"tool\":\"split_open\",\"side\":\"right\"}\n")
            .expect("ask for a split");
        let delivery = pane.next_delivery();
        let What::Open {
            side,
            program,
            args,
        } = delivery.what
        else {
            panic!("the window was not asked to open: {:?}", delivery.what);
        };
        assert_eq!(side, Side::Right);
        assert_eq!(program, None, "a program nobody named was invented");
        assert!(args.is_empty());
        delivery
            .answer
            .send(Answer::Opened(Some("h-1".to_owned())))
            .expect("answer the open");
        assert_eq!(read_reply(&mut client), "{\"ok\":true,\"pane\":\"h-1\"}");

        client
            .write_all(b"{\"tool\":\"split_read\",\"pane\":\"h-1\",\"scrollback\":true}\n")
            .expect("read it");
        let delivery = pane.next_delivery();
        let What::Read {
            pane: handle,
            scrollback,
        } = delivery.what
        else {
            panic!("the window was not asked to read: {:?}", delivery.what);
        };
        assert_eq!(handle, "h-1");
        assert!(scrollback, "the scrollback flag did not travel");
        delivery
            .answer
            .send(Answer::Text("hello".to_owned()))
            .expect("answer the read");
        assert_eq!(read_reply(&mut client), "{\"ok\":true,\"text\":\"hello\"}");

        client
            .write_all(b"{\"tool\":\"split_close\",\"pane\":\"h-1\"}\n")
            .expect("close it");
        let delivery = pane.next_delivery();
        assert!(
            matches!(delivery.what, What::Close { .. }),
            "the window was not asked to close: {:?}",
            delivery.what
        );
        delivery
            .answer
            .send(Answer::Taken)
            .expect("answer the close");
        assert_eq!(read_reply(&mut client), "{\"ok\":true}");
    }

    /// Retirement keeps the connection identity and returns the window's exact outcome counts.
    #[test]
    fn retirement_travels_to_the_window_and_its_counts_return() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(b"{\"tool\":\"panel_retire\"}\n")
            .expect("retire");
        let delivery = pane.next_delivery();
        assert!(matches!(delivery.what, What::Retire));
        delivery
            .answer
            .send(Answer::Retired {
                closed: 3,
                preserved: 1,
            })
            .expect("answer retirement");

        assert_eq!(
            read_reply(&mut client),
            "{\"ok\":true,\"closed\":3,\"preserved\":1}"
        );
    }

    /// **A patch is the pane's, like a publication and unlike the three verbs above**, and its ops
    /// reach it already read.
    ///
    /// The window here is the test: it takes the delivery, checks what it was handed, and answers.
    /// That is what pins that this verb no longer falls to the unknown-tool refusal, that what
    /// travels is the read op list and not the text it arrived as, and that an empty `ok` is the
    /// shape the wire declared for it — a reply wanting any other shape turns the pane's answer into
    /// a refusal the producer cannot act on.
    #[test]
    fn a_patch_travels_to_the_pane_with_its_ops_already_read() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(
                b"{\"tool\":\"panel_patch\",\"ops\":[{\"op\":\"delete\",\"key\":\"mr-1\"},\
                  {\"op\":\"set\",\"field\":\"attention\",\"value\":\"done\"}]}\n",
            )
            .expect("ask for a patch");

        let delivery = pane.next_delivery();
        let What::Patch { ops } = delivery.what else {
            panic!("the pane was not asked to patch: {:?}", delivery.what);
        };
        assert_eq!(
            ops.ops(),
            [
                panel_contract::Op::Delete {
                    key: "mr-1".to_owned()
                },
                panel_contract::Op::Set(panel_contract::Scalar::Attention(Some(
                    panel_contract::Attention::Done
                ))),
            ]
        );

        delivery
            .answer
            .send(Answer::Taken)
            .expect("answer the patch");
        assert_eq!(read_reply(&mut client), "{\"ok\":true}");
    }

    /// **A window that answers the wrong shape is refused rather than handed over as a success**, and
    /// only the shape's name travels: a `Taken` reaching a read would otherwise come back as
    /// `{"ok":true}` carrying no text — the silent failure this crate's reply path exists to avoid —
    /// and a read reaching a close would echo a console's screen into an error line.
    #[test]
    fn a_mismatched_answer_is_refused_rather_than_reshaped() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(b"{\"tool\":\"split_read\",\"pane\":\"h-1\"}\n")
            .expect("read it");
        let delivery = pane.next_delivery();
        delivery.answer.send(Answer::Taken).expect("answer wrongly");
        let refusal = read_reply(&mut client);

        assert!(
            refusal.contains("answered a taken"),
            "a mismatched answer was not refused: {refusal}"
        );
        assert!(
            !refusal.contains("ok\":true"),
            "the mismatched answer was handed over as a success: {refusal}"
        );
    }

    /// The window is told there is something to pick up. Without it a publication would sit in the
    /// inbox until something else happened to wake the window — which, in a window at rest, is
    /// never.
    #[test]
    fn publishing_wakes_the_window() {
        let pane = a_pane();
        let mut client = pane.connect();

        client
            .write_all(publication("flow", "{\"lines\":[]}").as_bytes())
            .expect("publish");
        let _ = pane.next_delivery();

        let deadline = Instant::now() + Duration::from_secs(5);
        while pane.woken.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(pane.woken.load(Ordering::Relaxed) > 0, "nobody was woken");
    }

    /// **A client looping on `connect` does not make unbounded threads inside the window.** The
    /// assertion is on the thread count and not only on the refusal: a cap that answered «no» and
    /// spawned anyway would pass a test that only read the reply.
    #[test]
    fn a_pane_serves_as_many_connections_as_it_serves_and_no_more() {
        let pane = a_pane();
        let channel = pane.channel.as_ref().expect("the channel is open");
        let mut held = Vec::new();

        for _ in 0..MAX_CONNECTIONS {
            held.push(pane.connect());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while channel.alive() < MAX_CONNECTIONS + 1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            channel.alive(),
            MAX_CONNECTIONS + 1,
            "acceptor and its eight"
        );

        let mut over = pane.connect();
        let refusal = read_reply(&mut over);

        assert!(refusal.contains("\"ok\":false"), "{refusal}");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            channel.alive(),
            MAX_CONNECTIONS + 1,
            "the refused connection got a thread anyway"
        );
    }
}
