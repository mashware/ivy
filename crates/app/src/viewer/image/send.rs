//! Send: a copy of the image with its marks written into the origin console's own folder, then one
//! line naming the copy, with the notes, typed into that console and submitted with Enter.
use super::super::delivery::{self, Enter, Refusal};
use super::tools::Asked;
use super::{Slot, flatten};
use crate::viewer::load::{Job, take};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The copies Send delivered to one console, removed when the console closes. Only the paths it
/// recorded go, with `remove_file`, which unlinks a link rather than what it points to; whatever
/// else the console's program left in its directory stays, and keeps the directory.
#[derive(Default)]
pub(crate) struct Written(Vec<PathBuf>);

impl Written {
    pub(super) fn record(&mut self, path: PathBuf) {
        self.0.push(path);
    }
}

impl Drop for Written {
    /// A copy someone already removed is not worth a word; one that stays is, as `PanelDir` says
    /// of the directory it could not remove.
    fn drop(&mut self) {
        for path in &self.0 {
            if let Err(error) = std::fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("the annotated copy {} remains ({error})", path.display());
            }
        }
    }
}

/// What a click on Send fixed: the image it was for, the console, the copy's name and the line
/// painted under the image and its notes.
#[derive(Clone)]
pub(super) struct Request {
    /// Which opening of an image this Send belongs to, checked against its console's slot again
    /// before the copy starts and before the line is typed. That console refuses another image
    /// while its marks are unsent, which they stay until the Send delivers; this id keeps the line
    /// off another image should that refusal ever give.
    pub(super) document: u64,
    pub(super) pane: usize,
    /// The console's name at the click: what another console's greyed Send waits for, and what
    /// every line this Send tells standard error is prefixed with.
    pub(super) console: String,
    pub(super) path: PathBuf,
    pub(super) line: String,
    /// An explicit answer permits the line despite later typing (D113).
    pub(super) answer: Option<Answer>,
}

/// Whether the person chose to submit the annotated line or leave it at the prompt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Answer {
    Anyway,
    WithoutEnter,
}

/// Where a Send is, from the click to the Enter; one at a time.
#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    /// Clicked; the copy starts on the next frame's input pass.
    Asked(Request),
    /// D113: wait for an answer, holding the path if the copy was already written.
    Confirm(Request, Option<PathBuf>),
    Writing(Request, Job<PathBuf>),
    Written(Request, PathBuf),
    /// The Enter owed for a line written in the frame named here by the Send last; it is pressed
    /// in a later one.
    Enter(Enter, u64, Request),
}

#[derive(Default)]
pub(crate) struct Sending {
    /// Where the copy would go for the image's console this frame, or why it cannot.
    pub(super) target: Option<Result<PathBuf, String>>,
    phase: Phase,
    /// The worker of a Send cancelled while it wrote its copy. A write cannot be interrupted, so
    /// it is kept and polled until it ends, and what it wrote is removed then; no other Send
    /// starts meanwhile, as the decoder's abandoned slot keeps a second decode from starting, so
    /// two workers never race for one copy name (#197).
    parked: Option<Parked>,
}

/// A cancelled Send's worker, kept until it ends.
struct Parked {
    request: Request,
    job: Job<PathBuf>,
    /// Whether standard error was already told that its console closed: a console closed with its
    /// Send in flight is said once, however the worker ends.
    closed_told: bool,
}

impl Sending {
    pub(super) fn busy(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// Whether a cancelled Send's worker is still writing its copy.
    #[cfg(test)]
    pub(super) fn parked(&self) -> bool {
        self.parked.is_some()
    }

    /// Whether `poll` has anything to do: a Send on its way, or a cancelled one's worker.
    pub(super) fn pending(&self) -> bool {
        self.busy() || self.parked.is_some()
    }

    /// Whether the Send whose line is not typed yet was clicked on the image opened as
    /// `document`, which freezes that image's marks and no other console's.
    pub(super) fn busy_for(&self, document: u64) -> bool {
        self.in_flight().is_some_and(|r| r.document == document)
    }

    /// What the Send button of the image opened as `document` stands for. The label and whether
    /// the button answers are both read from this one value, so a button greyed by a Send says why.
    pub(super) fn button(&self, document: u64) -> Button<'_> {
        match &self.phase {
            Phase::Confirm(r, _) if r.document == document => Button::Confirm,
            Phase::Asked(r) | Phase::Writing(r, _) | Phase::Written(r, _)
                if r.document == document =>
            {
                Button::Sending
            }
            Phase::Asked(r)
            | Phase::Writing(r, _)
            | Phase::Written(r, _)
            | Phase::Confirm(r, _) => Button::Waiting(&r.console),
            Phase::Enter(_, _, r) if r.document == document => Button::Entering,
            Phase::Enter(_, _, r) => Button::Waiting(&r.console),
            Phase::Idle => match &self.parked {
                Some(parked) => Button::Finishing(&parked.request.console),
                None => Button::Free,
            },
        }
    }

