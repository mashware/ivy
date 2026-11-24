use super::*;
use std::os::windows::process::CommandExt;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[test]
fn capability_command_does_not_allocate_a_console() {
    const CASE: &str =
        "codex_relay::windows_console_tests::capability_command_does_not_allocate_a_console";
    const MARKER: &str = "codex-child-without-console";
    match std::env::var("IVY_CODEX_CONSOLE_TEST").as_deref() {
        Ok("witness") => {
            assert!(unsafe { GetConsoleWindow() }.is_null());
            println!("{MARKER}");
        }
        Ok("host") => {
            assert!(unsafe { GetConsoleWindow() }.is_null());
            // This host is isolated from other tests; its environment is inherited by the witness.
            unsafe { std::env::set_var("IVY_CODEX_CONSOLE_TEST", "witness") };
            let exe = std::env::current_exe().unwrap();
            let result = output(
                exe.as_os_str(),
                &[CASE, "--exact", "--nocapture"],
                Duration::from_secs(15),
            )
            .unwrap();
            assert!(result.contains(MARKER), "{result}");
        }
        _ => {
            let mut host = Command::new(std::env::current_exe().unwrap());
            host.args([CASE, "--exact", "--nocapture"])
                .env("IVY_CODEX_CONSOLE_TEST", "host")
                .creation_flags(0x0000_0008)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut host = host.spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if let Some(status) = host.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                if Instant::now() >= deadline {
                    kill_and_reap(host);
                    panic!("detached host timed out");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
