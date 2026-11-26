#![cfg(test)]

use super::*;
use std::time::{Duration, Instant};

fn pending() -> (Process, Sender<term_core::Result<Ready>>) {
    let (sender, result) = channel();
    let (output_open, _) = channel();
    (
        Process {
            live: None,
            #[cfg(windows)]
            invocation: None,
            terminal: term_core::Termination::Pending,
            cost: Duration::ZERO,
            context: None,
            pending: Some(Pending {
                result,
                _output_open: output_open,
                reads: Default::default(),
            }),
            input: vec![],
            startup: 0,
            input_since: None,
            refused: false,
            refusals: 0,
            failure: None,
            startup_failure: None,
            delivery_failed: false,
            dismissed: false,
        },
        sender,
    )
}

crate::windows_unix_omission!(
    pending_input_and_latest_geometry_reach_the_real_program_in_order,
    "unix-only: runs /bin/sh and stty to verify queued input and PTY geometry"
);
#[cfg(unix)]
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
    complete.send(Ok(prepare(child))).unwrap();
    let (output, _, _) = process.poll().expect("ready launch");
    process.resize(Size { rows: 31, cols: 91 }).unwrap();
    let input = std::mem::take(&mut process.input);
    process.live.as_mut().unwrap().write(&input).unwrap();
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

// Probes the child with a Unix signal; the Windows suite is #162.
crate::windows_unix_omission!(
    closing_a_pending_launch_drops_the_late_child_without_resurrecting_the_console,
    "unix-only: requires Unix process signals"
);
#[cfg(unix)]
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
    drop(complete.send(Ok(prepare((pty, output)))));
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

