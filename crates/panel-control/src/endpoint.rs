//! Where a pane's channel lives, and who is allowed to reach it.
//!
//! **The access control is the directory's permission and nothing else.** `PanelDir` creates its
//! directory `0o700` and the socket is born inside it, so a process of another user cannot reach
//! the endpoint without the directory's mode having already failed — at which point a check of our
//! own would be bypassed by the same failure. That is the whole of the POSIX story, and it is
//! deliberately not supplemented.

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
use interprocess::local_socket::{Listener, Stream, prelude::*};
use interprocess::local_socket::{ListenerOptions, Name};

/// The channel-owned end of a private wake pair.
///
/// Dropping it makes the acceptor's end readable with a hang-up. It needs neither the endpoint's
/// pathname nor room in its connection backlog, and closing a descriptor cannot wait.
pub(crate) struct AcceptorStopper {
    #[cfg(unix)]
    _signal: UnixStream,
}

/// The listener and private wake descriptor owned by one acceptor thread.
pub(crate) struct Acceptor {
    listener: Listener,
    #[cfg(unix)]
    signal: UnixStream,
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
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the control channel has no Windows listener until F2.7",
        ))
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
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "the control channel has no Windows listener until F2.7",
            ))
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
        self.listener.accept()
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

/// Whether [`bind`] binds on this platform: a producer that has to decide whether to promise a
/// channel — the MCP bridge's handshake, and `Producer::of_this_pane`, which refuses before
/// connecting — reads it here instead of restating the platform list.
/// The day F2.7 binds on Windows this has to change with it, and `binding_is_what_binds_says`
/// fails there until it does.
pub const BINDS: bool = cfg!(not(windows));

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

/// Windows has no filesystem socket until its control channel is implemented.
#[cfg(windows)]
pub fn socket_present(_dir: &Path) -> bool {
    false
}

/// Binds a pane's endpoint, ready to accept.
///
/// **It does not bind on Windows, and that is a decision rather than an omission.** A named pipe
/// gets a default security descriptor that lets any process on the machine open it, so binding one
/// without an explicit DACL would turn a channel whose whole access control is a `0o700` directory
/// into a machine-wide one — silently, and on the one platform where a non-local caller can exist
/// at all. The delivery that brings this crate cannot compile or run a line of Windows code, so the
/// descriptor cannot be asserted; the rule it was given is that what cannot be proven is not bound.
/// **F2.7 is the milestone that compiles here for the first time**, and it is where the pipe, its
/// descriptor and the assertion over both arrive together. Until then a Windows pane has no channel
/// and publishes by file, exactly as every pane did before this milestone.
///
/// **The listener blocks in `accept`, and that is the shape that costs nothing at rest.** An
/// earlier version made the accept non-blocking so the acceptor could poll its stop flag, which cost
/// every pane a thread waking five times a second for the life of the window — four idle panes
/// alone were about twenty wake-ups a second, and on a laptop that is what shows up as battery. The
/// acceptor parks in `poll` over the listener and a private wake descriptor; dropping the other end
/// wakes it without polling at rest or opening a client connection. The streams it hands out are
/// blocking, which is what lets a receive timeout be put on them (see [`crate::listener`]).
pub fn bind(dir: &Path, id: &str) -> io::Result<Listener> {
    // `BINDS` says the same thing to callers that must not promise a channel; the test
    // `binding_is_what_binds_says` ties the two together.
    #[cfg(windows)]
    {
        let _ = (dir, id);
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the control channel is not bound on Windows until its security descriptor can be \
             asserted",
        ));
    }
    #[cfg(not(windows))]
    {
        ListenerOptions::new().name(name_of(dir, id)?).create_sync()
    }
}

/// Opens a connection to a pane's endpoint, from the producer's side.
pub fn connect(dir: &Path, id: &str) -> io::Result<Stream> {
    Stream::connect(name_of(dir, id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::PanelDir;

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
