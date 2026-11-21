//! Deliver text only to the console it belongs to: the Git review's draft to the console that
//! opened the review, and Send's line, with its Enter, to the console an image came from.
use eframe::egui;

pub(super) fn deliver(window: &mut crate::Window, ctx: &egui::Context) {
    if window.viewer.pending_send.is_none() && !window.image.send_pending() {
        return;
    }
    let started = std::time::Instant::now();
    deliver_inner(window, ctx);
    super::image::send::poll(window, ctx);
    window.viewer.record(started.elapsed());
    window.timed_separately += window.viewer.take_cost();
}

fn deliver_inner(window: &mut crate::Window, ctx: &egui::Context) {
    let Some(text) = window.viewer.pending_send.take() else {
        return;
    };
    let Some(origin) = window.viewer.origin.as_ref() else {
        window.viewer.message =
            "The origin console is unavailable; comments were not inserted.".into();
        return;
    };
    let sent = send(
        window,
        origin.pane,
        Send {
            text: &text,
            submit: false,
        },
    );
    window.viewer.message = match sent {
        Ok(_) => {
            window.viewer.review.clear();
            ctx.request_repaint();
            "Comments inserted in the origin console; review and submit them there.".into()
        }
        Err(refusal) => format!("{refusal}; comments were not inserted."),
    };
}

/// Text pasted into a console. `submit` is whether Enter follows it, which only Send asks for: the
/// review leaves its comments in the console for the person to submit.
pub(super) struct Send<'a> {
    pub(super) text: &'a str,
    pub(super) submit: bool,
}

/// An Enter owed to the console named here, whose paste was written this frame. It is pressed on
/// the next frame in a write of its own, as a person's Enter arrives: people driving Claude Code
/// from tmux report a paste and an Enter arriving together read as part of the paste (D113).
pub(super) struct Enter(pub(super) usize);

/// Why a paste was not written.
#[derive(Debug)]
pub(super) enum Refusal {
    Closed,
    Held,
    Deaf,
    Multiline,
    TooLong,
    Failed(String),
}

/// One wording for the review and for Send alike, each adding what did not happen.
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("The origin console has closed"),
            Self::Held => f.write_str("The origin console has a paste waiting for an answer"),
            Self::Deaf => f.write_str("The origin console cannot receive input"),
            Self::Multiline => f.write_str("The origin console does not take a multiline paste"),
            Self::TooLong => {
                f.write_str("The text is longer than the origin console takes in one paste")
            }
            Self::Failed(error) => write!(f, "Writing to the origin console failed: {error}"),
        }
    }
}

/// The bytes `text` takes as a paste into `pane` in the paste mode it has now, or why it cannot go.
fn compose(pane: &crate::Pane, text: &str) -> Result<Vec<u8>, Refusal> {
    if pane.held.is_some() {
        return Err(Refusal::Held);
    }
    if pane.pty.backlog().is_some_and(|backlog| backlog.gave_up) {
        return Err(Refusal::Deaf);
    }
    let text = text.trim_end_matches(['\r', '\n']);
    let bracketed = pane.screen.bracketed_paste();
    if !bracketed && text.contains(['\r', '\n']) {
        return Err(Refusal::Multiline);
    }
    let mut bytes = Vec::new();
    let mut held = None;
    crate::keyboard::paste_or_hold(text, bracketed, &mut bytes, &mut held);
    if held.is_some() {
        return Err(Refusal::TooLong);
    }
    Ok(bytes)
}

/// Whether `text` would be written into the console `name` now, writing nothing.
pub(super) fn check(window: &crate::Window, name: usize, text: &str) -> Result<(), Refusal> {
    let pane = window
        .all_panes()
        .find(|pane| pane.name == name)
        .ok_or(Refusal::Closed)?;
    compose(pane, text).map(drop)
}