crate::windows_unix_omission!(
    a_real_signal_retains_output_and_has_a_dismissible_truthful_notice,
    "unix-only: sends a Unix signal to a POSIX shell and checks signal termination"
);
#[cfg(unix)]
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
    while Instant::now() < deadline {
        pane.pump(Default::default());
        let text: String = pane
            .screen
            .view()
            .cells()
            .iter()
            .map(|cell| cell.c)
            .collect();
        // Process exit and delivery through the reader's bridge are independent observations.
        if pane.pty.termination() == term_core::Termination::Signal
            && text.contains("retained-output")
        {
            break;
        }
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
    pane.pty.delivery_failed = true;
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
        let (release, held) = channel();
        let waker = ctx.clone();
        let (process, output) = start_worker(
            std::sync::Arc::new(move || waker.request_repaint()),
            Default::default(),
            move || {
                held.recv_timeout(Duration::from_secs(2))?;
                Pty::spawn(
                    crate::shell_integration::fallback_shell(),
                    &[],
                    Size::default(),
                )
            },
        )
        .unwrap();
        pane.pty = process;
        pane.pty_output = crate::pty_output::PtyOutput::new(output, || {});
        keep_pending.push(release);
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

#[test]
fn a_failed_launch_stays_idle_after_its_notice_is_dismissed() {
    let ctx = eframe::egui::Context::default();
    let mut pane = crate::Pane::with_panel(
        &Default::default(),
        crate::shell_integration::fallback_shell(),
        &[],
        None,
        0,
    )
    .unwrap();
    let (mut process, done) = pending();
    process.context = Some(ctx.clone());
    done.send(Err("fixture launch failure".into())).unwrap();
    pane.pty = process;
    let (closed, output) = channel();
    drop(closed);
    pane.pty_output = crate::pty_output::PtyOutput::new(output, || {});
    for _ in 0..4 {
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
    }
    for dismissed in [false, true] {
        pane.pty.dismissed = dismissed;
        let output = ctx.run_ui(Default::default(), |_| refresh(&mut pane));
        let delay = output.viewport_output[&eframe::egui::ViewportId::ROOT].repaint_delay;
        output.drop_without_applying_deltas();
        assert_eq!(
            delay,
            Duration::MAX,
            "failed launch armed a repaint: {delay:?}"
        );
        assert!(pane.pty.keeps_open());
    }
}

crate::windows_unix_omission!(
    startup_prefix_is_interpreted_before_resize_even_with_the_bridge_parked,
    "unix-only: uses /bin/sh, stty and cat to test startup input ordering"
);
#[cfg(unix)]
#[test]
fn startup_prefix_is_interpreted_before_resize_even_with_the_bridge_parked() {
    for padding in [0, crate::PUMP_BUDGET + 7] {
        let original = crate::INITIAL_GRID;
        let narrow = Size {
            rows: original.rows,
            cols: original.cols / 2,
        };
        let mut prefix = vec![b' '; padding];
        prefix.extend(format!("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdef\x1b[1;{}H\x1b[10DX", original.cols).bytes());
        let mut expected = term_core::Screen::new(original);
        expected.advance(&prefix);
        expected.resize(narrow);
        let contents = |screen: &term_core::Screen| {
            screen
                .view()
                .cells()
                .iter()
                .map(|cell| cell.c)
                .collect::<Vec<_>>()
        };
        let mut wrong_grid = term_core::Screen::new(narrow);
        wrong_grid.advance(&prefix);
        assert_ne!(
            contents(&expected),
            contents(&wrong_grid),
            "fixture must distinguish the wrong grid"
        );
        let path =
            std::env::temp_dir().join(format!("ivy-startup-{}-{padding}", std::process::id()));
        let script = format!(
            "read -r line; printf '%s ' \"$line\" > '{}'; stty size >> '{}'; exec /bin/cat",
            path.display(),
            path.display()
        );
        let child = Pty::spawn("/bin/sh", &["-c", &script], original).unwrap();
        let (raw, output) = channel();
        raw.send(prefix.clone()).unwrap();
        let (woke, ready) = channel();
        let (mut process, held) = start_worker(
            std::sync::Arc::new(move || {
                let _ = woke.send(());
            }),
            Default::default(),
            move || Ok((child.0, output)),
        )
        .unwrap();
        if padding == 0 {
            process.write(b"first second\n").unwrap();
        }
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let mut pane = crate::Pane::with_panel(
            &Default::default(),
            crate::shell_integration::fallback_shell(),
            &[],
            None,
            0,
        )
        .unwrap();
        pane.pty = process;
        pane.pty_output = crate::pty_output::PtyOutput::new(held, || {});
        pane.measured = narrow;
        let (parked, release) = crate::pty_output::hold_next_bridge();
        pane.pump(Default::default());
        parked.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            pane.pty_output.is_alive(),
            "bridge was allowed to finish instead of being parked"
        );
        pane.resize();
        finish_input(&mut pane);
        if prefix.len() > crate::PUMP_BUDGET {
            assert_eq!(
                pane.received.len(),
                crate::PUMP_BUDGET,
                "startup bypassed normal frame budget"
            );
            assert_eq!(
                pane.screen.size(),
                original,
                "resized with startup bytes outstanding"
            );
            assert!(pane.pty.startup_pending());
            pane.pty.write(b"first second\n").unwrap();
            pane.pty.write(b"tail").unwrap();
            finish_input(&mut pane);
            assert_eq!(
                pane.pty.input, b"first second\ntail",
                "live input bypassed startup ordering"
            );
            assert!(
                !path.exists(),
                "pending input reached the program before reflow"
            );
        }
        for _ in 0..prefix.len().div_ceil(crate::PUMP_BUDGET) {
            if !pane.pty.startup_pending() {
                break;
            }
            pane.pump(Default::default());
        }
        assert!(
            !pane.pty.startup_pending(),
            "captured startup bytes never reached the grid"
        );
        pane.resize();
        finish_input(&mut pane);
        assert_eq!(contents(&pane.screen), contents(&expected));
        let expected_input = format!("first second {} {}\n", narrow.rows, narrow.cols);
        let deadline = Instant::now() + Duration::from_secs(3);
        while std::fs::read_to_string(&path).ok().as_deref() != Some(expected_input.as_str())
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("first second {} {}\n", narrow.rows, narrow.cols)
        );
        release.send(()).unwrap();
        drop(raw);
        drop(pane);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn startup_snapshot_has_a_finite_chunk_bound_and_preserves_the_remaining_stream() {
    let (pty, _output) = Pty::spawn(
        crate::shell_integration::fallback_shell(),
        &[],
        Size::default(),
    )
    .unwrap();
    let (source, output) = channel();
    for value in 0..9 {
        source.send(vec![value; 8192]).unwrap();
    }
    let ready = prepare((pty, output));
    assert_eq!(ready.startup.len(), 8 * 8192);
    assert_eq!(ready.spawned.1.try_recv().unwrap(), vec![8; 8192]);
}

#[test]
fn a_panicked_launch_worker_wakes_the_window_to_report_failure() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let (mut process, _output) = start_worker(
        Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        Default::default(),
        || panic!("fixture launch worker panic"),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while wakes.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert!(process.poll().is_none());
    assert!(!process.pending());
    assert_eq!(
        process.notice(),
        Some("This console's program could not be started.")
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the built target/debug/app and a real systemd user manager"]
fn normal_restored_fallback_and_takeover_paths_use_the_explicit_ivy_launcher() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = explicit_ivy_launcher();
    let root = std::env::temp_dir().join(format!("ivy-launch-paths-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let pidfile = root.join("pid");
    let program = root.join("fixture");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nprintf '%s' $$ > '{}'\nexec /bin/cat\n",
            pidfile.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let ctx = eframe::egui::Context::default();
    let parent = std::fs::read_to_string("/proc/self/cgroup").unwrap();
    let mut groups = std::collections::HashSet::new();
    let mut check = |pane: &mut crate::Pane, kind: &str| {
        assert!(pane.pty.pending(), "{kind} bypassed the launch worker");
        let deadline = Instant::now() + Duration::from_secs(4);
        while (!pidfile.exists() || pane.pty.pending()) && Instant::now() < deadline {
            refresh(pane);
            finish_input(pane);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(pane.pty.failure.is_none(), "{kind}: {:?}", pane.pty.failure);
        let pid: u32 = std::fs::read_to_string(&pidfile).unwrap().parse().unwrap();
        let group = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
        assert!(group.contains("/app.slice/ivy-console-"), "{kind}: {group}");
        assert_ne!(group, parent, "{kind} remained in the GUI scope");
        assert!(
            groups.insert(group),
            "{kind} reused another console's scope"
        );
        std::fs::remove_file(&pidfile).unwrap();
    };
    let mut normal =
        crate::Pane::with_panel(&ctx, program.to_str().unwrap(), &[], None, 0).unwrap();
    check(&mut normal, "normal");
    let saved = crate::session::interpret(
        &serde_json::json!({"v":2,"tabs":[{"root":{"leaf":{"panel_width":30,"folded":false}}}]})
            .to_string(),
    )
    .unwrap();
    let mut tabs = crate::Window::materialize(&ctx, &saved, program.to_str().unwrap()).unwrap();
    check(tabs[0].panes_mut().into_iter().next().unwrap(), "restored");
    let mut fallback =
        crate::Pane::with_panel(&ctx, "/definitely/missing/ivy-client", &[], None, 1).unwrap();
    fallback
        .pty
        .write(format!("printf '%s' $$ > '{}'\n", pidfile.display()).as_bytes())
        .unwrap();
    check(&mut fallback, "fallback");
    normal.restoration.failed = true;
    normal.restoration.fresh_shell_required = true;
    crate::resume::take_over(&mut normal).unwrap();
    normal
        .pty
        .write(format!("printf '%s' $$ > '{}'\n", pidfile.display()).as_bytes())
        .unwrap();
    check(&mut normal, "takeover");
    drop((normal, fallback, tabs));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
pub(crate) struct LauncherGuard;
#[cfg(target_os = "linux")]
impl Drop for LauncherGuard {
    fn drop(&mut self) {
        TEST_LAUNCHER.with(|path| *path.borrow_mut() = None);
    }
}
#[cfg(target_os = "linux")]
pub(crate) fn explicit_ivy_launcher() -> LauncherGuard {
    // A deliberately explicit product executable: never current_exe(), which is the test runner.
    let launcher = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/app")
        .canonicalize()
        .unwrap();
    assert_eq!(launcher.file_name().unwrap(), "app");
    TEST_LAUNCHER.with(|path| *path.borrow_mut() = Some(launcher));
    LauncherGuard
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the built target/debug/app and a real systemd user manager"]
fn a_real_scoped_exit_keeps_repainting_until_the_observer_finishes() {
    let _guard = explicit_ivy_launcher();
    let ctx = eframe::egui::Context::default();
    let mut pane = crate::Pane::with_panel(
        &ctx,
        "/bin/sh",
        &["-c".into(), "printf retained; exit 0".into()],
        None,
        0,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while (pane.pty.pending() || pane.pty_output.is_alive()) && Instant::now() < deadline {
        pane.pump(Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!pane.pty.pending());
    assert!(!pane.pty_output.is_alive());
    assert_eq!(
        pane.pty.termination(),
        term_core::Termination::Pending,
        "fixture bypassed scope observation"
    );
    assert!(!pane.is_finished(false));
    // Drain output/worker wakes; the observer is the only remaining reason to repaint.
    for _ in 0..4 {
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
    }
    let frame = ctx.run_ui(Default::default(), |_| refresh(&mut pane));
    let delay = frame.viewport_output[&eframe::egui::ViewportId::ROOT].repaint_delay;
    frame.drop_without_applying_deltas();
    assert!(
        delay <= Duration::from_millis(20),
        "no wake while observing: {delay:?}"
    );
    while pane.pty.termination() == term_core::Termination::Pending && Instant::now() < deadline {
        pane.pump(Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(pane.pty.termination(), term_core::Termination::Ordinary);
    assert!(
        pane.is_finished(false),
        "ordinary observed exit did not become closable"
    );
    let text: String = pane
        .screen
        .view()
        .cells()
        .iter()
        .map(|cell| cell.c)
        .collect();
    assert!(text.contains("retained"), "observer discarded output");
}

#[cfg(windows)]
#[test]
#[ignore = "unix-only: requires the built target/debug/app and a real systemd user manager"]
fn normal_restored_fallback_and_takeover_paths_use_the_explicit_ivy_launcher() {}

#[cfg(windows)]
#[test]
#[ignore = "unix-only: requires the built target/debug/app and a real systemd user manager"]
fn a_real_scoped_exit_keeps_repainting_until_the_observer_finishes() {}
