use super::*;
use std::time::{Duration, Instant};

fn pending() -> (Process, Sender<term_core::Result<Spawned>>) {
    let (sender, result) = channel();
    let (output_open, _) = channel();
    (
        Process {
            live: None,
            terminal: term_core::Termination::Pending,
            cost: Duration::ZERO,
            context: None,
            pending: Some(Pending {
                result,
                _output_open: output_open,
                reads: Default::default(),
            }),
            input: vec![],
            input_since: None,
            refused: false,
            refusals: 0,
            size: Size::default(),
            failure: None,
            dismissed: false,
        },
        sender,
    )
}

#[test]
fn pending_input_and_latest_geometry_reach_the_real_program_in_order() {
    let (mut process, complete) = pending();
    process.write(b"first ").unwrap();
    process.write(b"second\n").unwrap();
    process.resize(Size { rows: 31, cols: 91 }).unwrap();
    let child = Pty::spawn(
        "/bin/sh",
        &[
            "-c",
            "read -r line; printf 'RECEIVED:%s:' \"$line\"; stty size",
        ],
        Size::default(),
    )
    .unwrap();
    complete.send(Ok(child)).unwrap();
    let (output, _) = process.poll().expect("ready launch");
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut text = Vec::new();
    while Instant::now() < deadline {
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(20)) {
            text.extend(bytes);
        }
        if String::from_utf8_lossy(&text).contains("RECEIVED:first second:31 91") {
            return;
        }
    }
    panic!(
        "queued input/resize not delivered: {}",
        String::from_utf8_lossy(&text)
    );
}

#[test]
fn closing_a_pending_launch_drops_the_late_child_without_resurrecting_the_console() {
    let (process, complete) = pending();
    drop(process);
    let (pty, output) = Pty::spawn(
        "/bin/sh",
        &["-c", "printf '%s\\n' $$; exec sleep 30"],
        Size::default(),
    )
    .unwrap();
    let bytes = output.recv_timeout(Duration::from_secs(3)).unwrap();
    let pid: i32 = String::from_utf8_lossy(&bytes).trim().parse().unwrap();
    drop(complete.send(Ok((pty, output))));
    let deadline = Instant::now() + Duration::from_secs(3);
    while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "cancelled child's process survived"
    );
}

#[test]
fn many_stalled_launches_accept_input_without_waiting_for_the_workers() {
    let mut panes: Vec<_> = (0..8).map(|_| pending()).collect();
    for _ in 0..3 {
        for (process, _) in &mut panes {
            assert!(process.poll().is_none());
            process.write(b"input").unwrap();
            assert!(process.keeps_open());
        }
    }
    for (process, complete) in &mut panes {
        complete.send(Err("fixture launch failure".into())).unwrap();
        assert!(process.poll().is_none());
        assert_eq!(
            process.notice(),
            Some("This console's program could not be started.")
        );
        assert!(process.keeps_open());
    }
}

#[test]
fn a_real_signal_retains_output_and_has_a_dismissible_truthful_notice() {
    let mut pane = crate::Pane::with_panel(
        &Default::default(),
        "/bin/sh",
        &[
            "-c".into(),
            "printf 'retained-output'; kill -KILL $$".into(),
        ],
        None,
        0,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while pane.pty.termination() == term_core::Termination::Pending && Instant::now() < deadline {
        pane.pump(Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    pane.pump(Default::default());
    assert_eq!(pane.pty.termination(), term_core::Termination::Signal);
    assert_eq!(
        pane.pty.notice(),
        Some("This console was terminated by a signal.")
    );
    assert!(!pane.is_finished(false));
    // The backend's OOM evidence uses this same notice path; signal alone above never earns it.
    pane.pty.terminal = term_core::Termination::Oom;
    assert_eq!(
        pane.pty.notice(),
        Some("This console was stopped because of memory pressure.")
    );
    assert_eq!(
        crate::painter::notices::what_the_silence_is(&pane),
        Some(crate::painter::notices::Silence::Termination)
    );
    let before = format!("{:?}", pane.screen.view());
    let text: String = pane
        .screen
        .view()
        .cells()
        .iter()
        .map(|cell| cell.c)
        .collect();
    assert!(text.contains("retained-output"), "{text}");
    pane.pty.dismissed = true;
    assert!(pane.pty.notice().is_none());
    assert_eq!(before, format!("{:?}", pane.screen.view()));
    assert!(!pane.is_finished(false));
}

#[test]
fn pending_input_backpressure_refuses_a_retry_and_keeps_one_large_paste_whole() {
    let (mut process, _complete) = pending();
    let paste = vec![b'x'; 300 * 1024];
    process.write(&paste).unwrap();
    assert_eq!(process.backlog().unwrap().bytes, paste.len());
    assert!(process.write(b"retry").is_err());
    let backlog = process.backlog().unwrap();
    assert!(backlog.refused);
    assert_eq!(backlog.refusals, 1);
    assert_eq!(process.input, paste);
}

#[test]
fn the_real_window_accepts_a_new_tab_while_existing_launches_are_stalled() {
    use eframe::egui;
    let ctx = egui::Context::default();
    let mut window =
        crate::Window::with_tabs(&ctx, 1, crate::OpeningLayout::DEFAULT, None).unwrap();
    let mut keep_pending = Vec::new();
    for pane in window.all_panes_mut() {
        let (process, sender) = pending();
        pane.pty = process;
        keep_pending.push(sender);
    }
    let mut frame = eframe::Frame::_new_kittest();
    let modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        events: vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key: egui::Key::T,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
        ..Default::default()
    };
    let output = ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame));
    output.drop_without_applying_deltas();
    assert_eq!(
        window.tabs.len(),
        2,
        "the window did not handle its shortcut while launches were pending"
    );
    assert!(window.tabs[0].panes().iter().all(|pane| pane.pty.pending()));
    window.pump_them_all();
    assert!(window.launch_cost.calls > 0);
    assert!(!keep_pending.is_empty());
}
