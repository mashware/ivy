#![cfg(test)]

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui;
use panel_contract::PanelDir;
use panel_control::{Producer, PublicationOrigin};

const OUTSIDE: &str = "published from a process outside this console";

struct MessageWindow(windows_sys::Win32::Foundation::HWND);
impl MessageWindow {
    fn new(ctx: &egui::Context) -> Self {
        use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW;
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: the terminated class is live, and HWND_MESSAGE needs no visible owner.
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
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("ivy-panel-control-hwnd"), hwnd as isize)
        });
        Self(hwnd)
    }
}
impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: this test owns the window and destroys it on its creating thread.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.0);
        }
    }
}

fn quote(path: &Path) -> String {
    path.to_str().unwrap().replace('\'', "''")
}

fn launch(
    ctx: &egui::Context,
    dir: PanelDir,
    target: &Path,
    id: &str,
    source: &str,
) -> crate::Pane {
    let executable = std::env::current_exe().unwrap();
    let trigger = dir.path().join("trigger");
    let ready = dir.path().join("ready");
    let done = dir.path().join("published");
    let release = dir.path().join("release");
    let script = format!(
        "$ErrorActionPreference='Stop'; $deadline=[DateTime]::UtcNow.AddSeconds(90); \
         [IO.File]::WriteAllText('{}','ready'); \
         while (!(Test-Path -LiteralPath '{}') -and [DateTime]::UtcNow -lt $deadline) {{ Start-Sleep -Milliseconds 10 }}; \
         $env:PANEL_DIR='{}'; $env:PANEL_ID='{}'; $env:IVY_NATIVE_PROVENANCE_SOURCE='{}'; \
         & '{}' --exact panel::provenance_route::windows_tests::windows_provenance_publisher --nocapture; \
         if ($LASTEXITCODE -ne 0) {{ exit 1 }}; [IO.File]::WriteAllText('{}','published'); \
         while (!(Test-Path -LiteralPath '{}') -and [DateTime]::UtcNow -lt $deadline) {{ Start-Sleep -Milliseconds 10 }}",
        quote(&ready),
        quote(&trigger),
        quote(target),
        id,
        source,
        quote(&executable),
        quote(&done),
        quote(&release)
    );
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let program = format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe");
    crate::opening::launch_exact(
        ctx,
        crate::opening::Launch {
            program: &program,
            args: &[
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-Command".into(),
                script,
            ],
            directory: None,
            path: None,
        },
        Some(dir),
        0,
        crate::control::ReadActivity::default(),
    )
    .unwrap()
}

fn drive(pane: &mut crate::Pane, ctx: &egui::Context, mut ready: impl FnMut(&crate::Pane) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        pane.pump(Default::default());
        pane.refresh_its_panel(ctx);
        if ready(pane) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native publication did not reach its barrier"
        );
        std::thread::yield_now();
    }
}

fn assert_notice(pane: &mut crate::Pane, origin: PublicationOrigin) {
    assert_eq!(pane.panel_state.published_by, origin);
    assert!(!pane.panel_state.document().unwrap().contains(OUTSIDE));
    let (rows, _, _) = pane
        .panel_state
        .lay_out_if_needed(
            "pane",
            crate::panel::cache::LayoutKey {
                width: 100,
                theme: crate::theme::Theme::default(),
                warning: None,
                chrome: crate::panel::Chrome::Historical,
            },
            None,
        )
        .unwrap();
    assert_eq!(
        rows.iter().filter(|row| row.text == OUTSIDE).count(),
        usize::from(origin == PublicationOrigin::Outside)
    );
}

#[test]
fn windows_provenance_publisher() {
    let Ok(source) = std::env::var("IVY_NATIVE_PROVENANCE_SOURCE") else {
        return;
    };
    let document = serde_json::json!({"header":false,"lines":[source.clone()]}).to_string();
    Producer::of_this_pane()
        .unwrap()
        .publish(&source, &document)
        .unwrap();
}

#[test]
fn windows_provenance_opened_panes_label_only_verified_outside_and_keep_unknown() {
    let ctx = egui::Context::default();
    let _window = MessageWindow::new(&ctx);
    let first_dir = PanelDir::create().unwrap();
    let endpoint = first_dir.path().to_owned();
    let id = first_dir.id().to_owned();
    let mut first = launch(&ctx, first_dir, &endpoint, &id, "inside");
    assert!(
        first.control.is_some(),
        "application installed its actual Windows channel"
    );
    let scope = first.pty.foreground_scope().unwrap();
    drive(&mut first, &ctx, |_| endpoint.join("ready").exists());
    std::fs::write(endpoint.join("trigger"), "go").unwrap();
    drive(&mut first, &ctx, |_| endpoint.join("published").exists());
    assert_notice(&mut first, PublicationOrigin::Inside);
    drive(&mut first, &ctx, |pane| {
        pane.panel_state.held_by().is_none()
    });

    let sibling_dir = PanelDir::create().unwrap();
    let sibling_endpoint = sibling_dir.path().to_owned();
    let mut sibling = launch(&ctx, sibling_dir, &endpoint, &id, "sibling");
    drive(&mut sibling, &ctx, |_| {
        sibling_endpoint.join("ready").exists()
    });
    std::fs::write(sibling_endpoint.join("trigger"), "go").unwrap();
    drive(&mut first, &ctx, |_| {
        sibling_endpoint.join("published").exists()
    });
    assert_notice(&mut first, PublicationOrigin::Outside);
    drive(&mut first, &ctx, |pane| {
        pane.panel_state.held_by().is_none()
    });

    let raw_dir = endpoint.clone();
    let raw_id = id.clone();
    let raw = std::thread::spawn(move || {
        let mut stream =
            panel_control::endpoint::connect_authenticated(&raw_dir, &raw_id, std::process::id())
                .unwrap();
        stream
            .set_recv_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(b"{\"tool\":\"panel_set\",\"source\":\"raw\",\"document\":{\"lines\":[\"raw accepted\"]}}\n").unwrap();
        let mut reply = Vec::new();
        loop {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            if byte[0] == b'\n' {
                break;
            }
            reply.push(byte[0]);
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&reply).unwrap()["ok"],
            true
        );
    });
    drive(&mut first, &ctx, |_| raw.is_finished());
    raw.join().unwrap();
    assert_notice(&mut first, PublicationOrigin::Unknown);
    assert!(
        first
            .panel_state
            .document()
            .unwrap()
            .contains("raw accepted")
    );
    drive(&mut first, &ctx, |pane| {
        pane.panel_state.held_by().is_none()
    });

    std::fs::write(endpoint.join("release"), "go").unwrap();
    drive(&mut first, &ctx, |_| !scope.is_open());
    let after_dir = endpoint.clone();
    let after_id = id.clone();
    let after = std::thread::spawn(move || {
        Producer::of_server(&after_dir, &after_id, std::process::id())
            .unwrap()
            .publish("after root exit", r#"{"lines":["revoked accepted"]}"#)
            .unwrap();
    });
    drive(&mut first, &ctx, |_| after.is_finished());
    after.join().unwrap();
    assert_notice(&mut first, PublicationOrigin::Unknown);
    assert!(
        first
            .panel_state
            .document()
            .unwrap()
            .contains("revoked accepted")
    );
    std::fs::write(sibling_endpoint.join("release"), "go").unwrap();
    first.control.as_mut().unwrap().stop_and_did_in_the_window();
}
