//! A native notice still works when the graphics needed by the application do not (D117).
use log::{Level, Log, Metadata, Record};
use std::sync::{
    Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

const TARGET: &str = "egui_glow::painter";
const VERSION_LIMIT: usize = 256;

struct Version {
    active: AtomicBool,
    found: Mutex<Option<String>>,
}

impl Version {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
            found: Mutex::new(None),
        }
    }

    fn ready(&self) {
        self.active.store(false, Ordering::Release);
        self.found
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

impl Log for Version {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.active.load(Ordering::Acquire)
            && metadata.target() == TARGET
            && metadata.level() == Level::Debug
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Only this initializer's record is formatted. No log stream or file is collected.
        let text = record.args().to_string();
        let Some(version) = text
            .strip_prefix("\nopengl version: ")
            .and_then(|rest| rest.split_once('\n').map(|(version, _)| version))
        else {
            return;
        };
        if version.is_empty()
            || version.len() > VERSION_LIMIT
            || version.chars().any(char::is_control)
        {
            return;
        }
        *self.found.lock().unwrap_or_else(PoisonError::into_inner) = Some(version.to_owned());
    }

    fn flush(&self) {}
}

#[cfg(windows)]
static VERSION: Version = Version::new();
#[cfg(windows)]
static OWNS_LOGGER: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub(super) fn start() {
    if log::set_logger(&VERSION).is_ok() {
        OWNS_LOGGER.store(true, Ordering::Release);
        log::set_max_level(log::LevelFilter::Debug);
    }
}

#[cfg(windows)]
pub(super) fn ready() {
    VERSION.ready();
    if OWNS_LOGGER.load(Ordering::Acquire) {
        log::set_max_level(log::LevelFilter::Off);
    }
}

fn message(error: &eframe::Error, version: Option<&str>) -> Option<String> {
    if !matches!(
        error,
        eframe::Error::OpenGL(_) | eframe::Error::Glutin(_) | eframe::Error::NoGlutinConfigs(..)
    ) {
        return None;
    }
    Some(format!(
        "Ivy could not initialize graphics.\n\nIvy requires OpenGL 2.0 or later.\nDetected OpenGL version: {}\n\nTry updating your graphics driver or using a session with graphics acceleration.\n\nDetails: {error}",
        version.unwrap_or("unavailable")
    ))
}

fn notify_with(error: &eframe::Error, version: Option<&str>, show: impl FnOnce(&str)) {
    if let Some(text) = message(error, version) {
        show(&text);
    }
}

#[cfg(windows)]
pub(super) fn notify(error: &eframe::Error) {
    let version = VERSION
        .found
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    ready();
    notify_with(error, version.as_deref(), |text| {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
        };
        let title: Vec<u16> = "Ivy could not start\0".encode_utf16().collect();
        let text: Vec<u16> = text
            .replace('\0', "?")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // Both strings are NUL-terminated and live for the entire synchronous call; no owner
        // window exists because initialization failed before the first visible frame.
        let shown = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
            )
        };
        if shown == 0 {
            eprintln!(
                "ivy: could not show the graphics startup error: {}",
                std::io::Error::last_os_error()
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(version: &Version, target: &str, level: Level, text: &str) {
        version.log(
            &Record::builder()
                .target(target)
                .level(level)
                .args(format_args!("{text}"))
                .build(),
        );
    }

    #[test]
    fn failing_context_version_reaches_the_notice_without_renderer_or_vendor() {
        let version = Version::new();
        feed(
            &version,
            TARGET,
            Level::Debug,
            "\nopengl version: 1.1.0\nopengl renderer: GDI Generic\nopengl vendor: Microsoft",
        );
        let found = version.found.lock().unwrap();
        let error = eframe::Error::OpenGL("egui_glow requires opengl 2.0+. ".to_owned().into());
        let mut notice = String::new();
        notify_with(&error, found.as_deref(), |text| notice = text.to_owned());
        assert!(notice.contains("OpenGL 2.0 or later"));
        assert!(notice.contains("Detected OpenGL version: 1.1.0"));
        assert!(!notice.contains("GDI Generic"));
        assert!(!notice.contains("Microsoft"));
    }

    #[test]
    fn absent_or_unrelated_records_never_invent_a_version() {
        let version = Version::new();
        for (target, level, text) in [
            ("other", Level::Debug, "\nopengl version: 1.1\n"),
            (TARGET, Level::Info, "\nopengl version: 1.1\n"),
            (TARGET, Level::Debug, "a different record"),
            (TARGET, Level::Debug, "\nopengl version: \n"),
            (TARGET, Level::Debug, "\nopengl version: 1.1\r\n"),
        ] {
            feed(&version, target, level, text);
        }
        feed(
            &version,
            TARGET,
            Level::Debug,
            &format!("\nopengl version: {}\n", "x".repeat(VERSION_LIMIT + 1)),
        );
        assert!(version.found.lock().unwrap().is_none());
        let error = eframe::Error::OpenGL("shader compilation failed".to_owned().into());
        let text = message(&error, None).unwrap();
        assert!(text.contains("Detected OpenGL version: unavailable"));
        assert!(text.contains("shader compilation failed"));
        assert!(!text.contains("unsupported version"));
        assert!(!text.contains("Detected OpenGL version: 1.1"));
    }

    #[test]
    fn successful_initialization_stops_retention_and_other_errors_show_no_graphics_notice() {
        let version = Version::new();
        feed(&version, TARGET, Level::Debug, "\nopengl version: 4.6\n");
        version.ready();
        feed(&version, TARGET, Level::Debug, "\nopengl version: 1.1\n");
        assert!(version.found.lock().unwrap().is_none());
        assert!(
            !version.enabled(
                &Metadata::builder()
                    .target(TARGET)
                    .level(Level::Debug)
                    .build()
            )
        );
        let error = eframe::Error::AppCreation(Box::new(std::io::Error::other("no shell")));
        notify_with(&error, None, |_| {
            panic!("application error was labelled as graphics failure")
        });
    }
}
