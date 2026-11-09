//! Desktop reads retain their destination while a clipboard owner answers off the UI thread.
use eframe::egui;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Clipboard,
    Primary,
}

struct Read {
    pane: usize,
    source: Source,
}

struct Reply {
    pane: usize,
    text: Result<String, String>,
}

#[derive(Default)]
struct Mailbox {
    read: Option<Read>,
    primary: Option<(String, Instant)>,
}

struct Worker {
    requests: mpsc::SyncSender<()>,
    replies: mpsc::Receiver<Reply>,
    mailbox: Arc<Mutex<Mailbox>>,
    context: egui::Context,
}

struct Pending {
    deadline: Instant,
    expired: bool,
}

/// One outstanding read keeps late clipboard replies from becoming a different pane's paste.
#[derive(Default)]
pub(crate) struct Service {
    worker: Option<Worker>,
    pending: Option<Pending>,
}

impl Service {
    pub(crate) fn new(context: egui::Context) -> Self {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (sender, replies) = mpsc::channel();
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let selection = mailbox.clone();
        let repaint = context.clone();
        match std::thread::Builder::new()
            .name("clipboard".into())
            .spawn(move || run(receiver, sender, selection, repaint))
        {
            Ok(thread) => {
                // Abandoned on purpose: an unresponsive clipboard owner must not freeze closing.
                // The backend owns its connections; it borrows nothing from the application.
                drop(thread);
                Self {
                    worker: Some(Worker {
                        requests,
                        replies,
                        mailbox,
                        context,
                    }),
                    pending: None,
                }
            }
            Err(error) => {
                eprintln!("Clipboard is unavailable: {error}");
                Self::default()
            }
        }
    }

    pub(crate) fn request(&mut self, pane: usize, source: Source, now: Instant) -> bool {
        if self.pending.is_some() {
            eprintln!("Clipboard is busy; this paste was not requested");
            return false;
        }
        let Some(worker) = &self.worker else {
            eprintln!("Clipboard is unavailable");
            return false;
        };
        let Ok(mut mailbox) = worker.mailbox.lock() else {
            eprintln!("Clipboard mailbox is unavailable");
            return false;
        };
        mailbox.read = Some(Read { pane, source });
        if matches!(
            worker.requests.try_send(()),
            Err(mpsc::TrySendError::Disconnected(()))
        ) {
            mailbox.read = None;
            eprintln!("Clipboard request could not be queued");
            return false;
        }
        drop(mailbox);
        self.pending = Some(Pending {
            deadline: now + DEADLINE,
            expired: false,
        });
        worker.context.request_repaint_after(DEADLINE);
        true
    }

    pub(crate) fn publish_primary(&mut self, text: String) {
        if !cfg!(target_os = "linux") || text.is_empty() {
            return;
        }
        if let Some(worker) = &self.worker {
            // The worker only takes this value under the lock; backend calls never hold it.
            if let Ok(mut mailbox) = worker.mailbox.lock() {
                mailbox.primary = Some((text, Instant::now() + DEADLINE));
                if matches!(
                    worker.requests.try_send(()),
                    Err(mpsc::TrySendError::Disconnected(()))
                ) {
                    eprintln!("Primary selection could not be queued");
                }
            }
        }
    }

