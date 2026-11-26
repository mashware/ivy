//! Claude statusline forwarding and best-effort quota publication.

#[cfg(target_os = "linux")]
#[path = "quota_statusline/linux.rs"]
mod linux;

pub fn run(command: Option<&std::ffi::OsStr>) -> i32 {
    #[cfg(target_os = "linux")]
    {
        linux::run(command)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = command;
        2
    }
}
