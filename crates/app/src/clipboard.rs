//! Desktop reads retain their destination while a clipboard owner answers off the UI thread.
use eframe::egui;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(2);
/// arboard decodes the screenshot inside the read, and a debug build decoded a 2400×1600 PNG in
/// 1.5–4 s under load (#178): the text deadline would expire on an ordinary capture.
const IMAGE_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Clipboard,
    Primary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wanted {
    Text,
    Image,
}

struct Read {
    pane: usize,
    source: Source,
    wanted: Wanted,
}

enum Content {
    Text(Result<String, String>),
    /// `None` when the clipboard holds no image.
    Image(Result<Option<image::RgbaImage>, String>),
}

struct Reply {
    pane: usize,
    content: Content,
}

/// What a clipboard read brought back for the console it was asked for.
#[derive(Debug, PartialEq)]
pub(crate) enum Delivered {
    Text(usize, String),
    Image(usize, Pasted),
}

/// What an image read found. Unlike text, every ending is said in the drawer: a button pressed for
/// nothing visible reads as a button that does not work.
#[derive(Debug, PartialEq)]
pub(crate) enum Pasted {
    Image(image::RgbaImage),
    Absent,
    Failed(String),
    /// The clipboard's owner did not answer in time; whatever it answers later is discarded.
    Expired,
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
    pane: usize,
    wanted: Wanted,
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
        let read = Read {
            pane,
            source,
            wanted: Wanted::Text,
        };
        self.ask(read, now)
    }

    /// Asks for the image on the ordinary clipboard, for the console `pane`.
    pub(crate) fn request_image(&mut self, pane: usize, now: Instant) -> bool {
        let read = Read {
            pane,
            source: Source::Clipboard,
            wanted: Wanted::Image,
        };
        self.ask(read, now)
    }

    /// Whether a read is still outstanding, which refuses another until it ends.
    #[cfg(test)]
    pub(crate) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    fn ask(&mut self, read: Read, now: Instant) -> bool {
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
        let (pane, wanted) = (read.pane, read.wanted);
        mailbox.read = Some(read);
        if matches!(
            worker.requests.try_send(()),
            Err(mpsc::TrySendError::Disconnected(()))
        ) {
            mailbox.read = None;
            eprintln!("Clipboard request could not be queued");
            return false;
        }
        drop(mailbox);
        let deadline = match wanted {
            Wanted::Text => DEADLINE,
            Wanted::Image => IMAGE_DEADLINE,
        };
        self.pending = Some(Pending {
            pane,
            wanted,
            deadline: now + deadline,
            expired: false,
        });
        worker.context.request_repaint_after(deadline);
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

    pub(crate) fn poll(&mut self, now: Instant) -> Option<Delivered> {
        let worker = self.worker.as_ref()?;
        match worker.replies.try_recv() {
            Ok(reply) => {
                let pending = self.pending.take()?;
                if pending.expired || now >= pending.deadline {
                    if pending.expired {
                        return None;
                    }
                    eprintln!("Clipboard read timed out; its late reply was discarded");
                    return expired(&pending);
                }
                match reply.content {
                    Content::Text(Ok(text)) if !text.is_empty() => {
                        return Some(Delivered::Text(reply.pane, text));
                    }
                    Content::Text(Ok(_)) => {}
                    Content::Text(Err(error)) => eprintln!("Clipboard read failed: {error}"),
                    Content::Image(found) => {
                        let pasted = match found {
                            Ok(Some(image)) => Pasted::Image(image),
                            Ok(None) => Pasted::Absent,
                            Err(error) => {
                                eprintln!("Clipboard image read failed: {error}");
                                Pasted::Failed(error)
                            }
                        };
                        return Some(Delivered::Image(reply.pane, pasted));
                    }
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
                    return expired(pending);
                }
            }
        }
        None
    }
}

/// An image read that ran out of time is said once; a text one only on standard error, as before.
fn expired(pending: &Pending) -> Option<Delivered> {
    (pending.wanted == Wanted::Image).then(|| Delivered::Image(pending.pane, Pasted::Expired))
}