    pub(crate) fn poll(&mut self, now: Instant) -> Option<(usize, String)> {
        let worker = self.worker.as_ref()?;
        match worker.replies.try_recv() {
            Ok(reply) => {
                let pending = self.pending.take()?;
                if pending.expired || now >= pending.deadline {
                    if !pending.expired {
                        eprintln!("Clipboard read timed out; its late reply was discarded");
                    }
                    return None;
                }
                match reply.text {
                    Ok(text) if !text.is_empty() => return Some((reply.pane, text)),
                    Ok(_) => {}
                    Err(error) => eprintln!("Clipboard read failed: {error}"),
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                eprintln!("Clipboard worker stopped");
                self.pending = None;
                self.worker = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                if let Some(pending) = &mut self.pending
                    && !pending.expired
                    && now >= pending.deadline
                {
                    pending.expired = true;
                    eprintln!(
                        "Clipboard read timed out; waiting for its owner before another read"
                    );
                }
            }
        }
        None
    }
}

pub(crate) fn deliver(window: &mut crate::Window, context: &egui::Context) {
    if window.clipboard.pending.is_none() {
        return;
    }
    let started = Instant::now();
    if let Some((name, text)) = window.clipboard.poll(Instant::now())
        && let Some(pane) = window.all_panes_mut().find(|pane| pane.name == name)
    {
        crate::keyboard::paste_into(pane, &text);
        context.request_repaint();
    }
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}

fn run(
    requests: mpsc::Receiver<()>,
    replies: mpsc::Sender<Reply>,
    mailbox: Arc<Mutex<Mailbox>>,
    context: egui::Context,
) {
    let mut backend = None;
    let native_context = context.clone();
    run_with(requests, replies, mailbox, context, |read, selection| {
        // Capability/seat setup finishes before operations, without borrowing the app's window.
        let backend = backend.get_or_insert_with(Backend::new);
        if let Some(selection) = selection {
            match backend.as_mut() {
                Ok(backend) => {
                    let focused = native_context.input(|input| input.focused);
                    if let Err(error) =
                        publish_current_selection(selection, Instant::now(), focused, |text| {
                            backend.store_primary(text)
                        })
                    {
                        eprintln!("Primary selection could not be published: {error}");
                    }
                }
                Err(error) => eprintln!("Primary selection is unavailable: {error}"),
            }
        }
        read.map(|read| Reply {
            pane: read.pane,
            text: match backend.as_mut() {
                Ok(backend) => backend.load(read.source),
                Err(error) => Err(error.clone()),
            },
        })
    });
}

fn run_with(
    requests: mpsc::Receiver<()>,
    replies: mpsc::Sender<Reply>,
    mailbox: Arc<Mutex<Mailbox>>,
    context: egui::Context,
    mut operate: impl FnMut(Option<Read>, Option<(String, Instant)>) -> Option<Reply>,
) {
    loop {
        // Bounded: dropping Service closes the only wake sender and ends this idle receive.
        if requests.recv().is_err() {
            break;
        }
        // Backend calls never hold this lock, so the UI never waits for a clipboard owner.
        let (read, selection) = match mailbox.lock() {
            Ok(mut mailbox) => (mailbox.read.take(), mailbox.primary.take()),
            Err(_) => break,
        };
        if read.is_none() && selection.is_none() {
            continue;
        }
        if let Some(reply) = operate(read, selection) {
            if replies.send(reply).is_err() {
                break;
            }
            context.request_repaint();
        }
    }
}

// A blocked clipboard owner must not publish an old selection after the person left the window.
fn publish_current_selection(
    (text, deadline): (String, Instant),
    now: Instant,
    focused: bool,
    publish: impl FnOnce(String) -> Result<(), String>,
) -> Result<(), String> {
    if now >= deadline || !focused {
        return Ok(());
    }
    publish(text)
}

struct Backend(arboard::Clipboard);

impl Backend {
    fn new() -> Result<Self, String> {
        // On Linux arboard tries Wayland data-control, then X11/XWayland. Pure Wayland
        // without data-control or a reachable X server is explicitly unavailable.
        arboard::Clipboard::new()
            .map(Self)
            .map_err(|error| error.to_string())
    }

    fn load(&mut self, source: Source) -> Result<String, String> {
        #[cfg(target_os = "linux")]
        {
            use arboard::GetExtLinux;
            self.0
                .get()
                .clipboard(source.selection())
                .text()
                .map_err(|error| error.to_string())
        }
        #[cfg(not(target_os = "linux"))]
        match source {
            Source::Clipboard => self.0.get_text().map_err(|error| error.to_string()),
            Source::Primary => Err("primary selection is unsupported on this platform".into()),
        }
    }

    fn store_primary(&mut self, text: String) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            self.0
                .set()
                .clipboard(Source::Primary.selection())
                .text(text)
                .map_err(|error| error.to_string())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = text;
            Err("primary selection is unsupported".into())
        }
    }
}

