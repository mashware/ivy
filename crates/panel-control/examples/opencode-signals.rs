//! Native plugin/helper/channel witness; it never launches an AI client or a model request.

use panel_contract::PanelDir;
use panel_control::{Answer, Channel, Delivery, What};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

struct Pane {
    dir: PanelDir,
    channel: Channel,
    inbox: Receiver<Delivery>,
    #[cfg(windows)]
    releases: panel_control::ReleaseRegistry,
}

impl Pane {
    fn open(#[allow(unused_variables)] window: &Window) -> io::Result<Self> {
        let dir = PanelDir::create()?;
        let (sender, inbox) = sync_channel(8);
        #[cfg(windows)]
        let (channel, releases) = {
            let (services, _, _) = panel_control::WindowsServices::new(window.0 as isize)?;
            let releases = services.release_registry();
            let channel = Channel::open_windows(dir.path(), dir.id(), sender, services)?;
            (channel, releases)
        };
        #[cfg(unix)]
        let channel = Channel::open(dir.path(), dir.id(), sender, std::sync::Arc::new(|| {}))?;
        Ok(Self {
            dir,
            channel,
            inbox,
            #[cfg(windows)]
            releases,
        })
    }

    fn drain(&self, pane: usize, witness: &mut Witness, log: &mut std::fs::File) -> io::Result<()> {
        while let Ok(mut delivery) = self.inbox.try_recv() {
            match &delivery.what {
                What::AgentSignal(snapshot) => {
                    let snapshot = snapshot.as_ref().map(|snapshot| {
                        witness.busy[pane] |= snapshot.activity == panel_control::Activity::Busy;
                        let waiting = snapshot.permission || snapshot.question;
                        witness.waited[pane] |= waiting;
                        witness.cleared[pane] |= witness.waited[pane] && !waiting;
                        witness.waiting[pane] = waiting;
                        serde_json::json!({
                            "session": snapshot.session,
                            "permission": snapshot.permission,
                            "question": snapshot.question,
                            "stale": snapshot.stale,
                            "activity": match snapshot.activity {
                                panel_control::Activity::Busy => "busy",
                                panel_control::Activity::Idle => "idle",
                                panel_control::Activity::Unknown => "unknown",
                            },
                        })
                    });
                    if snapshot.is_none() {
                        witness.withdrawn[pane] = true;
                        witness.waiting[pane] = false;
                    }
                    witness.overlap |= witness.waiting == [true, true];
                    witness.independent_clear |= witness.cleared[0] && witness.waiting[1];
                    writeln!(
                        log,
                        "{}",
                        serde_json::json!({"pane":pane,"snapshot":snapshot})
                    )?;
                    log.flush()?;
                    delivery
                        .answer
                        .send(Answer::Taken)
                        .map_err(io::Error::other)?;
                }
                What::Closed => {}
                other => {
                    return Err(io::Error::other(format!(
                        "helper escaped its signal-only mode: {other:?}"
                    )));
                }
            }
            #[cfg(windows)]
            delivery.complete();
            #[cfg(not(windows))]
            let _ = &mut delivery;
        }
        #[cfg(windows)]
        let _ = self.releases.take_closed_ready();
        Ok(())
    }
}

#[derive(Default)]
struct Witness {
    busy: [bool; 2],
    waiting: [bool; 2],
    waited: [bool; 2],
    cleared: [bool; 2],
    withdrawn: [bool; 2],
    overlap: bool,
    independent_clear: bool,
}

#[cfg(windows)]
struct Window(windows_sys::Win32::Foundation::HWND);
#[cfg(not(windows))]
struct Window;

impl Window {
    fn open() -> io::Result<Self> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, HWND_MESSAGE};
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            // SAFETY: STATIC is a system class; the hidden message window is owned by this thread.
            let handle = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(handle))
        }
        #[cfg(not(windows))]
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the window was created by this thread and outlives all channel workers.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.0);
        }
    }
}

fn run() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let script = PathBuf::from(
        args.next()
            .ok_or_else(|| io::Error::other("expected Node fixture path"))?,
    );
    let helper = PathBuf::from(
        args.next()
            .ok_or_else(|| io::Error::other("expected helper path"))?,
    );
    if args.next().is_some() || !script.is_absolute() || !helper.is_absolute() {
        return Err(io::Error::other(
            "fixture and helper must be absolute paths",
        ));
    }
    let window = Window::open()?;
    let mut panes = [Pane::open(&window)?, Pane::open(&window)?];
    let events = panes[0].dir.path().join("test-signal-events.jsonl");
    let mut log = std::fs::File::create(&events)?;
    let identities: Vec<_> = panes
        .iter()
        .map(|pane| {
            serde_json::json!({
                "id":pane.dir.id(),"dir":pane.dir.path(),"pid":std::process::id(),
            })
        })
        .collect();
    let mut child = Command::new("node")
        .arg("--test")
        .arg(script)
        .env("IVY_TEST_PANES", serde_json::to_string(&identities)?)
        .env("IVY_TEST_HELPER", helper)
        .env("IVY_TEST_EVENTS", events)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut witness = Witness::default();
    let result = 'exchange: loop {
        for (index, pane) in panes.iter().enumerate() {
            if let Err(failure) = pane.drain(index, &mut witness, &mut log) {
                break 'exchange Err(failure);
            }
        }
        if let Some(status) = child.try_wait()? {
            break if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!("Node fixture failed: {status}")))
            };
        }
        if Instant::now() >= deadline {
            break Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "native plugin witness exceeded its watchdog",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(windows)]
    for pane in &mut panes {
        pane.channel.shutdown();
    }
    #[cfg(not(windows))]
    for pane in &mut panes {
        let _ = &pane.channel;
    }
    result?;
    if !witness.busy.iter().all(|x| *x)
        || !witness.waited.iter().all(|x| *x)
        || !witness.cleared.iter().all(|x| *x)
        || !witness.withdrawn.iter().all(|x| *x)
        || !witness.overlap
        || !witness.independent_clear
    {
        return Err(io::Error::other(format!(
            "incomplete witness: busy={:?} wait={:?} clear={:?} withdraw={:?} overlap={} independent_clear={}",
            witness.busy,
            witness.waited,
            witness.cleared,
            witness.withdrawn,
            witness.overlap,
            witness.independent_clear
        )));
    }
    println!(
        "PASS OpenCode actual plugin/helper/channel: busy, wait, clear, withdrawal and two-pane isolation; transport={}",
        if cfg!(windows) { "Windows" } else { "Unix" }
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("OpenCode native witness: {error}");
        std::process::exit(1);
    }
}
