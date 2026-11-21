//! Send: a copy of the image with its marks written into the origin console's own folder, then one
//! line naming the copy, with the notes, typed into that console and submitted with Enter.
use super::super::delivery::{self, Enter, Refusal};
use super::flatten;
use super::tools::Asked;
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
    fn record(&mut self, path: PathBuf) {
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
pub(super) struct Request {
    /// Which opening of an image this Send belongs to, checked again before the copy starts and
    /// before the line is typed. The drawer refuses to open another image while marks are unsent,
    /// which they stay until the Send delivers; this id keeps the line off another image should
    /// that refusal ever give.
    pub(super) document: u64,
    pub(super) pane: usize,
    pub(super) path: PathBuf,
    pub(super) line: String,
    /// What the person answered when Send asked about a draft at the console's prompt, `None`
    /// while it has not asked. An answered Send is not asked again (#196): what is typed after the
    /// answer goes with the line, as the person chose.
    pub(super) answer: Option<Answer>,
}

/// The answers to the question about a draft at the prompt that send the line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Answer {
    /// «Send anyway»: the line, then Enter, as a Send with nothing typed.
    Anyway,
    /// «Type without Enter»: the line, and Enter is left to the person.
    WithoutEnter,
}

/// Where a Send is, from the click to the Enter; one at a time.
#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    /// Clicked; the copy starts on the next frame's input pass.
    Asked(Request),
    /// Something was typed at the console's prompt since its last Enter: nothing is typed until
    /// the person answers. Holds the copy's path when it was already written.
    Confirm(Request, Option<PathBuf>),
    Writing(Request, Job<PathBuf>),
    /// Answered over a copy already written: its line goes on the next frame's input pass.
    Written(Request, PathBuf),
    /// The Enter owed for a line written in the frame named here; it is pressed in a later one.
    Enter(Enter, u64),
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
    parked: Option<(Request, Job<PathBuf>)>,
    /// What a parked worker's end left to say, until a drawer lays it out: the image it was for
    /// may be closed by then, and the next one opened clears the notice it would have gone to.
    pub(super) unshown: Option<String>,
}

impl Sending {
    pub(super) fn busy(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// Whether `poll` has anything to do: a Send on its way, or a cancelled one's worker.
    pub(super) fn pending(&self) -> bool {
        self.busy() || self.parked.is_some()
    }

    /// Whether Cancel can still take the Send back: until its line is typed. Offered for the
    /// whole write rather than after a delay, because a slow disk is not a stuck one and no
    /// constant tells them apart; the person does.
    pub(super) fn cancellable(&self) -> bool {
        self.in_flight().is_some()
    }

    /// Whether Send is waiting for the person to answer about a draft at the prompt.
    pub(super) fn confirming(&self) -> bool {
        matches!(self.phase, Phase::Confirm(..))
    }

    /// What the Send button shows.
    pub(super) fn button(&self) -> Button {
        match self.phase {
            Phase::Confirm(..) => Button::Confirm,
            Phase::Idle if self.parked.is_some() => Button::Parked,
            Phase::Idle => Button::Ready,
            _ => Button::Sending,
        }
    }

    /// Takes up what a click in the notes asked, saying in `notice` what became of it.
    pub(super) fn answer(&mut self, asked: Option<Asked>, notice: &mut Option<String>) {
        match asked {
            Some(Asked::Send(request)) => {
                self.ask(request);
                *notice = None;
            }
            Some(Asked::Answer(answer)) if self.confirming() => {
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
            Some(Asked::Cancel) => {
                if let Some(said) = self.cancel() {
                    *notice = Some(said);
                }
            }
            _ => {}
        }
    }

    /// Takes back the Send whose line is not typed yet: a copy already written is removed, a
    /// worker still writing one is parked. Answers what to say, or `None` with nothing to take.
    fn cancel(&mut self) -> Option<String> {
        match std::mem::take(&mut self.phase) {
            Phase::Asked(_) | Phase::Confirm(_, None) => Some(CANCELLED.into()),
            // Its console is still there: `poll` drops a question whose console closed, folder
            // and all, before a click can answer it.
            Phase::Confirm(_, Some(path)) | Phase::Written(_, path) => {
                Some(match std::fs::remove_file(&path) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => format!(
                        "{CANCELLED} The copy at {} could not be removed: {error}.",
                        path.display()
                    ),
                    _ => CANCELLED.into(),
                })
            }
            Phase::Writing(request, job) => {
                // `start` refuses while a worker is parked, so there is never a second one here.
                debug_assert!(self.parked.is_none(), "a second worker parked");
                self.parked = Some((request, job));
                Some(format!("{CANCELLED} {STILL_FINISHING}."))
            }
            phase => {
                self.phase = phase;
                None
            }
        }
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
            | Phase::Confirm(request, _)
            | Phase::Writing(request, _)
            | Phase::Written(request, _) => Some(request),
            _ => None,
        }
    }
}