#[cfg(target_os = "linux")]
impl Source {
    fn selection(self) -> arboard::LinuxClipboardKind {
        match self {
            Self::Clipboard => arboard::LinuxClipboardKind::Clipboard,
            Self::Primary => arboard::LinuxClipboardKind::Primary,
        }
    }
}

/// Desktop-free peer for UI integration tests; it uses the real request and reply paths.
#[cfg(test)]
pub(crate) struct Harness {
    wake: mpsc::Receiver<()>,
    mailbox: Arc<Mutex<Mailbox>>,
    replies: mpsc::Sender<Reply>,
}

#[cfg(test)]
impl Harness {
    pub(crate) fn requested(&self) -> Option<(usize, Source)> {
        let _ = self.wake.try_recv();
        self.mailbox
            .lock()
            .unwrap()
            .read
            .take()
            .map(|read| (read.pane, read.source))
    }

    pub(crate) fn reply(&self, pane: usize, text: Result<String, String>) {
        self.replies.send(Reply { pane, text }).unwrap();
    }

    pub(crate) fn primary(&self) -> Option<String> {
        self.mailbox
            .lock()
            .unwrap()
            .primary
            .take()
            .map(|(text, _)| text)
    }
}

#[cfg(test)]
impl Service {
    pub(crate) fn testing_blocked(
        context: egui::Context,
    ) -> (
        Self,
        mpsc::Receiver<()>,
        mpsc::Sender<()>,
        mpsc::Receiver<()>,
    ) {
        let (mut service, harness) = Self::testing();
        service.worker.as_mut().unwrap().context = context.clone();
        let (entered, entering) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (finished, finishing) = mpsc::channel();
        std::thread::spawn(move || {
            run_with(
                harness.wake,
                harness.replies,
                harness.mailbox,
                context,
                |read, _| {
                    read.map(|read| {
                        entered.send(()).unwrap();
                        let text = released
                            .recv_timeout(DEADLINE + DEADLINE)
                            .map(|()| "clipboard completion".into())
                            .map_err(|_| "test clipboard owner did not release".into());
                        finished.send(()).unwrap();
                        Reply {
                            pane: read.pane,
                            text,
                        }
                    })
                },
            );
        });
        (service, entering, release, finishing)
    }

