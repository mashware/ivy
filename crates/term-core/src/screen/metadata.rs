//! Bounded OSC metadata, independent of terminal painting.
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Metadata {
    state: State,
    data: Vec<u8>,
    path: Option<PathBuf>,
    reported: bool,
    generation: u64,
    pub(super) program: Option<super::ProgramSnapshot>,
    pub(super) program_generation: u64,
    pub(super) query: bool,
    pub(super) enabled_claude: bool,
    pub(super) enabled_codex: bool,
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
impl Metadata {
    pub(super) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub(super) fn reported(&self) -> bool {
        self.reported
    }
    pub(super) fn generation(&self) -> u64 {
        self.generation
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
                        self.program = None;
                        self.query = false;
                        self.program_generation = self.program_generation.wrapping_add(1);
                        self.path = None;
                        self.reported = false;
                        self.generation = self.generation.wrapping_add(1);
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
        if let Some(payload) = self.data.strip_prefix(b"7501;") {
            if payload == b"?" {
                self.query = true;
            } else if let Some(value) = parse_program(payload) {
                if let Some(snapshot) = value {
                    let enabled = match snapshot.client {
                        super::ProgramClient::Claude => self.enabled_claude,
                        super::ProgramClient::Codex => self.enabled_codex,
                    };
                    if !enabled {
                        return;
                    }
                }
                self.program = value;
                self.program_generation = self.program_generation.wrapping_add(1);
            }
            return;
        }
        let Some(uri) = self.data.strip_prefix(b"7;") else {
            return;
        };
        self.generation = self.generation.wrapping_add(1);
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

fn parse_program(payload: &[u8]) -> Option<Option<super::ProgramSnapshot>> {
    use super::{
        ProgramClient as Client, ProgramKind as Kind, ProgramSnapshot, ProgramState as State,
    };
    let mut state = None;
    let mut client = None;
    let mut kind = None;
    for field in payload.split(|b| *b == b':') {
        let at = field.iter().position(|b| *b == b'=')?;
        let (key, value) = (&field[..at], &field[at + 1..]);
        if value.is_empty() {
            return None;
        }
        match key {
            b"state" if state.is_none() => {
                state = Some(match value {
                    b"clear" => None,
                    b"idle" => Some(State::Idle),
                    b"working" => Some(State::Working),
                    b"blocked" => Some(State::Blocked),
                    b"done" => Some(State::Done),
                    b"error" => Some(State::Error),
                    _ => return None,
                })
            }
            b"app" if client.is_none() => {
                client = Some(match value {
                    b"claude-code" => Client::Claude,
                    b"codex" => Client::Codex,
                    _ => return None,
                })
            }
            b"kind" if kind.is_none() => {
                kind = Some(match value {
                    b"permission" => Kind::Permission,
                    b"question" => Kind::Question,
                    b"auth" => Kind::Auth,
                    _ => return None,
                })
            }
            // Child declarations are never root declarations, even when clearing.
            b"id" => return None,
            // Optional details are opaque and bounded by the scanner; never displayed or retained.
            b"msg" | b"title" | b"progress" => {}
            _ => return None,
        }
    }
    match state? {
        None if client.is_none() && kind.is_none() => Some(None),
        Some(state) if kind.is_none() || state == State::Blocked => Some(Some(ProgramSnapshot {
            client: client?,
            state,
            kind,
        })),
        _ => None,
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
    // Windows supplies its local computer name in the inherited session environment. This
    // is locality metadata, not authentication; the same session already selects the shell.
    #[cfg(windows)]
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        return !name.is_empty() && host.eq_ignore_ascii_case(name.as_bytes());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_snapshots_survive_every_chunk_boundary_and_replace_wait() {
        use super::super::{ProgramClient, ProgramKind, ProgramState};
        let frame = b"\x1b]7501;state=blocked:app=claude-code:kind=question\x1b\\";
        for cut in 0..=frame.len() {
            let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
            screen.set_program_clients(true, true);
            screen.advance(&frame[..cut]);
            screen.advance(&frame[cut..]);
            let snapshot = screen.program_snapshot().unwrap();
            assert_eq!(
                (snapshot.client, snapshot.state, snapshot.kind),
                (
                    ProgramClient::Claude,
                    ProgramState::Blocked,
                    Some(ProgramKind::Question)
                )
            );
            screen.advance(b"\x1b]7501;state=idle:app=codex\x07");
            assert_eq!(screen.program_snapshot().unwrap().state, ProgramState::Idle);
            assert_eq!(screen.program_generation(), 2);
            screen.advance(b"\x1b]7501;state=clear\x07");
            assert_eq!(screen.program_snapshot(), None);
        }
    }

    #[test]
    fn native_rejects_children_invalid_strings_and_coalesces_queries() {
        let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
        screen.set_program_clients(true, true);
        screen.advance(b"\x1b]7501;state=working:app=codex\x07");
        let before = screen.program_snapshot();
        for invalid in [
            b"\x1b]7501;state=clear:id=child\x07".as_slice(),
            b"\x1b]7501;state=blocked:app=codex:kind=unknown\x07",
            b"\x1b]7501;state=working:app=unknown\x07",
            b"\x1bP\x1b]7501;state=clear\x07\x1b\\",
            b"\x1b]7501;state=clear\x18",
        ] {
            screen.advance(invalid);
            assert_eq!(screen.program_snapshot(), before);
        }
        screen.advance(b"\x1b]7501;?\x07\x1b]7501;?\x1b\\");
        assert!(screen.take_program_query());
        assert!(!screen.take_program_query());
        screen.advance(b"\x1bc");
        assert_eq!(screen.program_snapshot(), None);
        assert_eq!(screen.program_generation(), 2);
    }

    #[test]
    fn disabled_sources_do_not_replace_enabled_sources_in_the_same_batch() {
        let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
        screen.advance(b"\x1b]7501;state=working:app=codex\x07");
        assert!(screen.program_snapshot().is_none());
        screen.set_program_clients(false, true);
        screen.advance(b"\x1b]7501;state=working:app=codex\x07\x1b]7501;state=blocked:app=claude-code:kind=question\x07");
        assert_eq!(
            screen.program_snapshot().unwrap().client,
            super::super::ProgramClient::Codex
        );
        let generation = screen.program_generation();
        screen.set_program_clients(false, false);
        assert!(screen.program_snapshot().is_none());
        assert_eq!(screen.program_generation(), generation + 1);
    }

    #[test]
    fn repeated_directory_reports_and_resets_advance_the_observation() {
        let mut screen = crate::Screen::new(crate::Size { rows: 2, cols: 30 });
        assert_eq!(screen.working_directory_generation(), 0);
        for expected in 1..=2 {
            screen.advance(b"\x1b]7;file://localhost/tmp\x07");
            assert_eq!(screen.working_directory_generation(), expected);
            assert_eq!(
                screen.working_directory(),
                Some(std::path::Path::new("/tmp"))
            );
        }
        screen.advance(b"\x1bc");
        assert_eq!(screen.working_directory_generation(), 3);
        assert!(screen.working_directory().is_none());
        screen.advance(b"\x1b]7;file://remote.invalid/tmp\x07");
        assert_eq!(screen.working_directory_generation(), 4);
        assert!(screen.working_directory().is_none());
    }

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
        let mut d = Metadata::default();
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
        let mut d = Metadata::default();
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
        let mut d = Metadata::default();
        d.advance(format!("\x1b]7;file://{host}/local\x07").as_bytes());
        assert_eq!(d.path(), Some(Path::new("/local")));
        d.advance(b"\x1b]7;file://definitely-a-different-host.invalid/remote\x07");
        assert_eq!(d.path(), None);
    }
    #[test]
    #[cfg(windows)]
    fn accepts_windows_session_hostname_but_not_a_different_host() {
        let name = std::env::var("COMPUTERNAME").expect("Windows session computer name");
        assert!(!name.is_empty());
        let mut d = Metadata::default();
        d.advance(format!("\x1b]7;file://{name}/C:/work\x07").as_bytes());
        assert_eq!(d.path(), Some(Path::new("/C:/work")));
        d.advance(b"\x1b]7;file://different-host.invalid/C:/work\x07");
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
