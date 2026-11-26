// A child-owned result file distinguishes process execution from ConPTY echo (D90).
use super::*;
use std::ffi::OsStr;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use term_core::{Pty, Size};
use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleW,
    System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, UnregisterClassW, WNDCLASSW,
    },
};

// Keep the actual bridge object: failure cleanup must never kill a recycled numeric PID.
struct BridgeCleanup(OwnedHandle);
impl BridgeCleanup {
    fn retain(work: &Path) -> Self {
        let pid = std::fs::read_to_string(work.join("bridge-pid"))
            .unwrap()
            .parse()
            .unwrap();
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, pid) };
        assert!(
            !handle.is_null(),
            "retain bridge cleanup handle: {}",
            std::io::Error::last_os_error()
        );
        Self(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}
impl Drop for BridgeCleanup {
    fn drop(&mut self) {
        unsafe {
            TerminateProcess(self.0.as_raw_handle(), 1);
            WaitForSingleObject(self.0.as_raw_handle(), 1000);
        }
    }
}

struct NativeWindow {
    hwnd: isize,
    class: Vec<u16>,
    instance: isize,
}
impl NativeWindow {
    fn new() -> Self {
        let class: Vec<u16> = format!(
            "IvyHookFixture{}{:?}",
            std::process::id(),
            std::thread::current().id()
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let instance = unsafe { GetModuleHandleW(null()) };
        let registration = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        assert_ne!(
            unsafe { RegisterClassW(&registration) },
            0,
            "register native fixture window"
        );
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                (-3isize) as _,
                null_mut(),
                instance,
                null(),
            )
        };
        assert!(!hwnd.is_null(), "create native fixture window");
        Self {
            hwnd: hwnd as isize,
            class,
            instance: instance as isize,
        }
    }
}