pub(crate) fn deliver(window: &mut crate::Window, context: &egui::Context) {
    if window.clipboard.pending.is_none() {
        return;
    }
    let started = Instant::now();
    match window.clipboard.poll(Instant::now()) {
        Some(Delivered::Text(name, text)) => {
            if let Some(pane) = window.all_panes_mut().find(|pane| pane.name == name) {
                crate::keyboard::paste_into(pane, &text);
                context.request_repaint();
            }
        }
        Some(Delivered::Image(name, pasted)) => {
            crate::viewer::image::paste::received(window, name, pasted, context);
            context.request_repaint();
        }
        None => {}
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
            content: match (backend.as_mut(), read.wanted) {
                (Ok(backend), Wanted::Text) => Content::Text(backend.load(read.source)),
                (Ok(backend), Wanted::Image) => Content::Image(backend.load_image()),
                (Err(error), Wanted::Text) => Content::Text(Err(error.clone())),
                (Err(error), Wanted::Image) => Content::Image(Err(error.clone())),
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

    /// The ordinary clipboard's image, or `None` when it holds none — text alone included.
    fn load_image(&mut self) -> Result<Option<image::RgbaImage>, String> {
        let data = match self.0.get_image() {
            Ok(data) => data,
            Err(arboard::Error::ContentNotAvailable) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let (width, height) = (u32::try_from(data.width), u32::try_from(data.height));
        let (Ok(width), Ok(height)) = (width, height) else {
            return Err("the clipboard image is larger than an image can be".into());
        };
        image::RgbaImage::from_raw(width, height, data.bytes.into_owned())
            .map(Some)
            .ok_or_else(|| "the clipboard image's pixels do not match its size".into())
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
        let content = Content::Text(text);
        self.replies.send(Reply { pane, content }).unwrap();
    }

    /// The console an image read was asked for, if the read outstanding is one.
    pub(crate) fn requested_image(&self) -> Option<usize> {
        let _ = self.wake.try_recv();
        let mut mailbox = self.mailbox.lock().unwrap();
        match mailbox.read.take() {
            Some(read) if read.wanted == Wanted::Image => Some(read.pane),
            other => {
                mailbox.read = other;
                None
            }
        }
    }

    pub(crate) fn reply_image(&self, pane: usize, found: Result<Option<image::RgbaImage>, String>) {
        let content = Content::Image(found);
        self.replies.send(Reply { pane, content }).unwrap();
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
                            content: Content::Text(text),
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

    crate::windows_unix_omission!(
        first_primary_operation_selects_primary_without_a_clipboard_fallback,
        "unix-only: exercises Linux primary-selection clipboard API semantics"
    );

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
                content: Content::Text(Ok("selected".into())),
            })
            .unwrap();
        assert_eq!(
            service.poll(now + Duration::from_millis(10)),
            Some(Delivered::Text(17, "selected".into()))
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
                content: Content::Text(Ok("stale".into())),
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
                content: Content::Text(Ok("late".into())),
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
                content: Content::Text(Err("empty owner".into())),
            })
            .unwrap();
        assert!(service.poll(now).is_none());
        assert!(service.request(5, Source::Primary, now));
    }

    crate::windows_unix_omission!(
        primary_publication_coalesces_without_issuing_an_ordinary_clipboard_command,
        "unix-only: exercises Linux primary-selection clipboard API semantics"
    );

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
                content: Content::Text(Err("primary selection is unsupported".into())),
            })
            .unwrap();
        assert!(service.poll(now).is_none());
        assert!(requests.try_recv().is_err());
        assert!(service.pending.is_none());
    }

    fn image_reply(pane: usize, found: Result<Option<image::RgbaImage>, String>) -> Reply {
        Reply {
            pane,
            content: Content::Image(found),
        }
    }

    /// Every way an image read ends reaches the caller, for the console it was asked for; an
    /// empty clipboard is not an error and says so.
    #[test]
    fn an_image_read_delivers_the_image_its_absence_or_its_failure() {
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]));
        let cases = [
            (Ok(Some(image.clone())), Pasted::Image(image)),
            (Ok(None), Pasted::Absent),
            (Err("owner gone".into()), Pasted::Failed("owner gone".into())),
        ];
        for (found, expected) in cases {
            let (mut service, requests, replies) = connected();
            let now = Instant::now();
            assert!(service.request_image(6, now));
            let read = requests.try_recv().unwrap();
            assert_eq!((read.pane, read.wanted), (6, Wanted::Image));
            assert_eq!(read.source, Source::Clipboard);
            replies.send(image_reply(read.pane, found)).unwrap();
            assert_eq!(service.poll(now), Some(Delivered::Image(6, expected)));
            assert!(!service.busy(), "a delivered image read still holds the clipboard");
        }
    }

    /// An image read outlives the text deadline, because arboard decodes inside it; past its own
    /// deadline it is said once, and its late image is discarded.
    #[test]
    fn an_image_read_has_a_longer_deadline_and_its_expiry_is_said_once() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request_image(4, now));
        let read = requests.try_recv().unwrap();
        assert_eq!(service.poll(now + DEADLINE), None, "expired at the text deadline");
        assert_eq!(
            service.poll(now + IMAGE_DEADLINE),
            Some(Delivered::Image(4, Pasted::Expired))
        );
        assert_eq!(service.poll(now + IMAGE_DEADLINE), None, "said twice");
        assert!(service.busy(), "a stuck owner was asked again");
        let late = image::RgbaImage::new(1, 1);
        replies.send(image_reply(read.pane, Ok(Some(late)))).unwrap();
        assert_eq!(service.poll(now + IMAGE_DEADLINE), None);
        assert!(!service.busy());
    }

    /// A late image whose deadline passed with no frame polling in between is said as expired,
    /// never shown.
    #[test]
    fn a_late_image_with_no_poll_in_between_is_said_expired() {
        let (mut service, requests, replies) = connected();
        let now = Instant::now();
        assert!(service.request_image(2, now));
        let read = requests.try_recv().unwrap();
        let late = image::RgbaImage::new(1, 1);
        replies.send(image_reply(read.pane, Ok(Some(late)))).unwrap();
        assert_eq!(
            service.poll(now + IMAGE_DEADLINE),
            Some(Delivered::Image(2, Pasted::Expired))
        );
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
