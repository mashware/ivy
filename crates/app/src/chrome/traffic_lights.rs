//! macOS lays its own close, minimize and zoom buttons out in a titlebar about 28 points tall, the
//! integrated header is taller, and nothing in winit 0.30 or eframe 0.36 moves them — so they are
//! centred here (D66).
//!
//! **AppKit puts them back whenever it lays its titlebar out again**, and it can do so after this
//! pass. So the frames are compared on every pass, written only when they differ, and a correction
//! asks for one more pass to look again — a few in a row at most, so a titlebar that keeps fighting
//! back cannot keep the window awake. The container is `closeButton.superview.superview`: a private
//! hierarchy, the one Electron's `trafficLightPosition` and Zed rely on, so anything other than what
//! is expected leaves the buttons where AppKit put them. Full screen is left alone: AppKit moves the
//! buttons into a bar of its own there.
//!
//! The geometry is kept apart from AppKit so a Linux test can hold it.

/// The titlebar container's bottom edge and height, in AppKit points (origin at the bottom), for a
/// window `window_height` tall under a header `header` tall. A window shorter than the header gets a
/// header as tall as itself, which is how the header is painted too.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn container(window_height: f64, header: f64) -> (f64, f64) {
    let height = header.min(window_height);
    (window_height - height, height)
}

/// Where a button `button_height` tall sits inside a container `height` tall: centred, and on a
/// whole device pixel, because half a point is a visible pixel on a Retina display.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn button_y(height: f64, button_height: f64, scale: f64) -> f64 {
    (((height - button_height) / 2.0) * scale).round() / scale
}

/// Centres the window buttons in the top `header` points of the window. Answers whether anything
/// had to be moved.
#[cfg(target_os = "macos")]
pub(crate) fn center(handle: &impl raw_window_handle::HasWindowHandle, header: f64) -> bool {
    use objc2_app_kit::{NSView, NSWindowButton, NSWindowStyleMask};
    use objc2_foundation::NSPoint;
    use raw_window_handle::RawWindowHandle;

    let Ok(handle) = handle.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    // SAFETY: the handle is winit's content view, borrowed for as long as `handle` lives, and this
    // runs on the main thread, inside the event loop's pass.
    let view = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    let Some(window) = view.window() else {
        return false;
    };
    if window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return false;
    }
    let [Some(close), Some(minimize), Some(zoom)] = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|which| window.standardWindowButton(which)) else {
        return false;
    };
    // SAFETY: `superview` is unsafe only because the hierarchy may change under the caller; it is
    // read and used within this pass, on the main thread.
    let Some(bar) = (unsafe { close.superview().and_then(|bar| bar.superview()) }) else {
        return false;
    };
    let mut moved = false;
    let (bottom, height) = container(window.frame().size.height, header);
    let mut wanted = bar.frame();
    if wanted.origin.y != bottom || wanted.size.height != height {
        wanted.origin.y = bottom;
        wanted.size.height = height;
        bar.setFrame(wanted);
        moved = true;
    }
    let scale = window.backingScaleFactor();
    for button in [close, minimize, zoom] {
        let at = button.frame();
        let y = button_y(height, at.size.height, scale);
        if at.origin.y != y {
            button.setFrameOrigin(NSPoint::new(at.origin.x, y));
            moved = true;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_container_hangs_from_the_top_of_the_window_as_tall_as_the_header() {
        assert_eq!(container(800.0, 49.0), (751.0, 49.0));
        assert_eq!(container(30.0, 49.0), (0.0, 30.0));
    }

    #[test]
    fn a_button_is_centred_on_a_whole_device_pixel() {
        // 49 − 14 leaves 35: a Retina display can draw the half point, a 1× display cannot.
        assert_eq!(button_y(49.0, 14.0, 2.0), 17.5);
        assert_eq!(button_y(49.0, 14.0, 1.0), 18.0);
        assert_eq!(button_y(49.0, 16.0, 2.0), 16.5);
    }
}