#[test]
fn windows_fresh_launch_worker_installs_hook_authority_in_its_actual_pane() {
    let native = NativeWindow::new();
    let ctx = egui::Context::default();
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ivy-panel-control-hwnd"), native.hwnd));
    let dir = PanelDir::create().unwrap();
    let work = std::env::temp_dir().join(format!("ivy-fresh-work-{}", std::process::id()));
    std::fs::create_dir(&work).unwrap();
    for (file, name) in [("bridge-path", "panel-mcp"), ("helper-path", "ivy-session")] {
        std::fs::write(work.join(file), helper(name).to_string_lossy().as_bytes()).unwrap();
    }
    let mut pane = crate::Pane::with_panel(
        &ctx,
        "cmd.exe",
        &["/d".into(), "/c".into(), "exit".into()],
        None,
        0,
    )
    .unwrap();
    pane.control.take();
    let selection = crate::launch::fresh::Selection {
        client: crate::launch::fresh::Client::Claude,
        images: [
            fixture_exe().to_str().unwrap().into(),
            String::new(),
            String::new(),
        ],
        directory: work.to_str().unwrap().into(),
    };
    let env = vec![
        ("PANEL_ID".into(), Some(dir.id().into())),
        ("PANEL_DIR".into(), Some(dir.path().as_os_str().to_owned())),
        (
            "PANEL_SERVER_PID".into(),
            Some(std::process::id().to_string().into()),
        ),
        (
            "IVY_SESSION_HELPER".into(),
            Some(helper("ivy-session").into_os_string()),
        ),
        ("IVY_CODEX_ROUTE".into(), None),
        ("IVY_FIXTURE_NESTED".into(), None),
    ];
    let (process, output) = crate::launch::fresh::start(
        Arc::new(|| {}),
        Default::default(),
        selection,
        Size::default(),
        env,
    )
    .unwrap();
    assert!(process.pending(), "fresh launch reserves a pending console");
    pane.pty = process;
    pane.pty.context = Some(ctx);
    pane.pty_output = crate::pty_output::PtyOutput::new(output, || {});
    pane.panel = Some(dir);
    let deadline = Instant::now() + Duration::from_secs(20);
    while pane.pty.pending() {
        crate::launch::complete(&mut pane);
        assert!(Instant::now() < deadline, "fresh worker did not complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        pane.pty.failure.is_none(),
        "fresh startup failed: {:?}",
        pane.pty.failure
    );
    assert!(pane.control.is_some(), "completion installs native control");
    wait_file(&work.join("ready"), || {});
    let _bridge_cleanup = BridgeCleanup::retain(&work);
    let witness = std::fs::read_to_string(work.join("launch-witness")).unwrap();
    let (argc, cwd) = witness.split_once('\n').unwrap();
    assert_eq!(
        argc, "0",
        "the actual fresh child received fixed empty argv"
    );
    assert_eq!(
        std::fs::canonicalize(cwd).unwrap(),
        std::fs::canonicalize(&work).unwrap()
    );
    std::fs::write(work.join("command"), "hold").unwrap();
    let reply = wait_file(&work.join("result"), || apply_pane(&mut pane));
    assert!(
        reply.contains("published") && !reply.contains("isError"),
        "fresh holder: {reply}"
    );
    let mut tab = crate::tree::Tab::with_panes(
        vec![pane],
        &crate::OpeningLayout {
            columns: 1,
            rows: 1,
        },
        0,
    );
    assert!(tab.attention(SystemTime::now()).is_some());
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "publish").unwrap();
    let reply = wait_file(&work.join("result"), || {});
    assert!(
        reply.contains("{\"ok\":true}"),
        "fresh worker hook authority: {reply}"
    );
    apply_pane(tab.panes_mut()[0]);
    assert!(tab.attention(SystemTime::now()).is_none());
    assert_eq!(tab.panes()[0].control.as_ref().unwrap().did().overlaid, 1);
    assert!(
        tab.panes()[0]
            .panel_state
            .document()
            .unwrap()
            .contains("After")
    );
    std::fs::write(work.join("command"), "exit").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while tab.panes_mut()[0].pty.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(tab.panes_mut()[0].pty.exit_success(), Some(true));
    drop(tab);
    let _ = std::fs::remove_dir_all(work);
}

fn apply_pane(pane: &mut crate::Pane) {
    pane.control.as_mut().unwrap().apply(
        &mut pane.panel_state,
        &mut pane.signals,
        SystemTime::now(),
    );
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd as _);
            UnregisterClassW(self.class.as_ptr(), self.instance as _);
        }
    }
}