    pub(super) fn confirming(&self, document: u64) -> bool {
        matches!(&self.phase, Phase::Confirm(r, _) if r.document == document)
    }

    /// Takes up what a click in the notes asked, saying in `notice` what became of it.
    pub(super) fn answer(&mut self, asked: Option<Asked>, notice: &mut Option<String>) {
        match asked {
            Some(Asked::Send(request)) => {
                self.ask(request);
                *notice = None;
            }
            Some(Asked::Answer(document, answer)) if self.confirming(document) => {
                let Phase::Confirm(mut request, copy) = std::mem::take(&mut self.phase) else {
                    unreachable!("confirming");
                };
                request.answer = Some(answer);
                self.phase = match copy {
                    Some(path) => Phase::Written(request, path),
                    None => Phase::Asked(request),
                };
                *notice = None;
            }
            Some(Asked::Cancel(document)) => {
                if let Some(said) = self.cancel(|r| r.document == document) {
                    *notice = Some(said);
                }
            }
            _ => {}
        }
    }

    /// D113: Cancel is scoped to its image and removes a completed copy immediately.
    fn cancel(&mut self, which: impl FnOnce(&Request) -> bool) -> Option<String> {
        if !self.in_flight().is_some_and(which) {
            return None;
        }
        let said = match std::mem::take(&mut self.phase) {
            Phase::Confirm(_, Some(path)) | Phase::Written(_, path) => {
                match std::fs::remove_file(&path) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => format!(
                        "{CANCELLED} The copy at {} could not be removed: {error}.",
                        path.display()
                    ),
                    _ => CANCELLED.into(),
                }
            }
            Phase::Writing(request, job) => {
                debug_assert!(self.parked.is_none(), "a second worker parked");
                self.parked = Some(Parked {
                    request,
                    job,
                    closed_told: false,
                });
                format!("{CANCELLED} {STILL_FINISHING}.")
            }
            _ => CANCELLED.into(),
        };
        Some(said)
    }

    /// A click on Send, taken up on the next frame. One at a time: a click while another Send is
    /// busy would drop that one's worker and its owed Enter, so it asks nothing.
    pub(super) fn ask(&mut self, request: Request) {
        if self.busy() {
            return;
        }
        self.phase = Phase::Asked(request);
    }

    /// The Send whose line is not typed yet.
    fn in_flight(&self) -> Option<&Request> {
        match &self.phase {
            Phase::Asked(request)
            | Phase::Writing(request, _)
            | Phase::Confirm(request, _)
            | Phase::Written(request, _) => Some(request),
            _ => None,
        }
    }
}

impl Drop for Sending {
    fn drop(&mut self) {
        if let Phase::Confirm(_, Some(path)) | Phase::Written(_, path) = &self.phase
            && let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("the annotated copy {} remains ({error})", path.display());
        }
    }
}

