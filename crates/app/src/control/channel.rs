//! Optional off-frame services share the pane endpoint without changing panel ownership.
use super::*;

#[cfg(test)]
#[cfg(unix)]
pub(crate) fn open_with_sessions(
    dir: &PanelDir,
    reader: crate::panel::DocumentReader,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<PaneControl> {
    open_services(dir, reader, reads, sessions, None, None, wake, None)
}

#[allow(clippy::too_many_arguments)] // UI capabilities are assembled at this one boundary.
pub(crate) fn open_with_authority(
    dir: &PanelDir,
    state: &PanelState,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    authority: Option<Arc<crate::panel::overlay::Authority>>,
    inspector: panel_control::PublicationInspector,
    wake: impl Fn() + Send + Sync + 'static,
    hwnd: Option<isize>,
) -> Option<PaneControl> {
    open_services(
        dir,
        state.reader(),
        reads,
        sessions,
        authority.map(|authority| (state, authority)),
        Some(inspector),
        wake,
        hwnd,
    )
}

#[allow(clippy::too_many_arguments)] // Mirrors the optional platform capabilities above.
fn open_services(
    dir: &PanelDir,
    reader: crate::panel::DocumentReader,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    authorization: Option<(&PanelState, Arc<crate::panel::overlay::Authority>)>,
    inspector: Option<panel_control::PublicationInspector>,
    wake: impl Fn() + Send + Sync + 'static,
    hwnd: Option<isize>,
) -> Option<PaneControl> {
    #[cfg(not(windows))]
    let _ = hwnd;
    let (sender, inbox) = sync_channel(MAX_PENDING);
    #[cfg(test)]
    let test_sender = sender.clone();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
    #[cfg(not(windows))]
    let read_document = measured_reader(reader, reads.clone());
    #[cfg(not(windows))]
    let (publisher, authority) = authorization.map_or((None, None), |(state, authority)| {
        let handler = crate::panel::overlay::service(
            state,
            Arc::clone(&authority),
            sender.clone(),
            Arc::clone(&wake),
        );
        (Some(handler), Some(authority))
    });
    #[cfg(windows)]
    let (opened, windows_publishers) = {
        let _ = (reader, sessions, authorization, inspector);
        let opened = hwnd
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "the native control window handle is unavailable",
                )
            })
            .and_then(panel_control::WindowsServices::new)
            .and_then(|(services, document, plan)| {
                let releases = services.release_registry();
                Channel::open_windows(dir.path(), dir.id(), sender, services)
                    .map(|channel| (channel, document, plan, releases))
            });
        match opened {
            Ok((channel, document, plan, releases)) => {
                (Ok(channel), Some((document, plan, releases)))
            }
            Err(problem) => (Err(problem), None),
        }
    };
    #[cfg(windows)]
    let authority = None;
    #[cfg(not(windows))]
    let opened = match inspector {
        Some(inspector) => Channel::open_with_publication_inspector(
            dir.path(),
            dir.id(),
            sender,
            Some(read_document),
            sessions,
            publisher,
            Some(inspector),
            Arc::clone(&wake),
        ),
        None => Channel::open_with_authorization_services(
            dir.path(),
            dir.id(),
            sender,
            Some(read_document),
            sessions,
            publisher,
            Arc::clone(&wake),
        ),
    };
    match opened {
        Ok(channel) => {
            #[cfg(not(windows))]
            channel.set_plan_reader(Arc::new(|| {
                panel_control::Reply::Failed("this console's project is not known yet".into())
            }));
            #[cfg(windows)]
            let (windows_document, windows_plan, windows_releases) = windows_publishers
                .expect("Windows channel creation retains its concrete publishers");
            Some(PaneControl {
                authority,
                plan_identity: None,
                _channel: channel,
                inbox,
                #[cfg(test)]
                test_sender,
                wake,
                waiting_on_the_window: Vec::new(),
                did: Control::default(),
                reads,
                #[cfg(windows)]
                windows_document,
                #[cfg(windows)]
                windows_plan,
                #[cfg(windows)]
                windows_releases,
            })
        }
        // **Binding nothing on a platform that cannot assert who may connect is policy, not a
        // fault, so it is not reported.** Said once per pane at every start it reads as a broken
        // terminal, and the person reading it has nothing to do about it: the platform gets its
        // channel when F2.7 can put a permission on the endpoint.
        Err(failure) if failure.kind() == std::io::ErrorKind::Unsupported => None,
        Err(failure) => {
            eprintln!(
                "console {}: it goes without a control channel, its endpoint could not be bound ({failure})",
                dir.id()
            );
            None
        }
    }
}

#[cfg(windows)]
impl Drop for PaneControl {
    fn drop(&mut self) {
        self.windows_document.revoke();
        self.windows_plan.revoke();
        self.windows_releases.revoke();
    }
}

impl PaneControl {
    #[cfg(not(windows))]
    pub(crate) fn install_plan_reader(&self, reader: panel_control::listener::ReadPlan) {
        self._channel.set_plan_reader(reader);
    }
}

/// Only Windows wakes the window through its native handle; elsewhere there is nothing to keep.
pub(crate) fn remember_native_window(cc: &eframe::CreationContext<'_>) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = cc
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            });
        if let Some(hwnd) = hwnd {
            cc.egui_ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("ivy-panel-control-hwnd"), hwnd);
            });
        }
    }
    #[cfg(not(windows))]
    let _ = cc;
}

pub(crate) fn control_hwnd(ctx: &egui::Context) -> Option<isize> {
    #[cfg(windows)]
    {
        ctx.data(|data| data.get_temp(egui::Id::new("ivy-panel-control-hwnd")))
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        None
    }
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use std::ptr::{null, null_mut};

    #[test]
    fn windows_control_native_wake_posts_without_waiting_for_dispatch() {
        use windows_sys::Win32::{
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, MSG, PM_REMOVE, PeekMessageW,
                RegisterClassW, UnregisterClassW, WM_PAINT, WNDCLASSW,
            },
        };

        let class_name: Vec<u16> = format!("IvyControlWake{}", std::process::id())
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let instance = unsafe { GetModuleHandleW(null()) };
        let class = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        assert_ne!(
            unsafe { RegisterClassW(&class) },
            0,
            "register native test window"
        );
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
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
        assert!(!hwnd.is_null(), "create native message window");
        let wake = panel_control::WindowsWake::from_hwnd(hwnd as isize).unwrap();
        wake.post().expect("post native wake without dispatch");
        let mut message = MSG::default();
        assert_ne!(
            unsafe { PeekMessageW(&mut message, hwnd, WM_PAINT, WM_PAINT, PM_REMOVE) },
            0,
            "the posted WM_PAINT is queued while the window thread has not dispatched it"
        );
        wake.revoke();
        assert!(wake.post().is_err(), "revoked HWND rejects later posts");
        unsafe {
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), instance);
        }
    }
}