fn fixture_exe() -> &'static Path {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let output =
            std::env::temp_dir().join(format!("ivy-hook-fixture-{}.exe", std::process::id()));
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/windows_hook_agent.rs");
        let status = std::process::Command::new("rustc")
            .arg("--edition=2024")
            .arg(&source)
            .arg("-o")
            .arg(&output)
            .status()
            .expect("run rustc for native fixture");
        assert!(status.success(), "compile native fixture {source:?}");
        output
    })
}
fn helper(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let debug = exe.parent().unwrap().parent().unwrap();
    let helper = debug.join(format!("{name}.exe"));
    assert!(
        helper.is_file(),
        "missing {helper:?}; run cargo build --locked -p panel-control --bins before native hook tests"
    );
    helper
}
fn wait_file(path: &Path, mut tick: impl FnMut()) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text;
        }
        if let Ok(failure) = std::fs::read_to_string(path.parent().unwrap().join("failure")) {
            panic!("fixture failed: {failure}");
        }
        assert!(
            Instant::now() < deadline,
            "fixture timed out waiting for {path:?}"
        );
        tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn windows_live_hook_real_bridge_and_helper_clear_wait() {
    let window = NativeWindow::new();
    let dir = PanelDir::create().unwrap();
    let work = std::env::temp_dir().join(format!("ivy-hook-work-{}", std::process::id()));
    std::fs::create_dir(&work).expect("create unique fixture directory");
    std::fs::write(
        work.join("bridge-path"),
        helper("panel-mcp").to_string_lossy().as_bytes(),
    )
    .unwrap();
    std::fs::write(
        work.join("helper-path"),
        helper("ivy-session").to_string_lossy().as_bytes(),
    )
    .unwrap();
    let server_pid = std::process::id().to_string();
    let (mut pty, output) = Pty::spawn_with_env_in(
        fixture_exe().to_str().unwrap(),
        &[],
        Size::default(),
        &[
            ("PANEL_ID", Some(OsStr::new(dir.id()))),
            ("PANEL_DIR", Some(dir.path().as_os_str())),
            ("PANEL_SERVER_PID", Some(OsStr::new(&server_pid))),
            ("IVY_CODEX_ROUTE", None),
            ("IVY_FIXTURE_NESTED", None),
        ],
        Some(&work),
    )
    .expect("launch fixture in native PTY");
    wait_file(&work.join("ready"), || while output.try_recv().is_ok() {});
    let _bridge_cleanup = BridgeCleanup::retain(&work);
    let scope = pty.foreground_scope().unwrap();
    let invocation = scope
        .fresh_invocation(fixture_exe(), &[])
        .expect("register actual native root");
    let authority = crate::panel::overlay::Authority::fresh(scope, invocation);
    let mut state = PanelState::default();
    let mut control = open_services(
        &dir,
        state.reader(),
        Default::default(),
        None,
        Some((&state, authority)),
        None,
        || {},
        Some(window.hwnd),
    )
    .expect("open native control channel");
    let mut signals = Default::default();
    std::fs::write(work.join("command"), "hold").unwrap();
    let reply = wait_file(&work.join("result"), || {
        control.apply(&mut state, &mut signals, SystemTime::now());
        while output.try_recv().is_ok() {}
    });
    assert!(
        reply.contains("published") && !reply.contains("isError"),
        "holder reply: {reply}"
    );
    assert!(state.attention(SystemTime::now()).is_some());
    let command =
        PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot")).join("System32/cmd.exe");
    let mut pane = crate::Pane::with_panel(
        &egui::Context::default(),
        command.to_str().unwrap(),
        &["/d".into(), "/c".into(), "pause".into()],
        None,
        0,
    )
    .expect("open disposable console for the actual tab aggregation");
    pane.panel_state = std::mem::take(&mut state);
    let mut tab = crate::tree::Tab::with_panes(
        vec![pane],
        &crate::OpeningLayout {
            columns: 1,
            rows: 1,
        },
        0,
    );
    assert!(tab.attention(SystemTime::now()).is_some());
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "publish").unwrap();
    let reply = wait_file(&work.join("result"), || while output.try_recv().is_ok() {});
    assert!(reply.contains("{\"ok\":true}"), "helper reply: {reply}");
    control.apply(
        &mut tab.panes_mut()[0].panel_state,
        &mut signals,
        SystemTime::now(),
    );
    assert!(tab.attention(SystemTime::now()).is_none());
    state = std::mem::take(&mut tab.panes_mut()[0].panel_state);
    assert_eq!(control.did().overlaid, 1);
    assert!(state.attention(SystemTime::now()).is_none());
    assert_eq!(state.held_by(), Some("flow"));
    let document = state.document().unwrap();
    assert!(
        document.contains("After")
            && document.contains("build")
            && document.contains("\"milestone\":\"m\""),
        "{document}"
    );
    assert!(
        !document.contains("Injected") && !document.contains("Approve"),
        "{document}"
    );
    let foreign_work = work.join("foreign");
    std::fs::create_dir(&foreign_work).unwrap();
    for name in ["bridge-path", "helper-path"] {
        std::fs::copy(work.join(name), foreign_work.join(name)).unwrap();
    }
    let (mut foreign, foreign_output) = Pty::spawn_with_env_in(
        fixture_exe().to_str().unwrap(),
        &[],
        Size::default(),
        &[
            ("PANEL_ID", Some(OsStr::new(dir.id()))),
            ("PANEL_DIR", Some(dir.path().as_os_str())),
            ("PANEL_SERVER_PID", Some(OsStr::new(&server_pid))),
            ("IVY_CODEX_ROUTE", None),
            ("IVY_FIXTURE_NESTED", None),
        ],
        Some(&foreign_work),
    )
    .expect("launch foreign PTY with copied endpoint");
    wait_file(&foreign_work.join("ready"), || {
        while foreign_output.try_recv().is_ok() {}
    });
    let _foreign_bridge_cleanup = BridgeCleanup::retain(&foreign_work);
    std::fs::write(foreign_work.join("command"), "publish").unwrap();
    let reply = wait_file(&foreign_work.join("result"), || {
        while foreign_output.try_recv().is_ok() {}
    });
    assert!(
        reply.contains("does not descend"),
        "foreign PTY reply: {reply}"
    );
    control.apply(&mut state, &mut signals, SystemTime::now());
    assert_eq!(state.document().unwrap(), document);
    std::fs::write(foreign_work.join("command"), "exit").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while foreign.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(foreign.exit_success(), Some(true), "foreign fixture exits");
    drop(foreign);
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "nested").unwrap();
    let reply = wait_file(&work.join("result"), || while output.try_recv().is_ok() {});
    assert!(
        reply.contains("does not descend"),
        "nested client reply: {reply}"
    );
    control.apply(&mut state, &mut signals, SystemTime::now());
    assert_eq!(state.document().unwrap(), document);
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "publish").unwrap();
    let reply = wait_file(&work.join("result"), || while output.try_recv().is_ok() {});
    assert!(
        reply.contains("{\"ok\":true}"),
        "second helper reply: {reply}"
    );
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "publish").unwrap();
    let reply = wait_file(&work.join("result"), || while output.try_recv().is_ok() {});
    assert!(
        reply.contains("holding connection cannot admit another update"),
        "pressure reply: {reply}"
    );
    let admitted = control
        .inbox
        .recv_timeout(Duration::from_secs(2))
        .expect("real hook was queued");
    let holder = match &admitted.what {
        What::Overlay { holder, .. } => *holder,
        _ => panic!("expected real queued overlay"),
    };
    state
        .deliver(
            holder,
            "flow",
            panel_contract::parse_document(r#"{"lines":["New generation"],"attention":"wait"}"#)
                .unwrap(),
            SystemTime::now(),
        )
        .unwrap();
    control.enqueue_for_tests(admitted);
    control.apply(&mut state, &mut signals, SystemTime::now());
    assert_eq!(
        control.did().overlays_refused,
        1,
        "the real queued overlay must not replace a newer generation"
    );
    assert!(state.document().unwrap().contains("New generation"));
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "hold").unwrap();
    let reply = wait_file(&work.join("result"), || {
        control.apply(&mut state, &mut signals, SystemTime::now());
        while output.try_recv().is_ok() {}
    });
    assert!(
        reply.contains("published") && !reply.contains("isError"),
        "renewed holder reply: {reply}"
    );
    let document = state.document().unwrap();
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "publish").unwrap();
    let reply = wait_file(&work.join("result"), || while output.try_recv().is_ok() {});
    assert!(
        reply.contains("{\"ok\":true}"),
        "last helper reply: {reply}"
    );
    std::fs::remove_file(work.join("result")).unwrap();
    std::fs::write(work.join("command"), "exit").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while pty.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        pty.exit_success(),
        Some(true),
        "fixture root must actually exit"
    );
    control.apply(&mut state, &mut signals, SystemTime::now());
    assert_eq!(
        control.did().overlays_refused,
        2,
        "queued update is refused after root exit"
    );
    assert_eq!(state.document().unwrap(), document);
    drop(control);
    drop(pty);
    let _ = std::fs::remove_dir_all(work);
}
