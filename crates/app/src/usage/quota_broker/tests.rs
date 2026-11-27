use super::*;
use std::{
    env, fs,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
use term_core::{Pty, Size};

const HELPER_MODE: &str = "IVY_QUOTA_TEST_MODE";

#[test]
fn native_helper() {
    let Ok(mode) = env::var(HELPER_MODE) else {
        return;
    };
    let stream = UnixStream::connect(env::var_os(quota::ENDPOINT_ENV).unwrap()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let admission = quota::read_line(&stream, MAX_FRAME);
    if mode == "foreign" {
        assert!(admission.is_err());
    } else {
        let admission: Admission = serde_json::from_slice(&admission.unwrap()).unwrap();
        match mode.as_str() {
            "oversized" => {
                let mut bytes = vec![b' '; 1025];
                bytes.push(b'\n');
                let _ = (&stream).write_all(&bytes);
            }
            "stall" => {
                // A connected peer cannot keep the worker after the frame deadline.
                let started = Instant::now();
                (&stream).write_all(b"{").unwrap();
                assert_eq!((&stream).read(&mut [0; 1]).unwrap(), 0);
                assert!(started.elapsed() < Duration::from_millis(800));
            }
            "revoked" => {
                println!("quota-fixture-admitted");
                io::stdin().read_line(&mut String::new()).unwrap();
                let frame = Frame {
                    v: 1,
                    sequence: admission.sequence,
                    outcome: Outcome::ProducerError,
                };
                let _ = quota::write_line(&stream, &frame, MAX_FRAME);
            }
            _ => {
                let frame = match mode.as_str() {
                    "good" | "expiry" => Frame {
                        v: 1,
                        sequence: admission.sequence,
                        outcome: Outcome::Observed {
                            five_hour: Some(Window {
                                used_percentage: Some(0.0),
                                resets_at: Some(
                                    unix_seconds() + if mode == "expiry" { 2 } else { 30 },
                                ),
                            }),
                            seven_day: None,
                        },
                    },
                    "steady" | "moved" => Frame {
                        v: 1,
                        sequence: admission.sequence,
                        outcome: Outcome::Observed {
                            five_hour: Some(Window {
                                used_percentage: Some(if mode == "steady" { 10.0 } else { 20.0 }),
                                resets_at: Some(4_000_000_000),
                            }),
                            seven_day: None,
                        },
                    },
                    "wrong-sequence" => Frame {
                        v: 1,
                        sequence: admission.sequence + 1,
                        outcome: Outcome::ProducerError,
                    },
                    _ => panic!("unknown fixture mode"),
                };
                quota::write_line(&stream, &frame, MAX_FRAME).unwrap();
            }
        }
    }
    println!("quota-fixture-result={mode}");
}

struct Launch {
    pty: Pty,
    output: mpsc::Receiver<Vec<u8>>,
}

impl Launch {
    fn new(endpoint: &Path) -> Self {
        let test_name = format!(
            "{}::native_helper",
            module_path!().split_once("::").unwrap().1
        );
        let executable = env::current_exe().unwrap();
        let script = "while IFS= read -r mode; do [ \"$mode\" = quit ] && break; IVY_QUOTA_TEST_MODE=\"$mode\" \"$1\" --exact \"$2\" --nocapture; done";
        let (pty, output) = Pty::spawn_with_env_in(
            "/bin/sh",
            &["-c", script, "sh", executable.to_str().unwrap(), &test_name],
            Size {
                cols: 100,
                rows: 25,
            },
            &[(quota::ENDPOINT_ENV, Some(endpoint.as_os_str()))],
            None,
        )
        .unwrap();
        Self { pty, output }
    }

    fn bind(&self, broker: &Broker) {
        broker
            .attach(
                self.pty.foreground_scope().unwrap(),
                fs::canonicalize("/bin/sh").unwrap(),
            )
            .unwrap();
    }

    fn run(&mut self, mode: &str) {
        self.pty.write(format!("{mode}\n").as_bytes()).unwrap();
        self.wait_for_output(&format!("quota-fixture-result={mode}"));
    }

    fn wait_for_output(&self, witness: &str) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = Vec::new();
        while !String::from_utf8_lossy(&output).contains(witness) {
            let result = self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()));
            assert!(
                result.is_ok(),
                "helper did not emit {witness}: {}",
                String::from_utf8_lossy(&output)
            );
            output.extend(result.unwrap());
        }
    }
}