impl Drop for Sending {
    /// A question can wait for as long as nobody answers it, so the copy it holds is the app's to
    /// remove when the window goes; it was never recorded by a console's `Written`.
    fn drop(&mut self) {
        if let Phase::Confirm(_, Some(path)) | Phase::Written(_, path) = &self.phase
            && let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("the annotated copy {} remains ({error})", path.display());
        }
    }
}

/// What the Send button shows: ready, a Send on its way, greyed out for a cancelled Send's
/// worker, or the question about a draft at the prompt with its three answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Button {
    Ready,
    Sending,
    Parked,
    Confirm,
}

#[cfg(test)]
impl Sending {
    /// Puts the Send at the question about a draft, as a click over one does.
    pub(super) fn question(&mut self, request: Request) {
        self.phase = Phase::Confirm(request, None);
    }

    /// Whether a cancelled Send's worker is still writing its copy.
    pub(super) fn parked(&self) -> bool {
        self.parked.is_some()
    }

    /// Whether the Send has yet to write its line and is not waiting for the person: asked,
    /// writing its copy, or answered over a copy already written.
    pub(super) fn before_the_line(&self) -> bool {
        matches!(
            self.phase,
            Phase::Asked(_) | Phase::Writing(..) | Phase::Written(..)
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
    // A Send on its way keeps the name its line showed: the first free name is no longer it once
    // the worker creates the copy, and the line painted must stay the line that will be typed.
    let shown = window.image.document();
    let in_flight = window.image.send.in_flight();
    let kept = in_flight
        .filter(|r| Some(r.document) == shown)
        .map(|r| r.path.clone());
    if let Some(path) = kept {
        window.image.send.target = Some(Ok(path));
        return;
    }
    let target = window
        .image
        .origin()
        .filter(|_| window.image.has_marks())
        .map(|origin| copy_path(window, origin.pane));
    window.image.send.target = target;
}

/// Takes a Send from the click to the Enter, a step per frame, from the window's input pass.
pub(crate) fn poll(window: &mut crate::Window, ctx: &egui::Context) {
    // No timer while it runs: its worker wakes the window when it ends, and one that never ends
    // must not wake it for the life of the app.
    if let Some((_, job)) = &window.image.send.parked
        && let Some(written) = take(job)
    {
        let (request, _) = window.image.send.parked.take().expect("a parked worker");
        forsake(window, &request, written);
    }
    let frame = ctx.cumulative_frame_nr();
    let phase = match std::mem::take(&mut window.image.send.phase) {
        Phase::Enter(enter, at) if frame > at => {
            if let Err(refusal) = delivery::press(window, enter) {
                window.image.say(format!(
                    "The line was typed, but Enter was not pressed: {refusal}."
                ));
            }
            Phase::Idle
        }
        Phase::Confirm(request, copy) if !answerable(window, &request) => {
            abandon(window, &request, copy);
            Phase::Idle
        }
        Phase::Written(request, path) => deliver(window, request, &path, ctx),
        Phase::Asked(request) if drafted(window, &request) => confirm(window, request, None),
        Phase::Asked(request) => start(window, request, ctx).unwrap_or_else(|why| {
            window.image.say(format!("{why}; nothing was sent."));
            Phase::Idle
        }),
        phase => phase,
    };
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

/// Whether the person typed at the request's console since its last Enter and has not been asked
/// about it yet: the line would be joined to their draft and submitted with it (#196).
fn drafted(window: &crate::Window, request: &Request) -> bool {
    request.answer.is_none()
        && window
            .all_panes()
            .any(|pane| pane.name == request.pane && pane.typed)
}

/// Holds the Send until the person answers, saying why in the drawer. Asked at the click and,
/// when not answered yet, once the copy is written, just before the line would be typed: what was
/// typed while the copy was being written would be joined to it as well.
fn confirm(window: &mut crate::Window, request: Request, copy: Option<PathBuf>) -> Phase {
    let console = window
        .image
        .origin()
        .map_or_else(|| "the console".to_owned(), |o| o.label.clone());
    window.image.say(format!(
        "Something is typed at {console}'s prompt; it will be sent with this line."
    ));
    Phase::Confirm(request, copy)
}

/// Whether the question can still be answered: its image is the one shown and its console open.
fn answerable(window: &crate::Window, request: &Request) -> bool {
    window.image.document() == Some(request.document)
        && window.all_panes().any(|pane| pane.name == request.pane)
}

/// Drops a question nobody can answer any more. A console that closed is said where its image is
/// still shown; the copy goes as a cancelled one's does, with the folder of a closed console.
fn abandon(window: &mut crate::Window, request: &Request, copy: Option<PathBuf>) {
    if window.image.document() == Some(request.document) {
        window
            .image
            .say(format!("{}; nothing was sent.", Refusal::Closed));
    }
    if let Some(path) = copy {
        forsake(window, request, Ok(path));
    }
}

/// The worker could not write the copy; its error says whether what it had written of it stayed.
fn failed(window: &mut crate::Window, request: &Request, why: String) {
    // A console that closed under the worker is the likely cause, but not a certain one: both are
    // said, so a full disk is not hidden behind a close.
    let why = if window.all_panes().any(|p| p.name == request.pane) {
        why
    } else {
        format!("{}, and the copy's worker reported: {why}", Refusal::Closed)
    };
    let left = forsaken_folder(window, request);
    window.image.say(format!("{why}; nothing was sent{left}."));
}

/// A cancelled Send's worker ended: the copy it wrote is removed, and with it the folder of a
/// console that closed meanwhile. What could not be removed is said, and so is a worker's failure
/// in its own words, which name a half-written copy it could not remove; a copy it did not write
/// is not touched, because a refused `create_new` means the file is somebody else's.
fn forsake(window: &mut crate::Window, request: &Request, written: Result<PathBuf, String>) {
    let stays = match written {
        Ok(path) => match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Some(format!(
                "The cancelled copy at {} could not be removed: {error}",
                path.display()
            )),
            _ => None,
        },
        // As `failed` says it: a console that closed under the worker is the likely cause.
        Err(why) if !window.all_panes().any(|p| p.name == request.pane) => Some(format!(
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
            let ready = window.image.has_marks()
                && window
                    .image
                    .origin()
                    .is_some_and(|o| copy_path(window, o.pane).is_ok());
            let notice = &mut window.image.notice;
            if notice
                .as_deref()
                .is_some_and(|n| n.contains(STILL_FINISHING))
            {
                let ready = if ready { "; Send is ready" } else { "" };
                *notice = Some(format!("{CANCELLED} Its copy is gone{ready}."));
            }
            return;
        }
    };
    // Standard error too: whoever ran the binary keeps it whatever becomes of the drawer.
    eprintln!("{said}");
    // The worker it waited for has ended: a notice saying so is no longer true, and is not kept.
    let notice = &mut window.image.notice;
    if notice
        .as_deref()
        .is_some_and(|n| n.contains(STILL_FINISHING))
    {
        *notice = Some(CANCELLED.into());
    }
    window.image.send.unshown = Some(said);
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
/// to say about it in the drawer, which is nothing when it went or was already gone. `PanelDir`
/// told standard error the directory remains when the console closed, so a removal here is told
/// there too: otherwise that line stays the last word on a directory that is gone.
fn forsaken_folder(window: &crate::Window, request: &Request) -> String {
    let Some(folder) = request.path.parent() else {
        return String::new();
    };
    if window.all_panes().any(|p| p.name == request.pane) {
        return String::new();
    }
    match std::fs::remove_dir(folder) {
        Ok(()) => {
            eprintln!(
                "the panel directory {} was removed once its annotated copy was gone",
                folder.display()
            );
            String::new()
        }
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
    if window.image.document() != Some(request.document) {
        return Err(REPLACED.into());
    }
    if copy_path(window, request.pane)? != request.path {
        return Err("The copy's name changed since the line was shown".into());
    }
    delivery::check(window, request.pane, &request.line).map_err(|r| r.to_string())?;
    let (original, marks) = window.image.to_flatten().ok_or("The image is not loaded")?;
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
    let sent = if window.image.document() == Some(request.document) {
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
            if let Some(pane) = window.all_panes_mut().find(|p| p.name == request.pane) {
                pane.annotated.record(path.to_owned());
            }
            window.image.sent();
            ctx.request_repaint();
            enter.map_or(Phase::Idle, |enter| {
                Phase::Enter(enter, ctx.cumulative_frame_nr())
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
            let left = forsaken_folder(window, &request);
            window
                .image
                .say(format!("{why}; nothing was sent and {removed}{left}."));
            Phase::Idle
        }
    }
}

/// What Cancel says it did.
const CANCELLED: &str = "Send cancelled; nothing was sent.";

/// Why Send waits while a cancelled Send's worker is still writing its copy.
const STILL_FINISHING: &str = "Still finishing the previous copy";

/// Why a Send whose image is no longer the one shown goes nowhere.
const REPLACED: &str = "The image this was drawn on is no longer the one shown";

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
            pane.annotated.record(copy);
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
            pane.annotated.record(copy);
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
