//! Where a pane's channel lives, and who is allowed to reach it.
//!
//! `PanelDir` creates a `0o700` directory. Inherited clients also verify its owner and
//! permissions before connecting: a stale identity may name a replacement planted after the
//! console closed. This pathname check does not authenticate the peer across a replacement race.

use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(not(windows))]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
#[cfg(unix)]
pub(crate) use interprocess::local_socket::{Listener, Stream, prelude::*};
#[cfg(unix)]
use interprocess::local_socket::{ListenerOptions, Name};
#[cfg(windows)]
use interprocess::local_socket::{Name, ToNsName};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::WindowsPeer;
#[cfg(windows)]
pub use windows::{Listener, Stream};

/// The channel-owned end of a private wake pair.
///
/// Dropping it makes the acceptor's end readable with a hang-up. It needs neither the endpoint's
/// pathname nor room in its connection backlog, and closing a descriptor cannot wait.
pub(crate) struct AcceptorStopper {
    #[cfg(unix)]
    _signal: UnixStream,
    #[cfg(windows)]
    _signal: windows::StopSignal,
}

/// The listener and private wake descriptor owned by one acceptor thread.
pub(crate) struct Acceptor {
    #[cfg(unix)]
    listener: Listener,
    #[cfg(unix)]
    signal: UnixStream,
    #[cfg(windows)]
    inner: windows::Acceptor,
}

/// What woke the acceptor.
pub(crate) enum AcceptorReady {
    Connection,
    Stop,
}

/// Gives one bound listener the private descriptor pair that stops it.
pub(crate) fn acceptor(listener: Listener) -> io::Result<(AcceptorStopper, Acceptor)> {
    #[cfg(windows)]
    {
        let (signal, inner) = windows::acceptor(listener)?;
        Ok((AcceptorStopper { _signal: signal }, Acceptor { inner }))
    }
    #[cfg(unix)]
    {
        let (stopper, wake) = UnixStream::pair()?;
        Ok((
            AcceptorStopper { _signal: stopper },
            Acceptor {
                listener,
                signal: wake,
            },
        ))
    }
}

impl Acceptor {
    /// Sleeps until the endpoint has a connection or its owner disappears.
    pub(crate) fn wait(&self) -> io::Result<AcceptorReady> {
        #[cfg(windows)]
        {
            // Bounded: the channel signals its private stop event when shutdown begins.
            self.inner.wait()
        }
        #[cfg(unix)]
        {
            let Listener::UdSocket(listener) = &self.listener;
            let mut descriptors = [
                libc::pollfd {
                    fd: listener.inner().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.signal.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            // SAFETY: both descriptors remain owned for the call, the array has the declared
            // length, and `poll` only writes its `revents` fields.
            let ready = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, -1) };
            if ready < 0 {
                return Err(io::Error::last_os_error());
            }
            if descriptors[1].revents != 0 {
                Ok(AcceptorReady::Stop)
            } else {
                Ok(AcceptorReady::Connection)
            }
        }
    }

    /// Accepts the connection whose readiness [`Self::wait`] reported.
    pub(crate) fn accept(&self) -> io::Result<Stream> {
        #[cfg(windows)]
        {
            self.inner.accept()
        }
        #[cfg(unix)]
        {
            self.listener.accept()
        }
    }

    #[cfg(windows)]
    pub(crate) fn wait_for_stop(&self, timeout: std::time::Duration) -> io::Result<bool> {
        self.inner.wait_for_stop(timeout)
    }
}

