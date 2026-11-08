//! Deliver a reviewed draft only to the console that originated the viewer.
use eframe::egui;

pub(super) fn deliver(window: &mut crate::Window, ctx: &egui::Context) {
    if window.viewer.pending_send.is_none() {
        return;
    }
    let started = std::time::Instant::now();
    deliver_inner(window, ctx);
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
    let name = origin.pane;
    let Some(tab_index) = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|pane| pane.name == name))
    else {
        window.viewer.message = "The origin console was closed; comments were not inserted.".into();
        return;
    };
    let tab = &mut window.tabs[tab_index];
    let pane = tab.pane_called(name).expect("pane found in this tab");
    if pane.held.is_some() {
        window.viewer.message =
            "The origin console already has a pending paste; comments were not inserted.".into();
        return;
    }
    if pane.pty.backlog().is_some_and(|backlog| backlog.gave_up) {
        window.viewer.message =
            "The origin console cannot receive input; comments were not inserted.".into();
        return;
    }
    let text = text.trim_end_matches(['\r', '\n']);
    let bracketed = pane.screen.bracketed_paste();
    if !bracketed && text.contains(['\r', '\n']) {
        window.viewer.message =
            "The origin console does not support multiline paste; comments were not inserted."
                .into();
        return;
    }
    let mut bytes = Vec::new();
    crate::keyboard::paste_or_hold(text, bracketed, &mut bytes, &mut pane.held);
    if pane.held.is_some() {
        pane.held = None;
        window.viewer.message =
            "Comments exceed the safe paste limit; they were not inserted.".into();
        return;
    }
    if let Err(error) = pane.pty.write(&bytes) {
        window.viewer.message = format!("Comments were not inserted: {error}");
        return;
    }
    pane.screen.scroll_by(term_core::Scroll::ToBottom);
    pane.hand_it_back_to_the_person();
    tab.focus = name;
    window.active = tab_index;
    window.viewer.review.clear();
    window.viewer.message =
        "Comments inserted in the origin console; review and submit them there.".into();
    ctx.request_repaint();
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
        deliver(&mut window, &ctx);
        assert_eq!(first.so_far(), b"\x1b[200~one\ntwo\x1b[201~");
        window.tabs[0]
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b[?2004l");
        window.viewer.pending_send = Some("another\nline".into());
        deliver(&mut window, &ctx);
        assert!(window.viewer.message.contains("multiline"));
        assert_eq!(first.so_far(), b"\x1b[200~one\ntwo\x1b[201~");
        window.viewer.origin.as_mut().unwrap().pane = usize::MAX;
        window.viewer.pending_send = Some("never".into());
        deliver(&mut window, &ctx);
        assert!(window.viewer.message.contains("closed"));
        assert!(second.so_far().is_empty());
    }
}
