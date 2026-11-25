//! Native keyboard witness: real SendInput -> winit -> vendored egui-winit -> terminal bytes.
//! Run only on a disposable Windows desktop; it temporarily changes keyboard layouts and focus.

#[cfg(any(windows, test))]
struct TrialCases<T: 'static> {
    trials: &'static [T],
    completed: usize,
}

#[cfg(any(windows, test))]
impl<T> TrialCases<T> {
    fn current(&self) -> Option<&T> {
        self.trials.get(self.completed)
    }

    fn advance(&mut self) {
        assert!(self.current().is_some(), "No active trial to complete");
        self.completed += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::TrialCases;

    #[test]
    fn callbacks_after_the_last_trial_have_no_active_case() {
        let mut cases = TrialCases {
            trials: &["first", "last"],
            completed: 0,
        };
        assert_eq!(cases.current(), Some(&"first"));
        cases.advance();
        assert_eq!(cases.current(), Some(&"last"));
        cases.advance();
        // winit may still call both window_event and about_to_wait after exit is requested.
        for _callback in 0..2 {
            assert_eq!(cases.current(), None);
        }
        assert_eq!(cases.completed, 2);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This witness requires a disposable Windows desktop.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run()
}

#[cfg(windows)]
mod native {
    use super::TrialCases;
    use std::time::{Duration, Instant};

    use app::keyboard::{CursorKeys, bytes};
    use eframe::egui;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ActivateKeyboardLayout, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KLF_ACTIVATE, LoadKeyboardLayoutW, SendInput,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        platform::modifier_supplement::KeyEventExtModifierSupplement,
        window::{Window, WindowId},
    };

    struct Case {
        name: &'static str,
        layout: &'static str,
        modifiers: &'static [u16],
        key: u16,
        expected: &'static [u8],
    }

    const CASES: &[Case] = &[
        Case {
            name: "Spanish left Ctrl+Alt+2",
            layout: "0000040A",
            modifiers: &[0xA2, 0xA4],
            key: 0x32,
            expected: b"@",
        },
        Case {
            name: "Spanish real AltGr+2",
            layout: "0000040A",
            modifiers: &[0xA5],
            key: 0x32,
            expected: b"@",
        },
        Case {
            name: "US Ctrl+Alt+A",
            layout: "00000409",
            modifiers: &[0xA2, 0xA4],
            key: 0x41,
            expected: b"",
        },
        Case {
            name: "US Ctrl+Alt+Shift+A",
            layout: "00000409",
            modifiers: &[0xA2, 0xA4, 0xA0],
            key: 0x41,
            expected: b"",
        },
        Case {
            name: "German left Ctrl+Alt+Q",
            layout: "00000407",
            modifiers: &[0xA2, 0xA4],
            key: 0x51,
            expected: b"@",
        },
        Case {
            name: "Polish programmers Ctrl+Alt+C",
            layout: "00000415",
            modifiers: &[0xA2, 0xA4],
            key: 0x43,
            expected: "ć".as_bytes(),
        },
        Case {
            name: "Polish programmers Ctrl+Alt+X",
            layout: "00000415",
            modifiers: &[0xA2, 0xA4],
            key: 0x58,
            expected: "ź".as_bytes(),
        },
        Case {
            name: "US Ctrl+C",
            layout: "00000409",
            modifiers: &[0xA2],
            key: 0x43,
            expected: &[3],
        },
        Case {
            name: "US plain A",
            layout: "00000409",
            modifiers: &[],
            key: 0x41,
            expected: b"a",
        },
        Case {
            name: "Spanish plain 2",
            layout: "0000040A",
            modifiers: &[],
            key: 0x32,
            expected: b"2",
        },
    ];

    impl Case {
        fn physical_key(&self) -> PhysicalKey {
            PhysicalKey::Code(match self.key {
                0x32 => KeyCode::Digit2,
                0x41 => KeyCode::KeyA,
                0x43 => KeyCode::KeyC,
                0x51 => KeyCode::KeyQ,
                0x58 => KeyCode::KeyX,
                _ => panic!("Unlisted target key"),
            })
        }
    }

    fn input(key: u16, released: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: if released { KEYEVENTF_KEYUP } else { 0 }
                        | if key == 0xA5 {
                            KEYEVENTF_EXTENDEDKEY
                        } else {
                            0
                        },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send(case: &Case) {
        let mut inputs: Vec<_> = case
            .modifiers
            .iter()
            .map(|key| input(*key, false))
            .collect();
        inputs.push(input(case.key, false));
        inputs.push(input(case.key, true));
        inputs.extend(case.modifiers.iter().rev().map(|key| input(*key, true)));
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
        assert_eq!(
            sent as usize,
            inputs.len(),
            "SendInput failed for {}",
            case.name
        );
    }

    struct Witness {
        window: Option<Window>,
        adapter: Option<egui_winit::State>,
        original_layout: windows_sys::Win32::UI::Input::KeyboardAndMouse::HKL,
        cases: TrialCases<Case>,
        sent: bool,
        next: Instant,
        deadline: Instant,
        observed: Vec<u8>,
        keys: usize,
        failure: Option<String>,
    }

    impl Drop for Witness {
        fn drop(&mut self) {
            unsafe {
                ActivateKeyboardLayout(self.original_layout, 0);
            }
        }
    }

    impl ApplicationHandler for Witness {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let window = event_loop
                .create_window(
                    Window::default_attributes().with_title("Ivy Ctrl+Alt native witness"),
                )
                .unwrap();
            self.adapter = Some(egui_winit::State::new(
                egui::Context::default(),
                egui::ViewportId::ROOT,
                &window,
                None,
                None,
                None,
            ));
            window.focus_window();
            self.window = Some(window);
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            // winit can deliver queued callbacks after exit was requested by the last case.
            let Some(case) = self.cases.current() else {
                return;
            };
            let window = self.window.as_ref().unwrap();
            let adapter = self.adapter.as_mut().unwrap();
            if let WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } = &event
                && event.state == ElementState::Pressed
                && self.sent
            {
                if event.physical_key == case.physical_key() {
                    self.keys += 1;
                }
                println!(
                    "{} raw key={:?} text={:?} system={:?}",
                    case.name,
                    event.logical_key,
                    event.text,
                    event.text_with_all_modifiers()
                );
            }
            let _ = adapter.on_window_event(window, &event);
            let input = adapter.take_egui_input(window);
            for event in &input.events {
                if self.sent {
                    println!("{} egui={event:?}", case.name);
                    if let Some(bytes) = bytes(event, CursorKeys::Normal) {
                        self.observed.extend_from_slice(&bytes);
                    }
                    if !case.modifiers.is_empty()
                        && case.expected.iter().all(|b| *b >= 32)
                        && !case.expected.is_empty()
                    {
                        assert!(
                            !matches!(
                                event,
                                egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                            ),
                            "alternate text became a clipboard shortcut"
                        );
                    }
                }
            }
            if matches!(event, WindowEvent::CloseRequested) {
                panic!("Native keyboard witness closed before completion");
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            let Some(case) = self.cases.current() else {
                return;
            };
            assert!(
                Instant::now() < self.deadline,
                "Native keyboard witness timed out"
            );
            if Instant::now() < self.next {
                return;
            }
            let window = self.window.as_ref().unwrap();
            let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
                unreachable!()
            };
            let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            if !self.sent {
                unsafe {
                    SetForegroundWindow(hwnd);
                }
                if unsafe { GetForegroundWindow() } != hwnd || !window.has_focus() {
                    self.next = Instant::now() + Duration::from_millis(100);
                    event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
                    return;
                }
                let layout: Vec<u16> = case.layout.encode_utf16().chain(Some(0)).collect();
                let loaded = unsafe { LoadKeyboardLayoutW(layout.as_ptr(), KLF_ACTIVATE) };
                assert!(!loaded.is_null(), "Cannot load {}", case.layout);
                assert_eq!(
                    unsafe { GetKeyboardLayout(0) },
                    loaded,
                    "Layout activation failed"
                );
                self.observed.clear();
                self.keys = 0;
                self.sent = true;
                send(case);
                self.next = Instant::now() + Duration::from_millis(500);
            } else {
                assert!(
                    self.keys > 0,
                    "{} received no native target key press",
                    case.name
                );
                // An expected negative trial exits through the event loop, with its error
                // reported afterwards rather than unwinding through a native callback.
                if self.observed != case.expected {
                    self.failure = Some(format!(
                        "{} terminal bytes: expected {:?}, got {:?}",
                        case.name, case.expected, self.observed
                    ));
                    event_loop.exit();
                    return;
                }
                println!("PASS {} bytes={:?}", case.name, self.observed);
                self.cases.advance();
                if self.cases.current().is_none() {
                    println!("PASS all {} native keyboard cases", CASES.len());
                    event_loop.exit();
                    return;
                }
                self.sent = false;
                self.next = Instant::now() + Duration::from_millis(100);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            std::env::var("IVY_DISPOSABLE_RUNNER").as_deref(),
            Ok("yes"),
            "Native input is restricted to a disposable desktop"
        );
        let now = Instant::now();
        let mut witness = Witness {
            window: None,
            adapter: None,
            original_layout: unsafe { GetKeyboardLayout(0) },
            cases: TrialCases {
                trials: CASES,
                completed: 0,
            },
            sent: false,
            next: now + Duration::from_millis(300),
            deadline: now + Duration::from_secs(25),
            observed: Vec::new(),
            keys: 0,
            failure: None,
        };
        EventLoop::new()?.run_app(&mut witness)?;
        if let Some(failure) = witness.failure.take() {
            return Err(std::io::Error::other(failure).into());
        }
        assert_eq!(
            witness.cases.completed,
            CASES.len(),
            "Event loop ended without all native cases"
        );
        Ok(())
    }
}
