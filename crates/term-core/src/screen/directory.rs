//! Bounded OSC 7 metadata, independent of terminal painting.
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Directory {
    state: State,
    data: Vec<u8>,
    path: Option<PathBuf>,
    reported: bool,
}
#[derive(Default)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    OscEscape,
    Ignore,
    IgnoreEscape,
    String,
    StringEscape,
}
impl Directory {
    pub(super) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub(super) fn reported(&self) -> bool {
        self.reported
    }
    pub(super) fn advance(&mut self, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            if matches!(self.state, State::Ground) {
                let Some(offset) = bytes[index..].iter().position(|b| *b == 0x1b) else {
                    break;
                };
                index += offset;
            }
            let b = bytes[index];
            index += 1;
            self.state = match self.state {
                State::Ground => {
                    if b == 0x1b {
                        State::Escape
                    } else {
                        State::Ground
                    }
                }
                State::Escape => match b {
                    b']' => {
                        self.data.clear();
                        State::Osc
                    }
                    b'P' | b'_' | b'^' | b'X' => State::String,
                    0x1b => State::Escape,
                    b'c' => {
                        self.path = None;
                        self.reported = false;
                        State::Ground
                    }
                    _ => State::Ground,
                },
                State::Osc => match b {
                    7 => {
                        self.finish();
                        State::Ground
                    }
                    0x1b => State::OscEscape,
                    0x18 | 0x1a => State::Ground,
                    0..=31 | 127 => State::Ignore,
                    _ if self.data.len() < 8192 => {
                        self.data.push(b);
                        if self.data == b"7;" {
                            self.path = None;
                            self.reported = true;
                        }
                        State::Osc
                    }
                    _ => State::Ignore,
                },
                State::OscEscape => {
                    if b == b'\\' {
                        self.finish();
                        State::Ground
                    } else {
                        self.data.clear();
                        if b == b']' { State::Osc } else { State::Escape }
                    }
                }
                State::Ignore => match b {
                    7 | 0x18 | 0x1a => State::Ground,
                    0x1b => State::IgnoreEscape,
                    _ => State::Ignore,
                },
                State::String => match b {
                    0x1b => State::StringEscape,
                    0x18 | 0x1a => State::Ground,
                    _ => State::String,
                },
                State::StringEscape => {
                    if b == b'\\' {
                        State::Ground
                    } else {
                        State::String
                    }
                }
                State::IgnoreEscape => {
                    if b == b'\\' {
                        State::Ground
                    } else {
                        State::Ignore
                    }
                }
            };
        }
    }
    fn finish(&mut self) {
        let Some(uri) = self.data.strip_prefix(b"7;") else {
            return;
        };
        self.path = None;
        let Some(uri) = uri.strip_prefix(b"file://") else {
            return;
        };
        let Some(slash) = uri.iter().position(|&b| b == b'/') else {
            return;
        };
        if !is_local_host(&uri[..slash]) {
            return;
        }
        let mut bytes = Vec::new();
        let mut i = slash;
        while i < uri.len() {
            if uri[i] == b'%' {
                if i + 2 >= uri.len() {
                    return;
                }
                let Some(a) = (uri[i + 1] as char).to_digit(16) else {
                    return;
                };
                let Some(b) = (uri[i + 2] as char).to_digit(16) else {
                    return;
                };
                bytes.push((a * 16 + b) as u8);
                i += 3;
            } else {
                bytes.push(uri[i]);
                i += 1;
            }
        }
        if bytes.iter().any(|b| *b < 32 || *b == 127) {
            return;
        }
        if let Ok(text) = String::from_utf8(bytes) {
            self.path = Some(PathBuf::from(text));
        }
    }
}
/// Only an empty authority, localhost, or the operating system's own hostname is local.
pub(super) fn is_local_host(host: &[u8]) -> bool {
    if host.is_empty() || host.eq_ignore_ascii_case(b"localhost") {
        return true;
    }
    #[cfg(unix)]
    {
        let mut name = [0u8; 256];
        // The buffer is valid for its full length; an unterminated or failed result is rejected.
        if unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } == 0
            && let Some(end) = name.iter().position(|b| *b == 0)
        {
            return host.eq_ignore_ascii_case(&name[..end]);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reported_tracks_prefix_invalid_reports_and_reset() {
        let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
        assert!(!screen.working_directory_reported());
        screen.advance(b"\x1b]0;title\x07");
        assert!(!screen.working_directory_reported());
        screen.advance(b"\x1b]7;");
        assert!(screen.working_directory_reported());
        screen.advance(b"file://remote.invalid/path\x07");
        assert!(screen.working_directory_reported());
        assert!(screen.working_directory().is_none());
        screen.advance(b"\x1bc");
        assert!(!screen.working_directory_reported());
    }
    #[test]
    fn chunks_and_rejections() {
        let mut d = Directory::default();
        for b in b"\x1b]7;file://localhost/tmp/a%20b\x1b\\" {
            d.advance(&[*b]);
        }
        assert_eq!(d.path(), Some(Path::new("/tmp/a b")));
        d.advance(b"\x1b]0;title\x07");
        assert_eq!(d.path(), Some(Path::new("/tmp/a b")));
        d.advance(b"\x1b]7;file://remote/tmp/b\x07");
        assert_eq!(d.path(), None);
        d.advance(b"\x1b]7;file:///tmp/evil%00\x07");
        assert_eq!(d.path(), None);
        d.advance(b"\x1bc");
        assert_eq!(d.path(), None);
    }
    #[test]
    fn ignores_nested_strings_oversized_and_malformed_sequences() {
        let mut d = Directory::default();
        d.advance(b"\x1bPpayload\x07\x1b]7;file:///evil\x07\x1b\\");
        assert_eq!(d.path(), None);
        d.advance(b"\x1b]7;file:///");
        d.advance(&vec![b'a'; 8193]);
        d.advance(b"\x07");
        assert_eq!(d.path(), None);
        d.advance(b"\x1b]7;file:///bad\x18");
        assert_eq!(d.path(), None);
        d.advance(b"\x1b]7;file:///good\x07");
        assert_eq!(d.path(), Some(Path::new("/good")));
    }
    #[test]
    #[cfg(unix)]
    fn accepts_own_hostname_but_not_a_different_host() {
        let mut name = [0u8; 256];
        assert_eq!(
            unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) },
            0
        );
        let end = name.iter().position(|b| *b == 0).unwrap();
        let host = std::str::from_utf8(&name[..end]).unwrap();
        let mut d = Directory::default();
        d.advance(format!("\x1b]7;file://{host}/local\x07").as_bytes());
        assert_eq!(d.path(), Some(Path::new("/local")));
        d.advance(b"\x1b]7;file://definitely-a-different-host.invalid/remote\x07");
        assert_eq!(d.path(), None);
    }
    #[test]
    fn remote_directory_clears_context_in_next_painted_snapshot() {
        let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
        screen.advance(b"\x1b]7;file:///local\x07relative.md");
        let before = screen.view();
        screen.advance(b"\x1b]7;file://remote.invalid/remote\x07");
        let after = screen.view();
        assert_eq!(before.working_directory(), Some(Path::new("/local")));
        assert_eq!(after.working_directory(), None);
        assert_eq!(after.local_path_at(0, 2).as_deref(), Some("relative.md"));
    }
}
