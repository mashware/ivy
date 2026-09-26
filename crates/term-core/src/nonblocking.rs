//! Waiting on the pseudoterminal **outside** the system call that reads or writes it.
//!
//! **On Linux the master is non-blocking, and that amends what D29 decided.** D29 kept it blocking
//! because reader and writer are duplicates of one open file description, so the flag lands on both,
//! and the reader's blocking `read` is the output chain. What changed is a measurement: on Linux a
//! blocking write to a master whose buffer toward the program is full **does not end when the program
//! dies** — the slave gone, a read answering `EIO`, the write still waiting seconds later, and nothing
//! a closing process can do wakes it. So the writer thread outlived every console closed in that
//! state. With the flag on, neither side sleeps inside the read or the write call: each one waits
//! here, in a `poll` that also watches what should end the wait. The reader waits for what its
//! blocking `read` waited for, so the brake of D8 — the reader not reading while the channel is full —
//! is untouched.
//!
//! **Linux only, because it is the only platform measured**, and because the one other Unix this
//! project targets is known to break it: macOS's `poll` does not wait on a pseudoterminal, which is why
//! the filedescriptor crate beneath `portable-pty` uses `select` there. F2.7 decides what macOS does.

use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

/// Turns the master's open file description non-blocking, and with it every duplicate of it.
pub(crate) fn set_nonblocking(master: RawFd) -> io::Result<()> {
    // SAFETY: `fcntl` with `F_GETFL`/`F_SETFL` takes plain integers and touches no memory.
    let flags = unsafe { libc::fcntl(master, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: as above.
    if unsafe { libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// A descriptor of the master's own, for a thread to wait on for as long as it lives.
///
/// **Its own and not a borrowed number**: a thread abandoned at closing (D32) may outlive the `Pty`
/// that holds the master, and a number it merely remembered could by then name somebody else's file.
pub(crate) fn duplicate(master: RawFd) -> io::Result<OwnedFd> {
    // SAFETY: the caller hands over the master's descriptor while the master is alive, and the
    // borrow ends inside this call.
    unsafe { BorrowedFd::borrow_raw(master) }.try_clone_to_owned()
}

/// What waiting for room answered.
pub(crate) enum Room {
    /// There is room: write again.
    Ready,
    /// Whoever owns the console is closing it.
    Closing,
    /// The program's side of the pseudoterminal is closed, so the room will never come.
    Gone,
}

/// What the writer thread waits on: the master, and the far end of a pipe the console holds.
///
/// **The pipe is the trigger, and it is dropped, not written.** A flag is only read by a thread that
/// is looking at it, and a thread inside `poll` is not; closing the console drops the pipe's writing
/// end, which makes this end readable — at end of file — and returns the `poll` at once.
///
/// **`master` has to be a duplicate of the descriptor the writer writes to**, so that «would block»
/// there and room here speak of one open file description.
pub(crate) struct Writable {
    pub(crate) master: OwnedFd,
    pub(crate) knocked: io::PipeReader,
}

impl Writable {
    /// Waits until the master has room, the console is closing, or the program's side is gone.
    pub(crate) fn wait(&self) -> io::Result<Room> {
        let mut watched = [
            poll_for(self.master.as_raw_fd(), libc::POLLOUT),
            poll_for(self.knocked.as_fd().as_raw_fd(), libc::POLLIN),
        ];
        // Bounded: by room, by the console closing — the drop of the pipe's writing end — or by the
        // program's side of the pseudoterminal closing, which `poll` reports as a hang-up.
        wait_on(&mut watched)?;
        let [master, knocked] = watched;
        if knocked.revents != 0 {
            Ok(Room::Closing)
        } else if master.revents & libc::POLLOUT != 0 {
            Ok(Room::Ready)
        } else if master.revents & libc::POLLHUP != 0 {
            // Measured: a master whose program has gone answers a hang-up and never has room again.
            Ok(Room::Gone)
        } else {
            Err(unwaitable(master.revents))
        }
    }
}

/// The reader `portable-pty` builds over the master, made to wait instead of answering «would block».
///
/// **It waits for what the blocking `read` it replaces waited for**: data, or the end of the program's
/// side. So a survivor holding that side parks it exactly as it parked the blocking read, and D32's
/// abandonment is what bounds that, as before.
pub(crate) struct Readable<R> {
    pub(crate) reader: R,
    pub(crate) master: OwnedFd,
}

impl<R: Read> Read for Readable<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.reader.read(buf) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let mut watched = [poll_for(self.master.as_raw_fd(), libc::POLLIN)];
                    // Bounded: as the blocking read before it — by data, or by the program's side
                    // closing. A survivor holding that side is D32's case, bounded by abandonment.
                    wait_on(&mut watched)?;
                    // **A wake with neither data nor a hang-up is failed, not retried**: nothing says
                    // the next read makes progress, and if it answered «would block» again this loop
                    // would spin for the life of the console.
                    if watched[0].revents & (libc::POLLIN | libc::POLLHUP) == 0 {
                        return Err(unwaitable(watched[0].revents));
                    }
                }
                answer => return answer,
            }
        }
    }
}

/// What a `poll` that woke without the event it waited for, and without a hang-up, is reported as.
fn unwaitable(revents: libc::c_short) -> io::Error {
    io::Error::other(format!(
        "the pseudoterminal cannot be waited on (poll answered {revents:#x})"
    ))
}

fn poll_for(fd: RawFd, events: libc::c_short) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

/// `poll` with no deadline, again after a signal: an interrupted wait has not seen anything happen.
fn wait_on(watched: &mut [libc::pollfd]) -> io::Result<()> {
    loop {
        // SAFETY: `watched` is a valid, exclusively borrowed array of `pollfd` of the length given.
        let ready = unsafe { libc::poll(watched.as_mut_ptr(), watched.len() as libc::nfds_t, -1) };
        if ready >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A source that never has anything and always says so.
    struct AlwaysWouldBlock;

    impl Read for AlwaysWouldBlock {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::WouldBlock.into())
        }
    }

    /// **A wake that is neither data nor a hang-up ends the read with an error.** The descriptor
    /// waited on is the writing end of a pipe whose reading end is gone: `poll` answers it with an
    /// error flag at once, and the source keeps answering «would block». Retrying would spin for
    /// ever, so the read runs on a thread of its own and is given a deadline.
    #[test]
    fn a_wake_with_neither_data_nor_a_hang_up_fails_instead_of_spinning() {
        let (far_end, near_end) = io::pipe().expect("a pipe");
        drop(far_end);
        let mut readable = Readable {
            reader: AlwaysWouldBlock,
            master: OwnedFd::from(near_end),
        };
        let (answer, answered) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = answer.send(readable.read(&mut [0u8; 8]).map_err(|e| e.kind()));
        });
        match answered.recv_timeout(Duration::from_secs(2)) {
            Ok(Err(_)) => {}
            Ok(Ok(n)) => panic!("the read answered {n} bytes from a source that has none"),
            Err(_) => panic!("the read was still going after two seconds: it is spinning"),
        }
    }
}
