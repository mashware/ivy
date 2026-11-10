//! macOS lays its own close, minimize and zoom buttons out in a titlebar about 28 points tall, the
//! integrated header is taller, and nothing in winit 0.30 or eframe 0.36 moves them — so they are
//! centred here.
//!
//! **AppKit puts them back whenever it lays its titlebar out again**, and the only hooks that say when
//! belong to the window delegate, which winit owns. So the frames are compared on every pass and
//! written only when they differ. The container is `closeButton.superview.superview`: a private
//! hierarchy, the one Electron's `trafficLightPosition` and Zed rely on, so anything other than what is
//! expected leaves the buttons where AppKit put them. Full screen is left alone: AppKit moves the
//! buttons into a bar of its own there.
//!
//! Discarded: a unified `NSToolbar`, which centres the buttons by itself, because its view hit-tests
//! above the content and would take the clicks meant for the header's own buttons.
use objc2_app_kit::{NSView, NSWindowButton, NSWindowStyleMask};
use objc2_foundation::NSPoint;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const BUTTONS: [NSWindowButton; 3] = [
    NSWindowButton::CloseButton,
    NSWindowButton::MiniaturizeButton,
    NSWindowButton::ZoomButton,
];

/// Centres the window buttons in the top `header` points of the window, in AppKit points.
pub(crate) fn center(handle: &impl HasWindowHandle, header: f64) {
    let Ok(handle) = handle.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    // SAFETY: the handle is winit's content view, borrowed for as long as `handle` lives, and this
    // runs on the main thread, inside the event loop's pass.
    let view = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    let Some(window) = view.window() else {
        return;
    };
    if window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return;
    }
    let Some(buttons) = BUTTONS
        .iter()
        .map(|&which| window.standardWindowButton(which))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    // SAFETY: `superview` is unsafe only because the hierarchy may change under the caller; it is
    // read and used within this pass, on the main thread.
    let Some(container) = (unsafe { buttons[0].superview().and_then(|bar| bar.superview()) })
    else {
        return;
    };
    let mut wanted = container.frame();
    wanted.size.height = header;
    wanted.origin.y = window.frame().size.height - header;
    if container.frame() != wanted {
        container.setFrame(wanted);
    }
    for button in &buttons {
        let at = button.frame();
        let y = ((header - at.size.height) / 2.0).round();
        if at.origin.y != y {
            button.setFrameOrigin(NSPoint::new(at.origin.x, y));
        }
    }
}