/// The name of a pane's endpoint.
///
/// **A name and not a path**, because on Windows there is no path: a pipe lives in the machine's
/// namespace. The two platforms differ here and only here, which is what keeps everything above
/// this function written once.
///
/// **Which of the two arguments does the separating is the platform's business too.** On POSIX the
/// socket is a file inside the pane's own directory, so the directory is what keeps two panes apart
/// and the identifier is not read at all; on Windows the name carries no directory, so the
/// identifier is the whole of it. That is why the producer refuses to start with either variable
/// missing instead of defaulting one — see [`crate::Producer::of_this_pane`].
pub fn name_of(dir: &Path, id: &str) -> io::Result<Name<'static>> {
    #[cfg(not(windows))]
    {
        let _ = id;
        dir.join(panel_contract::CONTROL_SOCKET)
            .to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        let _ = dir;
        format!("ivy-{id}").to_ns_name::<GenericNamespaced>()
    }
}

/// Whether [`bind`] is supported on this platform. Producers use this to decide whether they can
/// offer the control channel.
pub const BINDS: bool = cfg!(any(unix, windows));

/// Whether the Unix control socket currently has socket metadata at the contract path.
///
/// This is a presence hint for MCP instructions, not a liveness check: connecting here would
/// consume a listener resource and turn initialization into an endpoint probe.
#[cfg(unix)]
pub fn socket_present(dir: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;

    std::fs::symlink_metadata(dir.join(panel_contract::CONTROL_SOCKET))
        .is_ok_and(|metadata| metadata.file_type().is_socket())
}

/// Windows has no filesystem socket. This hint reports whether inherited identity and server PID
/// are complete; it does not probe the named pipe or claim that the server is reachable.
#[cfg(windows)]
pub fn socket_present(_dir: &Path) -> bool {
    std::env::var("PANEL_ID").is_ok_and(|id| !id.is_empty())
        && std::env::var("PANEL_SERVER_PID")
            .ok()
            .and_then(|pid| pid.parse::<u32>().ok())
            .is_some_and(|pid| pid != 0)
        && private_to_this_user(_dir).is_ok()
}

/// Binds a pane's endpoint, ready to accept. Windows creates a byte-mode overlapped pipe with a
/// protected user-and-SYSTEM DACL and rejects remote clients. Unix uses the pane directory's socket.
///
/// The acceptor waits on the listener and a private stop signal, so it does no periodic polling at
/// rest and does not need to open a synthetic client connection during shutdown. The Unix acceptor
/// uses `poll`; the Windows acceptor waits on overlapped connect and stop events. Both transports
/// provide bounded read and write operations to the listener (see [`crate::listener`]).
pub fn bind(dir: &Path, id: &str) -> io::Result<Listener> {
    // `BINDS` says the same thing to callers that must not promise a channel; the test
    // `binding_is_what_binds_says` ties the two together.
    #[cfg(windows)]
    {
        windows::bind(dir, id)
    }
    #[cfg(not(windows))]
    {
        ListenerOptions::new().name(name_of(dir, id)?).create_sync()
    }
}

/// Refuses an inherited directory that another user could control.
pub fn private_to_this_user(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no arguments or memory preconditions.
        private_to_uid(dir, unsafe { libc::geteuid() })
    }
    #[cfg(windows)]
    {
        windows::private_to_this_user(dir)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = dir;
        Ok(())
    }
}

#[cfg(unix)]
fn private_to_uid(dir: &Path, uid: u32) -> io::Result<()> {
    use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
    // lstat("alias/.") follows alias. Strip only terminal slash/dot components;
    // normalising interior components (especially ..) would change symlink traversal.
    let mut bytes = dir.as_os_str().as_bytes();
    loop {
        if (bytes.len() > 1 && bytes.ends_with(b"/")) || bytes.ends_with(b"/.") {
            bytes = &bytes[..bytes.len() - 1];
        } else {
            break;
        }
    }
    let lookup = Path::new(std::ffi::OsStr::from_bytes(bytes));
    let metadata = std::fs::symlink_metadata(lookup).map_err(|failure| {
        let detail = if failure.kind() == io::ErrorKind::NotFound {
            "directory is gone".to_owned()
        } else {
            failure.to_string()
        };
        io::Error::new(
            failure.kind(),
            format!(
                "this console's panel could not be reached at {} ({detail})",
                dir.display()
            ),
        )
    })?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", dir.display()),
        ));
    }
    Ok(())
}