/// What a Send button stands for, given the one Send the window runs at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Button<'a> {
    Confirm,
    /// Nothing stands in the way of this image's Send.
    Free,
    /// This image's Send is on its way, and Cancel beside it can take it back until its line is
    /// typed: for the whole write rather than after a delay, because a slow disk is not a stuck
    /// one and no constant tells them apart; the person does.
    Sending,
    /// This image's Send typed its line and owes the Enter: past Cancel, which takes back only a
    /// line not typed yet.
    Entering,
    /// The Send of the console named here is on its way: one runs at a time.
    Waiting(&'a str),
    /// The cancelled Send of the console named here still has its worker writing a copy, which
    /// may take the name this one would write.
    Finishing(&'a str),
}

/// The name a Send gives its console wherever another console's Send or standard error reads it:
/// its label, with the number the app gives every console unless the label already is it, since
/// two consoles routinely share a title such as the program they run.
pub(super) fn console_name(origin: &crate::viewer::Source) -> String {
    let number = format!("Console {}", origin.pane + 1);
    if origin.label == number {
        number
    } else {
        format!("{} ({number})", origin.label)
    }
}

#[cfg(test)]
impl Sending {
    pub(super) fn question(&mut self, request: Request) {
        self.phase = Phase::Confirm(request, None);
    }

    /// The Send has not yet written its line.
    pub(super) fn before_the_line(&self) -> bool {
        matches!(
            self.phase,
            Phase::Asked(_) | Phase::Writing(..) | Phase::Confirm(..) | Phase::Written(..)
        )
    }

    pub(super) fn asked(&self) -> bool {
        matches!(self.phase, Phase::Asked(_))
    }

    pub(super) fn writing(&self) -> bool {
        matches!(self.phase, Phase::Writing(..))
    }

    /// Makes the copy being written one that has not finished, for as long as the returned sender
    /// lives, and says where it was going. The worker it replaces cannot be stopped, so it is
    /// waited for and what it wrote removed: otherwise it can write the copy after the test has
    /// written or removed its own, and the test reads a file nobody meant.
    pub(super) fn stall(
        &mut self,
    ) -> Option<(PathBuf, std::sync::mpsc::Sender<Result<PathBuf, String>>)> {
        let Phase::Writing(request, job) = &mut self.phase else {
            return None;
        };
        let (stalled, sender) = crate::viewer::load::stalled();
        let real = std::mem::replace(job, stalled);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let written = loop {
            if let Some(result) = take(&real) {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the copy's worker never ended"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        if let Ok(path) = written {
            std::fs::remove_file(path).expect("the worker's copy is removed");
        }
        Some((request.path.clone(), sender))
    }
}

/// The line Send types: the copy's path, each marker's note under its number, and the general note,
/// with the empty ones left out. Only the copy's path goes in, never the path a program printed:
/// the copy's name is the app's own, the original's is whatever the program chose. Every control
/// character becomes a space, so the line shown is the line sent: a line break would split it in a
/// console that does not mark pastes, a tab reaches the program as a key (completion in a shell),
/// the paste drops the other C0 characters and passes the C1 ones on to the program, where they are
/// not shown as themselves.
pub(crate) fn compose(copy: &Path, notes: &[&str], general: &str) -> String {
    let clean = |text: &str| text.replace(char::is_control, " ").trim().to_owned();
    let mut line = format!("Annotated screenshot: {}", copy.display());
    for (index, note) in notes.iter().enumerate() {
        let note = clean(note);
        if !note.is_empty() {
            line.push_str(&format!(" · {}: {note}", index + 1));
        }
    }
    let general = clean(general);
    if !general.is_empty() {
        line.push_str(&format!(" · Note: {general}"));
    }
    line
}

/// Where Send writes the copy for the console `name`: its own folder, under the first number no
/// entry there takes, or why it cannot. A name is taken by anything at it, a dangling link
/// included, because the copy is created with `create_new`, which refuses all of them.
fn copy_path(window: &crate::Window, name: usize) -> Result<PathBuf, String> {
    let pane = window
        .all_panes()
        .find(|pane| pane.name == name)
        .ok_or_else(|| Refusal::Closed.to_string())?;
    let folder = pane.panel.as_ref().ok_or_else(|| {
        "This console has no folder of its own (PANEL_DIR) to hold the copy".to_owned()
    })?;
    let free = (1..)
        .map(|n| folder.path().join(format!("annotated-{n}.png")))
        .find(|path| std::fs::symlink_metadata(path).is_err())
        .expect("some number is free");
    Ok(free)
}

/// Works out, before the drawer paints, where Send would write the shown image's copy. Only an
/// image with something to send asks the disk: an image merely looked at costs no call per frame.
pub(crate) fn aim(window: &mut crate::Window) {
    if !super::super::origin::showing_image(window) {
        window.image.send.target = None;
        return;
    }
    // A Send on its way keeps the name its line showed: the first free name is no longer it once
    // the worker creates the copy, and the line painted must stay the line that will be typed.
    let shown = super::focused(window);
    let in_flight = window.image.send.in_flight();
    let kept = in_flight
        .filter(|r| Some(r.document) == shown.and_then(Slot::document))
        .map(|r| r.path.clone());
    if let Some(path) = kept {
        window.image.send.target = Some(Ok(path));
        return;
    }
    let target = shown
        .filter(|slot| slot.has_marks())
        .and_then(Slot::origin)
        .map(|origin| origin.pane)
        .map(|pane| copy_path(window, pane));
    window.image.send.target = target;
}

/// The slot of the console `pane`, where everything a Send for it has to say goes.
fn slot(window: &mut crate::Window, pane: usize) -> Option<&mut Slot> {
    super::slot_of(&mut window.tabs, pane)
}

/// Says `said` in the slot of the console `pane`, called `console`. A console that closed took its
/// slot with it: what is left to say goes to standard error, never under another console's image.
fn say(window: &mut crate::Window, (pane, console): (usize, &str), said: String) {
    match slot(window, pane) {
        Some(slot) => slot.say(said),
        None => tell(console, &said),
    }
}

/// The line standard error is told about a Send for the console `console`: that stream is shared
/// by every console, so the line names which.
fn told(console: &str, said: &str) -> String {
    format!("Send to {console}: {said}")
}

/// Tells standard error `said` about a Send for the console `console`.
fn tell(console: &str, said: &str) {
    let line = told(console, said);
    #[cfg(test)]
    TOLD.with(|told| told.borrow_mut().push(line.clone()));
    eprintln!("{line}");
}

#[cfg(test)]
thread_local! {
    /// What this thread's Sends told standard error, for the tests to read.
    pub(super) static TOLD: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// D113: keep an already-owed Enter ahead of the next frame's console typing.
pub(crate) fn owes_enter(window: &crate::Window) -> bool {
    matches!(window.image.send.phase, Phase::Enter(..))
}

pub(crate) fn press_enter(window: &mut crate::Window, ctx: &egui::Context) {
    let frame = ctx.cumulative_frame_nr();
    if !matches!(&window.image.send.phase, Phase::Enter(_, at, _) if frame > *at) {
        return;
    }
    let Phase::Enter(enter, _, request) = std::mem::take(&mut window.image.send.phase) else {
        unreachable!("an Enter is owed");
    };
    if let Err(refusal) = delivery::press(window, enter) {
        let said = format!("The line was typed, but Enter was not pressed: {refusal}.");
        say(window, (request.pane, &request.console), said);
    }
}

/// Takes a Send from the click to the Enter, a step per frame, from the window's input pass.
pub(crate) fn poll(window: &mut crate::Window, ctx: &egui::Context) {
    press_enter(window, ctx);
    // A console that closed took its image, and the Cancel painted under it: its Send is taken
    // back as Cancel would, so its worker, if it has one, is parked and removes what it wrote, and
    // every other console's Send says what it waits for. That covers every way a console leaves.
    let gone = (window.image.send.in_flight())
        .filter(|r| !window.all_panes().any(|p| p.name == r.pane))
        .cloned();
    if let Some(request) = gone {
        let cancelled = window.image.send.cancel(|r| r.pane == request.pane);
        let left = if let Some(parked) = window.image.send.parked.as_mut() {
            parked.closed_told = true;
            String::new()
        } else {
            forsaken_folder(window, &request)
        };
        let extra = cancelled.filter(|said| said != CANCELLED && !said.contains(STILL_FINISHING));
        let extra = extra.map_or_else(String::new, |said| format!(" {said}"));
        tell(
            &request.console,
            &format!("{}; nothing was sent{left}.{extra}", Refusal::Closed),
        );
    }
    if let Some(parked) = &window.image.send.parked
        && let Some(written) = take(&parked.job)
    {
        let parked = window.image.send.parked.take().expect("a parked worker");
        forsake(window, &parked.request, parked.closed_told, written);
    }
    let phase = match std::mem::take(&mut window.image.send.phase) {
        Phase::Confirm(request, copy) if !answerable(window, &request) => {
            if let Some(path) = copy {
                forsake(window, &request, false, Ok(path));
            }
            Phase::Idle
        }
        Phase::Written(request, path) => deliver(window, request, &path, ctx),
        Phase::Asked(request) if drafted(window, &request) => confirm(window, request, None),
        Phase::Asked(request) => {
            let to = (request.pane, request.console.clone());
            start(window, request, ctx).unwrap_or_else(|why| {
                say(window, (to.0, &to.1), format!("{why}; nothing was sent."));
                Phase::Idle
            })
        }
        phase => phase,
    };
    // No timer while it runs: its worker wakes the window when it ends, and one that never ends
    // must not wake it for the life of the app.
    window.image.send.phase = match phase {
        Phase::Writing(request, job) => match take(&job) {
            None => {
                ctx.request_repaint_after(Duration::from_millis(50));
                Phase::Writing(request, job)
            }
            Some(Err(why)) => {
                failed(window, &request, why);
                Phase::Idle
            }
            Some(Ok(path)) if drafted(window, &request) => confirm(window, request, Some(path)),
            Some(Ok(path)) => deliver(window, request, &path, ctx),
        },
        phase => phase,
    };
}

fn drafted(window: &crate::Window, request: &Request) -> bool {
    request.answer.is_none()
        && window
            .all_panes()
            .any(|pane| pane.name == request.pane && pane.typed)
}

/// D113: ask again after writing only if nobody answered at the click.
fn confirm(window: &mut crate::Window, request: Request, copy: Option<PathBuf>) -> Phase {
    say(
        window,
        (request.pane, &request.console),
        format!(
            "Something is typed at {}'s prompt; it will be sent with this line.",
            request.console
        ),
    );
    Phase::Confirm(request, copy)
}

fn answerable(window: &crate::Window, request: &Request) -> bool {
    window
        .all_panes()
        .any(|p| p.name == request.pane && p.image.slot.document() == Some(request.document))
}

/// The worker could not write the copy; its error says whether what it had written of it stayed.
/// A closed console's worker is left to `forsake`: `poll` cancels its Send before reading it.
fn failed(window: &mut crate::Window, request: &Request, why: String) {
    debug_assert!(window.all_panes().any(|p| p.name == request.pane));
    let to = (request.pane, request.console.as_str());
    say(window, to, format!("{why}; nothing was sent."));
}

/// A cancelled Send's worker ended: the copy it wrote is removed, and with it the folder of a
/// console that closed meanwhile. What could not be removed is said, and so is a worker's failure
/// in its own words, which name a half-written copy it could not remove; a copy it did not write
/// is not touched, because a refused `create_new` means the file is somebody else's.
fn forsake(
    window: &mut crate::Window,
    request: &Request,
    closed_told: bool,
    written: Result<PathBuf, String>,
) {
    let closed = !window.all_panes().any(|p| p.name == request.pane);
    let stays = match written {
        Ok(path) => match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Some(format!(
                "The cancelled copy at {} could not be removed: {error}",
                path.display()
            )),
            _ => None,
        },
        // A console that closed under the worker is the likely cause, but a failure of its own is
        // said too; the close is named once, here or where `poll` cancelled the Send.
        Err(why) if closed && !closed_told => Some(format!(
            "{}, and the cancelled copy's worker reported: {why}",
            Refusal::Closed
        )),
        Err(why) => Some(format!("The cancelled copy's worker reported: {why}")),
    };
    let left = forsaken_folder(window, request);
    let said = match stays {
        Some(stays) => format!("{stays}{left}."),
        None if !left.is_empty() => format!("The cancelled copy is gone{left}."),
        None => {
            // Send is greyed out while the worker is parked, and the drawer says why: once it
            // ends cleanly, that is no longer true. Send is said ready only where it would be:
            // with marks to send and a folder to write their copy into.
            let ready = copy_path(window, request.pane).is_ok();
            let Some(slot) = slot(window, request.pane) else {
                return;
            };
            let ready = if ready && slot.has_marks() {
                "; Send is ready"
            } else {
                ""
            };
            settle(
                &mut slot.notice,
                &format!("{CANCELLED} Its copy is gone{ready}."),
            );
            return;
        }
    };
    // Standard error too: whoever ran the binary keeps it whatever becomes of the drawer.
    tell(&request.console, &said);
    // The worker it waited for has ended: a notice saying so is no longer true, and is not kept.
    if let Some(slot) = slot(window, request.pane) {
        settle(&mut slot.notice, CANCELLED);
        join(&mut slot.unshown, said);
    }
}

/// Puts `now` where `notice` said Send waits for a cancelled Send's worker, which has ended, and
/// keeps what was added after it: a refusal nobody has read yet.
fn settle(notice: &mut Option<String>, now: &str) {
    let Some(text) = notice.as_mut() else {
        return;
    };
    let waited = [
        format!("{CANCELLED} {STILL_FINISHING}."),
        format!("{STILL_FINISHING}; nothing was sent."),
    ];
    if let Some(waited) = waited.iter().find(|w| text.contains(w.as_str())) {
        *text = text.replacen(waited.as_str(), now, 1);
    }
}

/// Puts `said` after the drawer's `notice` rather than in its place: that one may be a refusal or
/// a failed Send nobody has read yet.
pub(super) fn join(notice: &mut Option<String>, said: String) {
    *notice = Some(match notice.take() {
        Some(current) => format!("{current} {said}"),
        None => said,
    });
}

/// A console that closed while its copy was being written could not remove its directory, which
/// held the copy; once the copy is gone, the directory goes as it would have. Answers what is left
/// to say about it, which only a closed console's Send has and so goes to standard error with the
/// rest of what that Send says; nothing when it was already gone. `PanelDir` told standard error
/// the directory remains when the console closed, so a removal here is told there too: otherwise
/// that line stays the last word on a directory that is gone.
fn forsaken_folder(window: &crate::Window, request: &Request) -> String {
    let Some(folder) = request.path.parent() else {
        return String::new();
    };
    if window.all_panes().any(|p| p.name == request.pane) {
        return String::new();
    }
    match std::fs::remove_dir(folder) {
        Ok(()) => format!(", and its folder {} was removed", folder.display()),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => format!(
            ", and its folder {} could not be removed: {error}",
            folder.display()
        ),
        Err(_) => String::new(),
    }
}

/// Every refusal that can be known before the copy is written, then the worker that writes it.
fn start(
    window: &mut crate::Window,
    request: Request,
    ctx: &egui::Context,
) -> Result<Phase, String> {
    // First: the parked worker may yet create the very file this Send's line names.
    if window.image.send.parked.is_some() {
        return Err(STILL_FINISHING.into());
    }
    let opened = |window: &mut crate::Window| slot(window, request.pane).and_then(|s| s.document());
    if opened(window) != Some(request.document) {
        return Err(REPLACED.into());
    }
    if copy_path(window, request.pane)? != request.path {
        return Err("The copy's name changed since the line was shown".into());
    }
    delivery::check(window, request.pane, &request.line).map_err(|r| r.to_string())?;
    let (original, marks) = slot(window, request.pane)
        .and_then(|slot| slot.to_flatten())
        .ok_or("The image is not loaded")?;
    let job = flatten::start(original, marks, request.path.clone(), ctx.clone())?;
    Ok(Phase::Writing(request, job))
}

/// The copy is written and closed: the line goes to the image's own console in this frame, or the
/// copy is removed and nothing is written anywhere. Answers what is left: the Enter, or nothing.
fn deliver(
    window: &mut crate::Window,
    request: Request,
    path: &Path,
    ctx: &egui::Context,
) -> Phase {
    // A Send whose console closed is cancelled by `poll` before its worker is read.
    debug_assert!(window.all_panes().any(|p| p.name == request.pane));
    let opened = slot(window, request.pane).and_then(|s| s.document());
    let sent = if opened == Some(request.document) {
        let send = delivery::Send {
            text: &request.line,
            submit: request.answer != Some(Answer::WithoutEnter),
        };
        delivery::send(window, request.pane, send).map_err(|r| r.to_string())
    } else {
        Err(REPLACED.to_owned())
    };
    match sent {
        Ok(enter) => {
            if let Some(console) = super::console_of(&mut window.tabs, request.pane) {
                console.annotated.record(path.to_owned());
                console.slot.sent();
            }
            ctx.request_repaint();
            enter.map_or(Phase::Idle, |enter| {
                Phase::Enter(enter, ctx.cumulative_frame_nr(), request)
            })
        }
        Err(why) => {
            let removed = match std::fs::remove_file(path) {
                Ok(()) => "the copy was removed".to_owned(),
                Err(error) => format!(
                    "the copy at {} could not be removed: {error}",
                    path.display()
                ),
            };
            let said = format!("{why}; nothing was sent and {removed}.");
            say(window, (request.pane, &request.console), said);
            Phase::Idle
        }
    }
}

/// What Cancel says it did.
const CANCELLED: &str = "Send cancelled; nothing was sent.";

/// Why Send waits while a cancelled Send's worker is still writing its copy.
const STILL_FINISHING: &str = "Still finishing the previous copy";

/// Why a Send whose image is no longer in its console's slot goes nowhere.
const REPLACED: &str = "The image this was drawn on is no longer the one in its console";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_copy_and_the_notes_and_leaves_out_the_empty_ones() {
        let copy = Path::new("/run/user/1000/ivy-1a2b3c4d/annotated-1.png");
        assert_eq!(
            compose(copy, &["button clipped", ""], ""),
            "Annotated screenshot: /run/user/1000/ivy-1a2b3c4d/annotated-1.png · 1: button clipped"
        );
        assert_eq!(
            compose(copy, &["", "a\tb\nc "], " the header "),
            "Annotated screenshot: /run/user/1000/ivy-1a2b3c4d/annotated-1.png · 2: a b c \
             · Note: the header"
        );
    }

    /// Every control character in a note becomes a space, the C1 ones included, so the paste
    /// writes exactly the line shown: it drops the C0 ones and would pass the C1 ones on.
    #[test]
    fn the_line_shown_is_the_line_the_paste_writes() {
        let copy = Path::new("/run/user/1000/ivy-1a2b3c4d/annotated-1.png");
        let line = compose(copy, &["\x1b[31mred\x7fx\u{85}y\u{9b}z"], "a\x03b");
        assert_eq!(
            line,
            "Annotated screenshot: /run/user/1000/ivy-1a2b3c4d/annotated-1.png · 1: [31mred x y z \
             · Note: a b"
        );
        let mut bytes = Vec::new();
        let mut held = None;
        crate::keyboard::paste_or_hold(&line, false, &mut bytes, &mut held);
        assert!(held.is_none());
        assert_eq!(String::from_utf8(bytes).unwrap(), line);
    }

    /// Cancel takes back only the Send clicked on its own image: one clicked on another opening
    /// stays on its way, and nothing is said.
    #[test]
    fn cancel_on_one_image_never_takes_back_anothers_send() {
        let mut sending = Sending::default();
        sending.ask(Request {
            document: 1,
            pane: 7,
            console: "A".into(),
            path: PathBuf::from("/a/annotated-1.png"),
            line: "x".into(),
            answer: None,
        });
        let mut notice = None;
        sending.answer(Some(Asked::Cancel(2)), &mut notice);
        assert!(sending.asked(), "another image's Cancel took the Send back");
        assert_eq!(notice, None);
        assert_eq!(sending.button(2), Button::Waiting("A"));
        sending.answer(Some(Asked::Cancel(1)), &mut notice);
        assert!(!sending.busy());
        assert_eq!(notice.as_deref(), Some(CANCELLED));
        // The Enter owed after a line is typed still holds every other image's Send, and its own
        // image's is past Cancel without waiting for itself.
        sending.phase = Phase::Enter(
            Enter(7),
            0,
            Request {
                document: 1,
                pane: 7,
                console: "A".into(),
                path: PathBuf::from("/a/annotated-1.png"),
                line: "x".into(),
                answer: None,
            },
        );
        assert_eq!(sending.button(2), Button::Waiting("A"));
        assert_eq!(sending.button(1), Button::Entering);
    }

    #[test]
    fn an_answer_from_another_image_leaves_the_draft_question_unanswered() {
        let mut sending = Sending::default();
        sending.question(Request {
            document: 1,
            pane: 7,
            console: "A".into(),
            path: std::env::temp_dir().join("ivy-unwritten-question.png"),
            line: "x".into(),
            answer: None,
        });
        let mut notice = Some("B's existing notice".to_owned());
        for answer in [Answer::Anyway, Answer::WithoutEnter] {
            sending.answer(Some(Asked::Answer(2, answer)), &mut notice);
            assert_eq!(sending.button(1), Button::Confirm);
            assert_eq!(sending.button(2), Button::Waiting("A"));
            assert_eq!(notice.as_deref(), Some("B's existing notice"));
        }
        sending.answer(Some(Asked::Cancel(2)), &mut notice);
        assert_eq!(sending.button(1), Button::Confirm);
        assert_eq!(notice.as_deref(), Some("B's existing notice"));
        sending.answer(Some(Asked::Answer(1, Answer::WithoutEnter)), &mut notice);
        assert!(sending.asked());
        assert_eq!(
            sending.in_flight().unwrap().answer,
            Some(Answer::WithoutEnter)
        );
        assert_eq!(notice, None);
    }

    /// A console with no name of its own already goes by its number, which is not said twice.
    #[test]
    fn a_console_named_by_its_number_is_not_numbered_twice() {
        let source = |label: &str| crate::viewer::Source {
            pane: 2,
            label: label.into(),
            cwd: None,
        };
        assert_eq!(console_name(&source("Console 3")), "Console 3");
        assert_eq!(console_name(&source("claude")), "claude (Console 3)");
    }

    /// A console that closes takes the copies Send left in its directory, and then the directory
    /// goes as it always does; a file of its program's is left alone and keeps the directory.
    #[test]
    fn a_closing_console_takes_its_copies_and_then_its_folder() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let names: Vec<usize> = window.all_panes().map(|p| p.name).collect();
        let mut folders = Vec::new();
        for name in &names {
            let dir = panel_contract::PanelDir::create().unwrap();
            let folder = dir.path().to_owned();
            let copy = folder.join("annotated-1.png");
            std::fs::write(&copy, b"png").unwrap();
            let pane = window.all_panes_mut().find(|p| p.name == *name).unwrap();
            pane.image.annotated.record(copy);
            pane.panel = Some(dir);
            folders.push(folder);
        }
        let theirs = folders[1].join("notes.txt");
        std::fs::write(&theirs, b"mine").unwrap();
        window.close_a_pane_called(names[0], &ctx).unwrap();
        assert!(!folders[0].exists(), "the closed console's folder stayed");
        drop(window);
        assert!(!folders[1].join("annotated-1.png").exists());
        assert_eq!(std::fs::read(&theirs).unwrap(), b"mine");
        let _ = std::fs::remove_dir_all(&folders[1]);
    }

    /// The two other ways a console leaves: its tab closed, and a retirement that detaches it.
    /// Both take its copies and then its folder.
    #[test]
    fn a_closed_tab_or_a_retired_console_takes_its_copies_and_its_folder() {
        let mut window =
            crate::tests::window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let mut folders = Vec::new();
        for name in [
            window.tabs[0].panes()[1].name,
            window.tabs[1].panes()[0].name,
        ] {
            let dir = panel_contract::PanelDir::create().unwrap();
            let folder = dir.path().to_owned();
            let copy = folder.join("annotated-1.png");
            std::fs::write(&copy, b"png").unwrap();
            let pane = window.all_panes_mut().find(|p| p.name == name).unwrap();
            pane.image.annotated.record(copy);
            pane.panel = Some(dir);
            folders.push((name, folder));
        }
        window.active = 1;
        window.close_the_active_tab(&ctx);
        assert_eq!(window.tabs.len(), 1);
        assert!(
            !folders[1].1.exists(),
            "the closed tab's copy or folder stayed"
        );
        let detached = window.detach_panes_called(&[folders[0].0], &ctx).unwrap();
        drop(detached);
        assert!(
            !folders[0].1.exists(),
            "the retired console's copy or folder stayed"
        );
    }
}