/// Writes `send.text` into the console `name` as a paste and gives it the keys, or says why not.
pub(super) fn send(
    window: &mut crate::Window,
    name: usize,
    send: Send,
) -> Result<Option<Enter>, Refusal> {
    let tab_index = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|pane| pane.name == name))
        .ok_or(Refusal::Closed)?;
    let tab = &mut window.tabs[tab_index];
    let pane = tab.pane_called(name).expect("pane found in this tab");
    let bytes = compose(pane, send.text)?;
    pane.pty
        .write(&bytes)
        .map_err(|e| Refusal::Failed(e.to_string()))?;
    crate::resume::wrote(pane, &bytes);
    pane.screen.scroll_by(term_core::Scroll::ToBottom);
    pane.hand_it_back_to_the_person();
    tab.focus = name;
    window.active = tab_index;
    Ok(send.submit.then_some(Enter(name)))
}

/// Presses the Enter owed to a console, if it is still there.
pub(super) fn press(window: &mut crate::Window, Enter(name): Enter) -> Result<(), Refusal> {
    let pane = window
        .all_panes_mut()
        .find(|pane| pane.name == name)
        .ok_or(Refusal::Closed)?;
    pane.pty
        .write(b"\r")
        .map_err(|e| Refusal::Failed(e.to_string()))?;
    crate::resume::wrote(pane, b"\r");
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::viewer::Source;
    use std::time::Duration;
    use term_core::received::Received;

    #[test]
    fn delivers_once_to_pinned_pane_and_refuses_closed_or_unwrapped_multiline() {
        let first = Received::new("review-first");
        let second = Received::new("review-second");
        let first_script = first.script("stty raw -echo");
        let second_script = second.script("stty raw -echo");
        let mut window = crate::tests::window_of_tabs([
            [&first_script, "sleep 30"],
            [&second_script, "sleep 30"],
        ]);
        first.wait_until_ready(Duration::from_secs(5));
        second.wait_until_ready(Duration::from_secs(5));
        let name = window.tabs[0].panes()[0].name;
        window.tabs[0]
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b[?2004h");
        window.active = 1;
        window.viewer.origin = Some(Source {
            pane: name,
            label: "origin".into(),
            cwd: None,
        });
        window.viewer.pending_send = Some("one\ntwo\n".into());
        let ctx = egui::Context::default();
        deliver(&mut window, &ctx);
        assert_eq!(
            first.wait_for(19, Duration::from_secs(5)),
            b"\x1b[200~one\ntwo\x1b[201~"
        );
        assert!(second.so_far().is_empty());
        assert_eq!(window.active, 0);
        // The review's draft sits at the prompt with no Enter: Send would be joined to it (#196).
        assert!(window.tabs[0].pane_called(name).unwrap().typed);
        press(&mut window, Enter(name)).unwrap();
        assert!(
            !window.tabs[0].pane_called(name).unwrap().typed,
            "an Enter left the draft recorded"
        );
        assert_eq!(
            first.wait_for(20, Duration::from_secs(5)),
            b"\x1b[200~one\ntwo\x1b[201~\r"
        );
        deliver(&mut window, &ctx);
        assert_eq!(first.so_far(), b"\x1b[200~one\ntwo\x1b[201~\r");
        window.tabs[0]
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b[?2004l");
        window.viewer.pending_send = Some("another\nline".into());
        deliver(&mut window, &ctx);
        assert!(window.viewer.message.contains("multiline"));
        assert_eq!(first.so_far(), b"\x1b[200~one\ntwo\x1b[201~\r");
        window.viewer.origin.as_mut().unwrap().pane = usize::MAX;
        window.viewer.pending_send = Some("never".into());
        deliver(&mut window, &ctx);
        assert!(window.viewer.message.contains("closed"));
        assert!(second.so_far().is_empty());
    }
}

#[cfg(all(test, windows))]
mod tests {
    crate::windows_unix_omission!(
        delivers_once_to_pinned_pane_and_refuses_closed_or_unwrapped_multiline,
        "unix-only: uses POSIX shell fixtures and Unix PTY byte-delivery assertions"
    );
}