fn wait_for(broker: &Broker, predicate: impl Fn(Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let snapshot = broker.snapshot();
        if predicate(snapshot) {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "last quota state: {snapshot:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn native_receiver_admits_its_launch_and_rejects_another_live_launch() {
    let wake = Wake::new(|| {});
    let first = Broker::prepare(wake.clone()).unwrap();
    let second = Broker::prepare(wake).unwrap();
    let mut primary = Launch::new(first.endpoint());
    primary.bind(&first);
    let mut foreign = Launch::new(first.endpoint());
    foreign.bind(&second);
    foreign.run("foreign");
    assert_eq!(first.snapshot(), Snapshot::Waiting);
    assert_eq!(second.snapshot(), Snapshot::Waiting);
    primary.run("good");
    let good = wait_for(&first, |snapshot| {
        matches!(snapshot, Snapshot::Observed { .. })
    });
    assert!(matches!(
        good,
        Snapshot::Observed {
            five_hour: WindowState::Reported(Window {
                used_percentage: Some(0.0),
                ..
            }),
            seven_day: WindowState::Absent,
            ..
        }
    ));
    for mode in ["wrong-sequence", "oversized", "stall"] {
        primary.run(mode);
        assert_eq!(first.snapshot(), good);
    }
    assert_eq!(second.snapshot(), Snapshot::Waiting);
    first.stop();
    assert_eq!(first.snapshot(), Snapshot::Disconnected);
    assert_eq!(second.snapshot(), Snapshot::Waiting);
    foreign.pty.write(b"quit\n").unwrap();
    wait_for(&second, |snapshot| snapshot == Snapshot::Disconnected);
}

#[test]
fn native_receiver_requires_the_expected_executable_and_revokes_in_flight() {
    let wake = Wake::new(|| {});
    let mismatched = Broker::prepare(wake.clone()).unwrap();
    let mut unrelated = Launch::new(mismatched.endpoint());
    mismatched
        .attach(
            unrelated.pty.foreground_scope().unwrap(),
            fs::canonicalize("/bin/false").unwrap(),
        )
        .unwrap();
    unrelated.run("foreign");
    assert_eq!(mismatched.snapshot(), Snapshot::Waiting);

    let broker = Broker::prepare(wake).unwrap();
    let mut primary = Launch::new(broker.endpoint());
    primary.bind(&broker);
    primary.pty.write(b"revoked\n").unwrap();
    primary.wait_for_output("quota-fixture-admitted");
    broker.stop();
    primary.pty.write(b"send\n").unwrap();
    primary.wait_for_output("quota-fixture-result=revoked");
    assert_eq!(broker.snapshot(), Snapshot::Disconnected);
}

#[test]
fn hidden_receipt_and_expiry_do_not_wake_the_view() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let wake = Wake::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    });
    let broker = Broker::prepare(wake).unwrap();
    let mut primary = Launch::new(broker.endpoint());
    primary.bind(&broker);
    primary.run("expiry");
    wait_for(&broker, |snapshot| {
        matches!(
            snapshot,
            Snapshot::Observed {
                five_hour: WindowState::Expired,
                seven_day: WindowState::Absent,
                ..
            }
        )
    });
    assert_eq!(wakes.load(Ordering::Relaxed), 0);
}