/// Opens a connection to a pane's endpoint, from the producer's side.
/// Windows verifies the server against the separately inherited `PANEL_SERVER_PID` before returning.
pub fn connect(dir: &Path, id: &str) -> io::Result<Stream> {
    #[cfg(windows)]
    {
        connect_authenticated(dir, id, crate::client::inherited_server_pid()?)
    }
    #[cfg(unix)]
    {
        Stream::connect(name_of(dir, id)?)
    }
}

/// Connects only after the pipe server process matches the inherited app PID.
#[cfg(windows)]
pub fn connect_authenticated(dir: &Path, id: &str, expected_pid: u32) -> io::Result<Stream> {
    let stream = windows::connect(dir, id)?;
    stream.verify_server_pid(expected_pid)?;
    Ok(stream)
}

/// Opens an opt-in verified Windows stream within the caller's total operation deadline.
/// Explicit unverified negotiation keeps ordinary publications usable without granting authority.
#[cfg(windows)]
pub(crate) fn connect_verified_authenticated(
    dir: &Path,
    id: &str,
    expected_pid: u32,
    deadline: std::time::Instant,
) -> io::Result<Stream> {
    windows::connect_verified(dir, id, expected_pid, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::PanelDir;

    crate::windows_unix_omission!(
        the_endpoint_lives_inside_the_panes_own_directory,
        "unix-only: requires Unix socket path placement"
    );
    crate::windows_unix_omission!(
        what_was_bound_is_what_gets_connected_to,
        "unix-only: requires Unix stream socket connection semantics"
    );
    crate::windows_unix_omission!(
        a_pane_cannot_be_reached_through_another_panes_endpoint,
        "unix-only: requires Unix stream socket routing"
    );

    crate::windows_unix_omission!(
        inherited_directory_requires_private_mode_type_and_effective_owner,
        "unix-only: requires Unix permission bits and ownership metadata"
    );

    #[cfg(unix)]
    #[test]

    fn inherited_directory_requires_private_mode_type_and_effective_owner() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        let panel = PanelDir::create().unwrap();
        let dir = panel.path();
        private_to_this_user(dir).unwrap();
        let uid = std::fs::symlink_metadata(dir).unwrap().uid();
        assert!(private_to_uid(dir, uid ^ 1).is_err());
        for bit in [0o040, 0o020, 0o010, 0o004, 0o002, 0o001] {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700 | bit)).unwrap();
            let error = private_to_this_user(dir).unwrap_err().to_string();
            assert!(error.contains(&dir.display().to_string()), "{error}");
            assert!(
                error.contains("not a private directory of this user"),
                "{error}"
            );
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let alias = dir.join("alias");
        symlink(dir, &alias).unwrap();
        for suffix in ["", "/", "/.", "/././"] {
            let path = std::path::PathBuf::from(format!("{}{suffix}", alias.display()));
            let error = private_to_this_user(&path).unwrap_err().to_string();
            assert!(error.contains(&path.display().to_string()), "{error}");
            assert!(
                error.contains("not a private directory of this user"),
                "{error}"
            );
            private_to_this_user(Path::new(&format!("{}{suffix}", dir.display()))).unwrap();
        }
        let file = dir.join("file");
        std::fs::write(&file, "").unwrap();
        assert!(private_to_this_user(&file).is_err());
        let missing = dir.join("gone");
        let error = private_to_this_user(&missing).unwrap_err().to_string();
        assert!(error.contains("directory is gone"), "{error}");
        assert!(error.contains(&missing.display().to_string()), "{error}");
        assert!(!error.contains("not a private directory"), "{error}");
        let error = private_to_this_user(&file.join("child"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("file/child"), "{error}");
        assert!(!error.contains("not a private directory"), "{error}");
    }

    /// **The endpoint is inside the pane's directory, which is where its permission comes from.**
    /// Asserting the name alone would pass with a socket in the shared temporary — the one place it
    /// must never be, because the whole access control is the directory's mode.
    #[cfg(not(windows))]
    #[test]
    fn the_endpoint_lives_inside_the_panes_own_directory() {
        let panel = PanelDir::create().expect("create the panel directory");

        let _listener = bind(panel.path(), panel.id()).expect("bind the endpoint");

        let socket = panel.path().join(panel_contract::CONTROL_SOCKET);
        assert!(socket.exists(), "nothing was bound at {}", socket.display());
    }

    crate::windows_unix_omission!(
        nothing_outside_this_user_can_reach_the_endpoint,
        "unix-only: requires Unix socket permissions and ownership metadata"
    );
    /// The mode is the directory's and is inherited rather than set: this is what fails if somebody
    /// ever moves the endpoint out of the directory, which is the change that would quietly widen it.
    ///
    /// **The mode read is the bound socket's own parent**, and not the directory `PanelDir::create`
    /// makes: reading the latter left `bind` out of the test entirely, so moving the socket to `/tmp`
    /// while keeping the directory `0o700` stayed green against its own comment. The subject is where
    /// the socket actually landed.
    #[cfg(unix)]
    #[test]
    fn nothing_outside_this_user_can_reach_the_endpoint() {
        use std::os::unix::fs::PermissionsExt;

        let panel = PanelDir::create().expect("create the panel directory");
        let _listener = bind(panel.path(), panel.id()).expect("bind the endpoint");
        let socket = panel.path().join(panel_contract::CONTROL_SOCKET);

        let containing = socket
            .parent()
            .expect("the bound socket has a parent")
            .metadata()
            .expect("read the containing directory's permissions")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(containing, 0o700, "the endpoint's door is {containing:o}");
    }

    #[cfg(not(windows))]
    #[test]
    fn what_was_bound_is_what_gets_connected_to() {
        let panel = PanelDir::create().expect("create the panel directory");
        let listener = bind(panel.path(), panel.id()).expect("bind the endpoint");

        // **The client connects first and the accept is a single blocking call.** The earlier loop
        // retried on `WouldBlock` without any deadline, so a regression that stopped accepting hung
        // the test instead of failing it.
        let client = connect(panel.path(), panel.id()).expect("connect to the endpoint");
        let accepted = listener.accept().expect("accept the connection");

        drop(client);
        drop(accepted);
    }

    /// **`BINDS` is what `bind` does, on whatever platform this runs.** The constant is the one a
    /// producer reads to decide whether to promise a channel; a `bind` that started working on
    /// Windows with the constant left false would keep agents there without instructions.
    #[test]
    fn binding_is_what_binds_says() {
        let panel = PanelDir::create().expect("create the panel directory");

        let bound = bind(panel.path(), panel.id());
        let refusal = bound.as_ref().err().map(ToString::to_string);
        assert_eq!(
            bound.is_ok(),
            BINDS,
            "BINDS disagrees with binding: {refusal:?}"
        );
        if let Err(refusal) = bound {
            assert_eq!(refusal.kind(), io::ErrorKind::Unsupported, "{refusal}");
        }
    }

    /// Two panes are two channels, asserted by **connecting** rather than by comparing names: a
    /// name that differed while both resolved to one endpoint would pass a comparison and break the
    /// scoping the whole design rests on — «the pane is the endpoint you opened».
    #[cfg(not(windows))]
    #[test]
    fn a_pane_cannot_be_reached_through_another_panes_endpoint() {
        let first = PanelDir::create().expect("create the first");
        let second = PanelDir::create().expect("create the second");
        let _bound = bind(first.path(), first.id()).expect("bind the first");

        let wrong_door = connect(second.path(), second.id());

        assert!(
            wrong_door.is_err(),
            "the second pane's name reached the first pane's endpoint"
        );
    }
}