    pub(crate) fn testing() -> (Self, Harness) {
        let (requests, wake) = mpsc::sync_channel(1);
        let (replies, delivered) = mpsc::channel();
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        (
            Self {
                worker: Some(Worker {
                    requests,
                    replies: delivered,
                    mailbox: mailbox.clone(),
                    context: egui::Context::default(),
                }),
                pending: None,
            },
            Harness {
                wake,
                mailbox,
                replies,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Requests {
        wake: mpsc::Receiver<()>,
        mailbox: Arc<Mutex<Mailbox>>,
    }

    impl Requests {
        fn try_recv(&self) -> Result<Read, ()> {
            let _ = self.wake.try_recv();
            self.mailbox.lock().unwrap().read.take().ok_or(())
        }
    }

    fn connected() -> (Service, Requests, mpsc::Sender<Reply>) {
        let (requests, received) = mpsc::sync_channel(1);
        let (replies, delivered) = mpsc::channel();
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let service = Service {
            worker: Some(Worker {
                requests,
                replies: delivered,
                mailbox: mailbox.clone(),
                context: egui::Context::default(),
            }),
            pending: None,
        };
        (
            service,
            Requests {
                wake: received,
                mailbox,
            },
            replies,
        )
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn first_primary_operation_selects_primary_without_a_clipboard_fallback() {
        assert!(matches!(
            Source::Primary.selection(),
            arboard::LinuxClipboardKind::Primary
        ));
        assert!(matches!(
            Source::Clipboard.selection(),
            arboard::LinuxClipboardKind::Clipboard
        ));
        let (mut service, requests, _replies) = connected();
        assert!(service.request(21, Source::Primary, Instant::now()));
        let first = requests.try_recv().unwrap();
        assert_eq!(first.pane, 21);
        assert!(matches!(
            first.source.selection(),
            arboard::LinuxClipboardKind::Primary
        ));
    }

    #[test]
    fn delayed_or_unfocused_primary_never_reaches_the_desktop_writer() {
        let now = Instant::now();
        for (deadline, focused) in [(now, true), (now + DEADLINE, false)] {
            publish_current_selection(("old selection".into(), deadline), now, focused, |_| {
                panic!("stale selection reached the desktop writer")
            })
            .unwrap();
        }
        let mut written = None;
        publish_current_selection(("current".into(), now + DEADLINE), now, true, |text| {
            written = Some(text);
            Ok(())
        })
        .unwrap();
        assert_eq!(written.as_deref(), Some("current"));
    }

    #[test]
    fn default_is_unavailable_without_opening_a_display() {
        let mut service = Service::default();
        assert!(!service.request(2, Source::Clipboard, Instant::now()));
        assert!(service.poll(Instant::now()).is_none());
    }

    #[test]
    fn delayed_reply_keeps_the_original_pane_and_clipboard_kind() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request(17, Source::Primary, now));
        let read = requests.try_recv().unwrap();
        assert_eq!(read.pane, 17);
        assert_eq!(read.source, Source::Primary);
        assert!(service.poll(now).is_none());
        replies
            .send(Reply {
                pane: read.pane,
                text: Ok("selected".into()),
            })
            .unwrap();
        assert_eq!(
            service.poll(now + Duration::from_millis(10)),
            Some((17, "selected".into()))
        );
    }

    #[test]
    fn timeout_does_not_release_a_busy_backend_or_deliver_its_late_text() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request(3, Source::Clipboard, now));
        let read = requests.try_recv().unwrap();
        assert!(service.poll(now + DEADLINE).is_none());
        assert!(!service.request(8, Source::Primary, now + DEADLINE));
        assert!(requests.try_recv().is_err());
        replies
            .send(Reply {
                pane: read.pane,
                text: Ok("stale".into()),
            })
            .unwrap();
        assert!(service.poll(now + DEADLINE).is_none());
        assert!(service.request(8, Source::Primary, now + DEADLINE));
        assert_eq!(requests.try_recv().unwrap().pane, 8);
    }

    #[test]
    fn deadline_applies_even_if_no_frame_polled_while_the_owner_was_busy() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request(1, Source::Clipboard, now));
        let read = requests.try_recv().unwrap();
        replies
            .send(Reply {
                pane: read.pane,
                text: Ok("late".into()),
            })
            .unwrap();
        assert!(service.poll(now + DEADLINE).is_none());
    }

    #[test]
    fn only_one_read_is_queued_and_errors_release_it() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request(4, Source::Clipboard, now));
        assert!(!service.request(5, Source::Primary, now));
        let read = requests.try_recv().unwrap();
        assert_eq!(read.source, Source::Clipboard);
        assert!(requests.try_recv().is_err());
        replies
            .send(Reply {
                pane: read.pane,
                text: Err("empty owner".into()),
            })
            .unwrap();
        assert!(service.poll(now).is_none());
        assert!(service.request(5, Source::Primary, now));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn primary_publication_coalesces_without_issuing_an_ordinary_clipboard_command() {
        let (mut service, requests, _replies) = connected();
        service.publish_primary("first".into());
        service.publish_primary("latest".into());
        service.publish_primary(String::new());
        assert_eq!(
            service
                .worker
                .as_ref()
                .unwrap()
                .mailbox
                .lock()
                .unwrap()
                .primary
                .as_ref()
                .map(|(text, _)| text.as_str()),
            Some("latest")
        );
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn unavailable_primary_does_not_fall_back_to_the_ordinary_clipboard() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request(9, Source::Primary, now));
        let read = requests.try_recv().unwrap();
        assert_eq!(read.source, Source::Primary);
        replies
            .send(Reply {
                pane: read.pane,
                text: Err("primary selection is unsupported".into()),
            })
            .unwrap();
        assert!(service.poll(now).is_none());
        assert!(requests.try_recv().is_err());
        assert!(service.pending.is_none());
    }

    #[test]
    fn worker_disconnection_disables_future_requests() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        drop(replies);
        assert!(service.poll(now).is_none());
        assert!(!service.request(1, Source::Clipboard, now));
        assert!(requests.try_recv().is_err());
    }
}