#[test]
fn received_quota_is_painted_only_in_the_open_view_and_the_chip_declares_its_reading() {
    use eframe::egui;

    let wake = Wake::new(|| {});
    let broker = Arc::new(Broker::prepare(wake.clone()).unwrap());
    let mut primary = Launch::new(broker.endpoint());
    primary.bind(&broker);
    primary.run("good");
    wait_for(&broker, |snapshot| {
        matches!(snapshot, Snapshot::Observed { .. })
    });

    let ctx = egui::Context::default();
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| style.animation_time = 0.0);
    }
    let mut window = crate::tests::window_of_tabs([["cat", "cat"]]);
    window.usage.quota.install_fixture(broker, wake);
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 720.0),
        )),
        ..Default::default()
    };
    let has_quota = |shapes: &[egui::epaint::ClippedShape], needle: &str| {
        shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains(needle))
        })
    };
    window.usage.open = true;
    window.usage.quota.open(&ctx);
    // The first modal pass computes its size before drawing.
    ctx.run_ui(input(), |ui| crate::usage::begin_pass(&mut window, ui))
        .textures_delta
        .clear();
    window.usage.timing = Default::default();
    let before = window.timed_separately;
    let mut visible = ctx.run_ui(input(), |ui| crate::usage::begin_pass(&mut window, ui));
    visible.textures_delta.clear();
    assert!(has_quota(&visible.shapes, "0.0% used"));
    assert!(has_quota(&visible.shapes, "account unverified"));
    let paint_cost = window.usage.timing.pending[1].unwrap();
    let chip_cost = window.usage.timing.pending[super::super::report::CHIP].unwrap();
    assert!(paint_cost > Duration::ZERO);
    assert_eq!(window.timed_separately - before, paint_cost + chip_cost);
    window.usage.close_frame();

    // Closed, the view paints nothing; what the chip still reads is declared under its own series
    // and nothing else is set aside (D132).
    window.usage.close(&ctx);
    let before = window.timed_separately;
    let mut hidden = ctx.run_ui(input(), |ui| crate::usage::begin_pass(&mut window, ui));
    hidden.textures_delta.clear();
    assert!(!has_quota(&hidden.shapes, "0.0% used"));
    assert_eq!(window.usage.timing.pending[1], None);
    let chip_cost = window.usage.timing.pending[super::super::report::CHIP].unwrap();
    assert_eq!(window.timed_separately - before, chip_cost);
    window.usage.close_frame();
    let mut report = Vec::new();
    window.usage.publish(&mut report).unwrap();
    assert!(
        String::from_utf8(report)
            .unwrap()
            .contains("Usage paint: frames=1")
    );
}

/// While the chip shows Claude and AI usage is closed, a launch wakes the window when its windows
/// change and not when it repeats them (D132).
#[test]
fn with_the_view_closed_only_a_changed_value_wakes_the_window_for_the_chip() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let wake = Wake::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    });
    wake.set_chip(true);
    let broker = Broker::prepare(wake.clone()).unwrap();
    let mut launch = Launch::new(broker.endpoint());
    launch.bind(&broker);
    let used = |snapshot: Snapshot| match snapshot {
        Snapshot::Observed {
            five_hour: WindowState::Reported(window),
            ..
        } => window.used_percentage,
        _ => None,
    };
    // A snapshot is visible a moment before its wake is requested, outside the broker's lock.
    let settled = || {
        std::thread::sleep(Duration::from_millis(50));
        wakes.load(Ordering::Relaxed)
    };
    launch.run("steady");
    wait_for(&broker, |snapshot| used(snapshot) == Some(10.0));
    assert_eq!(settled(), 1);
    // The chip's frame clears the pending wake; then the same values arrive again.
    wake.begin_frame();
    launch.run("steady");
    // The repeat changes nothing a test can wait for, so the pause lets it land before the next
    // frame clears the pending wake: a repeat that woke the window shows in the final count.
    std::thread::sleep(Duration::from_millis(150));
    wake.begin_frame();
    launch.run("moved");
    wait_for(&broker, |snapshot| used(snapshot) == Some(20.0));
    assert_eq!(settled(), 2, "the repeat must not wake");
    wake.set_chip(false);
    wake.begin_frame();
    launch.run("steady");
    wait_for(&broker, |snapshot| used(snapshot) == Some(10.0));
    assert_eq!(settled(), 2, "no chip, no view: no wake");
}

#[test]
fn visible_updates_coalesce_across_launches_and_stop_when_hidden() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let wake = Wake::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    });
    let first = Broker::prepare(wake.clone()).unwrap();
    let second = Broker::prepare(wake.clone()).unwrap();
    wake.set_visible(true);
    first.stop();
    second.stop();
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    wake.begin_frame();
    let third = Broker::prepare(wake.clone()).unwrap();
    third.stop();
    assert_eq!(wakes.load(Ordering::Relaxed), 2);
    wake.set_visible(false);
    let fourth = Broker::prepare(wake).unwrap();
    fourth.stop();
    assert_eq!(wakes.load(Ordering::Relaxed), 2);
}
