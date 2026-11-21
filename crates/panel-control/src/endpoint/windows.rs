//! Native, overlapped named-pipe transport.
//!
//! Every operation keeps its `OVERLAPPED` and event alive through cancellation completion. The
//! only waits here are kernel waits; the caller's receive/send deadlines bound them.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::IO::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::Threading::*;

const PIPE_PATH_PREFIX: &str = r"\\.\pipe\";
const PIPE_MODE: NAMED_PIPE_MODE =
    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
const PIPE_OPEN: FILE_FLAGS_AND_ATTRIBUTES = FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE;
const PIPE_OPEN_NEXT: FILE_FLAGS_AND_ATTRIBUTES = FILE_FLAG_OVERLAPPED;

fn winerr() -> io::Error {
    io::Error::last_os_error()
}
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

struct Handle(HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Handle {
    fn new(handle: HANDLE) -> io::Result<Self> {
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            Err(winerr())
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Security(*mut std::ffi::c_void);
impl Security {
    fn pipe() -> io::Result<Self> {
        descriptor(false)
    }
}
impl Drop for Security {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn descriptor(children: bool) -> io::Result<Security> {
    let sid = current_sid_string()?;
    let inherit = if children { "OICI" } else { "" };
    let system_ace = if sid == "S-1-5-18" {
        String::new()
    } else {
        format!("(A;{inherit};FA;;;SY)")
    };
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;{inherit};FA;;;{sid}){system_ace}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut raw = null_mut();
    let mut size = 0;
    // SAFETY: the SDDL is terminated and output pointers are valid.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut raw,
            &mut size,
        )
    } == 0
    {
        return Err(winerr());
    }
    Ok(Security(raw))
}

fn current_sid_string() -> io::Result<String> {
    let mut token = null_mut();
    // SAFETY: process pseudo-handle and output pointer are valid.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(winerr());
    }
    let token = Handle(token);
    let mut needed = 0;
    // SAFETY: probing the required buffer size is the documented call pattern.
    unsafe {
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed);
    }
    let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: buffer has the reported size and is suitably aligned for TOKEN_USER.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(winerr());
    }
    let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    let mut sid_string = null_mut();
    // SAFETY: token information owns a valid SID for the duration of this call.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_string) } == 0 {
        return Err(winerr());
    }
    let mut len = 0;
    unsafe {
        while *sid_string.add(len) != 0 {
            len += 1;
        }
    }
    let result = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid_string, len) });
    unsafe {
        LocalFree(sid_string.cast());
    }
    Ok(result)
}

struct ListenerInner {
    name: Vec<u16>,
    security: Security,
    handle: Option<Handle>,
    first: bool,
}
pub struct Listener {
    inner: Arc<Mutex<ListenerInner>>,
}
unsafe impl Send for Listener {}
unsafe impl Sync for Listener {}

fn new_pipe(name: &[u16], security: &Security, first: bool) -> io::Result<Handle> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    // SAFETY: all arguments point to live data, descriptor is explicit, and the resulting handle is overlapped.
    Handle::new(unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | if first { PIPE_OPEN } else { PIPE_OPEN_NEXT },
            PIPE_MODE,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            &attributes,
        )
    })
}

pub fn bind(_dir: &Path, id: &str) -> io::Result<Listener> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid pane identifier",
        ));
    }
    let security = Security::pipe()?;
    let name: Vec<u16> = format!("{PIPE_PATH_PREFIX}ivy-{id}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let handle = new_pipe(&name, &security, true)?;
    Ok(Listener {
        inner: Arc::new(Mutex::new(ListenerInner {
            name,
            security,
            handle: Some(handle),
            first: false,
        })),
    })
}

pub struct StopSignal {
    handle: Arc<Handle>,
    armed: bool,
}
pub struct Acceptor {
    listener: Arc<Mutex<ListenerInner>>,
    stop: Arc<Handle>,
    pending: Mutex<Option<(HANDLE, Box<OVERLAPPED>, Handle)>>,
    pending_ready: AtomicBool,
    connected: Mutex<bool>,
}
unsafe impl Send for Acceptor {}
unsafe impl Sync for Acceptor {}

pub fn acceptor(listener: Listener) -> io::Result<(StopSignal, Acceptor)> {
    let stop = Arc::new(event()?);
    let acceptor = Acceptor {
        listener: Arc::clone(&listener.inner),
        stop: Arc::clone(&stop),
        pending: Mutex::new(None),
        pending_ready: AtomicBool::new(false),
        connected: Mutex::new(false),
    };
    Ok((
        StopSignal {
            handle: stop,
            armed: true,
        },
        acceptor,
    ))
}

fn event() -> io::Result<Handle> {
    // SAFETY: manual reset event, initially nonsignaled, and non-inheritable.
    Handle::new(unsafe { CreateEventW(null(), 1, 0, null()) })
}

impl Acceptor {
    pub fn wait_for_stop(&self, timeout: Duration) -> io::Result<bool> {
        let millis = timeout.as_millis().min((u32::MAX - 1) as u128) as u32;
        match unsafe { WaitForSingleObject(self.stop.0, millis) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(winerr()),
        }
    }

    pub fn wait(&self) -> io::Result<super::AcceptorReady> {
        if *self.connected.lock().unwrap() {
            return Ok(super::AcceptorReady::Connection);
        }
        let mut pending = self.pending.lock().unwrap();
        if pending.is_none() {
            let listener = self.listener.lock().unwrap();
            let pipe = listener
                .handle
                .as_ref()
                .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "listener stopped"))?;
            let ev = event()?;
            let mut ov: Box<OVERLAPPED> = Box::new(unsafe { std::mem::zeroed() });
            ov.hEvent = ev.0;
            // SAFETY: OVERLAPPED and event stay in `pending` until completion is drained.
            let ok = unsafe { ConnectNamedPipe(pipe.0, &mut *ov) };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                if code == ERROR_PIPE_CONNECTED || code == ERROR_NO_DATA {
                    *self.connected.lock().unwrap() = true;
                    return Ok(super::AcceptorReady::Connection);
                }
                if code != ERROR_IO_PENDING {
                    return Err(io::Error::from_raw_os_error(code as i32));
                }
            }
            *pending = Some((pipe.0, ov, ev));
            self.pending_ready.store(true, Ordering::Release);
        }
        let (_, _ov, ev) = pending.as_ref().unwrap();
        // Stop is first so a simultaneous connect completion is treated as cancellation.
        let handles = [self.stop.0, ev.0];
        // SAFETY: both handles and the overlapped operation remain owned throughout the wait.
        let result = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
        if result == WAIT_OBJECT_0 {
            let (pipe, ov, _) = pending.take().unwrap();
            self.pending_ready.store(false, Ordering::Release);
            unsafe {
                CancelIoEx(pipe, &*ov);
            }
            let mut transferred = 0;
            unsafe {
                GetOverlappedResult(pipe, &*ov, &mut transferred, 1);
            }
            return Ok(super::AcceptorReady::Stop);
        }
        if result != WAIT_OBJECT_0 + 1 {
            return Err(winerr());
        }
        let (pipe, ov, _) = pending.take().unwrap();
        self.pending_ready.store(false, Ordering::Release);
        let mut transferred = 0;
        if unsafe { GetOverlappedResult(pipe, &*ov, &mut transferred, 0) } == 0 {
            if unsafe { GetLastError() } == ERROR_NO_DATA {
                *self.connected.lock().unwrap() = true;
                return Ok(super::AcceptorReady::Connection);
            }
            return Err(winerr());
        }
        *self.connected.lock().unwrap() = true;
        Ok(super::AcceptorReady::Connection)
    }

    pub fn accept(&self) -> io::Result<Stream> {
        if !std::mem::replace(&mut *self.connected.lock().unwrap(), false) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "no pipe connection is ready",
            ));
        }
        let mut listener = self.listener.lock().unwrap();
        let pipe = listener
            .handle
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "listener stopped"))?;
        // Keep the pipe name owned while preparing its successor instance.
        listener.handle = Some(new_pipe(
            &listener.name,
            &listener.security,
            listener.first,
        )?);
        listener.first = false;
        Ok(Stream {
            handle: pipe,
            stop: Arc::clone(&self.stop),
            tracker: Arc::new(IoTracker::default()),
            read_timeout: Mutex::new(None),
            write_timeout: Mutex::new(None),
        })
    }
}
impl Drop for Acceptor {
    fn drop(&mut self) {
        if let Ok(pending) = self.pending.get_mut() {
            if let Some((pipe, overlapped, _event)) = pending.take() {
                self.pending_ready.store(false, Ordering::Release);
                let mut transferred = 0;
                unsafe {
                    CancelIoEx(pipe, &*overlapped);
                    GetOverlappedResult(pipe, &*overlapped, &mut transferred, 1);
                }
            }
        }
    }
}

impl Drop for StopSignal {
    fn drop(&mut self) {
        if self.armed {
            unsafe {
                SetEvent(self.handle.0);
            }
        }
    }
}

impl Listener {
    pub fn accept(&self) -> io::Result<Stream> {
        let (mut stop, acceptor) = acceptor(self.clone())?;
        // Abandoned on purpose: this public blocking accept waits for a client. Channel workers
        // use their separately owned, cancellation-aware Acceptor instead of this convenience API.
        match acceptor.wait()? {
            super::AcceptorReady::Connection => {
                let stream = acceptor.accept()?;
                stop.armed = false;
                Ok(stream)
            }
            super::AcceptorReady::Stop => Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "listener stopped",
            )),
        }
    }
}
impl Clone for Listener {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub struct Stream {
    handle: Handle,
    stop: Arc<Handle>,
    tracker: Arc<IoTracker>,
    read_timeout: Mutex<Option<Duration>>,
    write_timeout: Mutex<Option<Duration>>,
}
unsafe impl Send for Stream {}
unsafe impl Sync for Stream {}
#[derive(Default)]
struct IoTracker {
    in_flight: AtomicUsize,
    pending: AtomicUsize,
}
struct IoGuard(Arc<IoTracker>, bool);
impl IoGuard {
    fn begin(tracker: &Arc<IoTracker>) -> Self {
        tracker.in_flight.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(tracker), false)
    }
    fn mark_pending(&mut self) {
        self.0.pending.fetch_add(1, Ordering::AcqRel);
        self.1 = true;
    }
}
impl Drop for IoGuard {
    fn drop(&mut self) {
        if self.1 {
            self.0.pending.fetch_sub(1, Ordering::AcqRel);
        }
        self.0.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Stream {
    pub fn verify_server_pid(&self, expected: u32) -> io::Result<()> {
        if expected == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "expected server PID is missing",
            ));
        }
        let mut actual = 0;
        if unsafe { GetNamedPipeServerProcessId(self.handle.0, &mut actual) } == 0 {
            return Err(winerr());
        }
        if actual != expected {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("control pipe server PID {actual} did not match expected PID {expected}"),
            ));
        }
        Ok(())
    }
    pub fn try_clone(&self) -> io::Result<Self> {
        let mut duplicate = null_mut();
        // SAFETY: duplicate refers to an independently closed duplicate of the current-process handle.
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.handle.0,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            return Err(winerr());
        }
        Ok(Self {
            handle: Handle(duplicate),
            stop: Arc::clone(&self.stop),
            tracker: Arc::clone(&self.tracker),
            read_timeout: Mutex::new(*self.read_timeout.lock().unwrap()),
            write_timeout: Mutex::new(*self.write_timeout.lock().unwrap()),
        })
    }
    pub fn set_recv_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        *self.read_timeout.lock().unwrap() = timeout;
        Ok(())
    }
    pub fn set_send_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        *self.write_timeout.lock().unwrap() = timeout;
        Ok(())
    }
    pub fn raw_handle(&self) -> HANDLE {
        self.handle.0
    }
}

fn transfer(
    handle: HANDLE,
    stop: HANDLE,
    tracker: &Arc<IoTracker>,
    buffer: *mut u8,
    len: u32,
    timeout: Option<Duration>,
    write: bool,
) -> io::Result<usize> {
    let mut operation = IoGuard::begin(tracker);
    // The stop event is manual-reset, so once it is set every later operation is refused here
    // instead of being issued and cancelled again.
    if unsafe { WaitForSingleObject(stop, 0) } == WAIT_OBJECT_0 {
        return Err(stopped());
    }
    let ev = event()?;
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.hEvent = ev.0;
    let mut transferred = 0;
    let ok = unsafe {
        if write {
            WriteFile(handle, buffer, len, &mut transferred, &mut ov)
        } else {
            ReadFile(handle, buffer, len, &mut transferred, &mut ov)
        }
    };
    if ok != 0 {
        return Ok(transferred as usize);
    }
    let code = unsafe { GetLastError() };
    if code != ERROR_IO_PENDING {
        if !write && code == ERROR_BROKEN_PIPE {
            return Ok(0);
        }
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    operation.mark_pending();
    let millis = timeout
        .map(|d| d.as_millis().min((u32::MAX - 1) as u128) as u32)
        .unwrap_or(INFINITE);
    let handles = [stop, ev.0];
    let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, millis) };
    if wait != WAIT_OBJECT_0 + 1 {
        unsafe {
            CancelIoEx(handle, &ov);
            GetOverlappedResult(handle, &ov, &mut transferred, 1);
        }
        return Err(if wait == WAIT_OBJECT_0 {
            stopped()
        } else if wait == WAIT_TIMEOUT {
            // `WouldBlock`, the kind std gives a Unix socket whose timeout expired: the channel's
            // read loop treats exactly that kind as "come back and look at the stop flag". Any
            // other kind there ends the connection, so an idle client would be dropped.
            io::Error::new(io::ErrorKind::WouldBlock, "pipe operation timed out")
        } else {
            winerr()
        });
    }
    if unsafe { GetOverlappedResult(handle, &ov, &mut transferred, 0) } == 0 {
        let code = unsafe { GetLastError() };
        if !write && code == ERROR_BROKEN_PIPE {
            return Ok(0);
        }
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    Ok(transferred as usize)
}
/// Not `Interrupted`: std's `read_exact` and `write_all` retry that kind, and with the stop event
/// left set the retry is refused again at once, so a stopped transfer would spin forever.
fn stopped() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, "pane channel stopped")
}
impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
}
impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Read for &Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        transfer(
            self.handle.0,
            self.stop.0,
            &self.tracker,
            buf.as_mut_ptr(),
            buf.len().min(u32::MAX as usize) as u32,
            *self.read_timeout.lock().unwrap(),
            false,
        )
    }
}
impl Write for &Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        transfer(
            self.handle.0,
            self.stop.0,
            &self.tracker,
            buf.as_ptr() as *mut u8,
            buf.len().min(u32::MAX as usize) as u32,
            *self.write_timeout.lock().unwrap(),
            true,
        )
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn connect(_dir: &Path, id: &str) -> io::Result<Stream> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid pane identifier",
        ));
    }
    let name = wide(OsStr::new(&format!("{PIPE_PATH_PREFIX}ivy-{id}")));
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            null_mut(),
        )
    };
    Ok(Stream {
        handle: Handle::new(handle)?,
        stop: Arc::new(event()?),
        tracker: Arc::new(IoTracker::default()),
        read_timeout: Mutex::new(None),
        write_timeout: Mutex::new(None),
    })
}

pub fn private_to_this_user(_dir: &Path) -> io::Result<()> {
    panel_contract::windows_directory::validate(_dir).map_err(|failure| {
        if failure.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                failure.kind(),
                format!(
                    "this console's panel could not be reached at {} (directory is gone)",
                    _dir.display()
                ),
            )
        } else if failure.kind() == io::ErrorKind::PermissionDenied {
            io::Error::new(
                failure.kind(),
                format!("{} is not a private directory of this user", _dir.display()),
            )
        } else {
            io::Error::new(
                failure.kind(),
                format!(
                    "this console's panel could not be reached at {} ({failure})",
                    _dir.display()
                ),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Instant;

    #[test]
    fn first_instance_refuses_a_duplicate_and_pipe_dacl_is_exact() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        assert!(
            bind(panel.path(), panel.id()).is_err(),
            "a second first instance captured the pipe name"
        );
        assert_pipe_acl(&listener);
    }

    #[test]
    fn a_client_that_connects_before_the_accept_wait_is_served() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let mut client = connect(panel.path(), panel.id()).unwrap();
        let mut server = listener.accept().unwrap();
        client.write_all(b"x").unwrap();
        let mut byte = [0];
        assert_eq!(server.read(&mut byte).unwrap(), 1);
        assert_eq!(byte[0], b'x');
    }

    #[test]
    fn pane_ids_bind_distinct_pipe_names() {
        let first = panel_contract::PanelDir::create().unwrap();
        let second = panel_contract::PanelDir::create().unwrap();
        let first_listener = bind(first.path(), first.id()).unwrap();
        let second_listener = bind(second.path(), second.id()).unwrap();
        let mut first_client = connect(first.path(), first.id()).unwrap();
        let mut second_client = connect(second.path(), second.id()).unwrap();
        let mut first_server = first_listener.accept().unwrap();
        let mut second_server = second_listener.accept().unwrap();
        first_client.write_all(b"a").unwrap();
        second_client.write_all(b"b").unwrap();
        let mut byte = [0];
        assert_eq!(first_server.read(&mut byte).unwrap(), 1);
        assert_eq!(byte[0], b'a');
        assert_eq!(second_server.read(&mut byte).unwrap(), 1);
        assert_eq!(byte[0], b'b');
    }

    #[test]
    fn dropping_a_channel_cancels_and_drains_a_pending_accept() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let (stop, acceptor) = acceptor(listener).unwrap();
        let acceptor = Arc::new(acceptor);
        let waiting = Arc::clone(&acceptor);
        let worker = thread::spawn(move || waiting.wait());
        wait_for_accept_pending(&acceptor);
        drop(stop);
        assert!(matches!(
            worker.join().unwrap().unwrap(),
            super::super::AcceptorReady::Stop
        ));
        assert!(
            acceptor.pending.lock().unwrap().is_none(),
            "pending connect OVERLAPPED was retained after stop"
        );
    }

    #[test]
    fn stop_cancels_idle_and_partial_reads_after_completion_is_drained() {
        for partial in [false, true] {
            let panel = panel_contract::PanelDir::create().unwrap();
            let listener = bind(panel.path(), panel.id()).unwrap();
            let (stop, acceptor) = acceptor(listener).unwrap();
            let mut client = connect(panel.path(), panel.id()).unwrap();
            acceptor.wait().unwrap();
            let mut server = acceptor.accept().unwrap();
            if partial {
                client.write_all(b"x").unwrap();
            }
            let tracker = Arc::clone(&server.tracker);
            let worker = bounded(move || {
                let mut bytes = [0; 2];
                server.read_exact(&mut bytes)
            });
            wait_for_io_pending(&tracker);
            drop(stop);
            let result = worker
                .recv_timeout(STOP_BOUND)
                .expect("stop did not end the read");
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
            assert_eq!(tracker.pending.load(Ordering::Acquire), 0);
            assert_eq!(tracker.in_flight.load(Ordering::Acquire), 0);
        }
    }

    #[test]
    fn stop_cancels_a_write_to_a_peer_that_is_not_reading() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let (stop, acceptor) = acceptor(listener).unwrap();
        let _client = connect(panel.path(), panel.id()).unwrap();
        acceptor.wait().unwrap();
        let mut server = acceptor.accept().unwrap();
        let tracker = Arc::clone(&server.tracker);
        let worker = bounded(move || server.write_all(&vec![b'x'; 16 * 1024 * 1024]));
        wait_for_io_pending(&tracker);
        drop(stop);
        let result = worker
            .recv_timeout(STOP_BOUND)
            .expect("stop did not end the write");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
        assert_eq!(tracker.pending.load(Ordering::Acquire), 0);
        assert_eq!(tracker.in_flight.load(Ordering::Acquire), 0);
    }

    /// An expired receive deadline reads as the kind the channel's loop retries, so a client that
    /// is quiet for longer than the deadline keeps its connection.
    #[test]
    fn an_expired_read_deadline_is_would_block_and_the_connection_survives() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let (_stop, acceptor) = acceptor(listener).unwrap();
        let mut client = connect(panel.path(), panel.id()).unwrap();
        acceptor.wait().unwrap();
        let mut server = acceptor.accept().unwrap();
        server
            .set_recv_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let mut byte = [0];
        let quiet = server.read(&mut byte).unwrap_err();
        assert_eq!(quiet.kind(), io::ErrorKind::WouldBlock);
        client.write_all(b"y").unwrap();
        assert_eq!(server.read(&mut byte).unwrap(), 1);
        assert_eq!(byte[0], b'y');
    }

    /// Once stopped, a new transfer is refused at once instead of being issued.
    #[test]
    fn a_transfer_after_stop_is_refused_without_waiting() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let (stop, acceptor) = acceptor(listener).unwrap();
        let mut client = connect(panel.path(), panel.id()).unwrap();
        acceptor.wait().unwrap();
        let server = acceptor.accept().unwrap();
        client.write_all(b"z").unwrap();
        drop(stop);
        let tracker = Arc::clone(&server.tracker);
        let worker = bounded(move || {
            let mut byte = [0];
            (&server).read(&mut byte)
        });
        let result = worker
            .recv_timeout(STOP_BOUND)
            .expect("a stopped read waited");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
        assert_eq!(tracker.in_flight.load(Ordering::Acquire), 0);
    }

    /// A hang fails the test within seconds instead of holding the whole job until its limit.
    const STOP_BOUND: Duration = Duration::from_secs(10);

    fn bounded<T: Send + 'static>(
        work: impl FnOnce() -> T + Send + 'static,
    ) -> std::sync::mpsc::Receiver<T> {
        let (done, outcome) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let _ = done.send(work());
        });
        outcome
    }

    #[test]
    fn connect_racing_with_stop_leaves_no_accept_operation_pending() {
        let panel = panel_contract::PanelDir::create().unwrap();
        let listener = bind(panel.path(), panel.id()).unwrap();
        let (stop, acceptor) = acceptor(listener).unwrap();
        let acceptor = Arc::new(acceptor);
        let waiting = Arc::clone(&acceptor);
        let worker = thread::spawn(move || waiting.wait());
        wait_for_accept_pending(&acceptor);
        let client = connect(panel.path(), panel.id()).unwrap();
        drop(stop);
        let outcome = worker.join().unwrap().unwrap();
        if matches!(outcome, super::super::AcceptorReady::Connection) {
            drop(acceptor.accept().unwrap());
        }
        drop(client);
        assert!(
            acceptor.pending.lock().unwrap().is_none(),
            "connect/stop race left a live OVERLAPPED"
        );
    }

    fn wait_for_accept_pending(acceptor: &Acceptor) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !acceptor.pending_ready.load(Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "accept operation never reached pending state"
            );
            thread::yield_now();
        }
    }

    fn wait_for_io_pending(tracker: &IoTracker) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while tracker.pending.load(Ordering::Acquire) == 0 {
            assert!(
                Instant::now() < deadline,
                "I/O operation never reached pending state"
            );
            thread::yield_now();
        }
    }

    fn assert_pipe_acl(listener: &Listener) {
        let listener = listener.inner.lock().unwrap();
        let handle = listener.handle.as_ref().unwrap().0;
        let mut dacl = null_mut();
        let mut descriptor = null_mut();
        let result = unsafe {
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(result, ERROR_SUCCESS);
        let descriptor = Security(descriptor);
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) },
            0
        );
        assert_ne!(control & SE_DACL_PROTECTED, 0);
        let mut info = ACL_SIZE_INFORMATION {
            AceCount: 0,
            AclBytesInUse: 0,
            AclBytesFree: 0,
        };
        assert_ne!(
            unsafe {
                GetAclInformation(
                    dacl,
                    (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                    std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                    AclSizeInformation,
                )
            },
            0
        );
        let sid_text = current_sid_string().unwrap();
        let sid_wide: Vec<u16> = sid_text.encode_utf16().chain(Some(0)).collect();
        let mut user_sid = null_mut();
        assert_ne!(
            unsafe { ConvertStringSidToSidW(sid_wide.as_ptr(), &mut user_sid) },
            0
        );
        let user_sid = Security(user_sid);
        let mut system_sid = [0x00000101u32, 0x05000000, 18];
        let system_sid = system_sid.as_mut_ptr().cast::<std::ffi::c_void>();
        let system_is_user = unsafe { EqualSid(user_sid.0, system_sid) } != 0;
        assert_eq!(info.AceCount, if system_is_user { 1 } else { 2 });
        let mut user_found = false;
        let mut system_found = false;
        for i in 0..info.AceCount {
            let mut ace = null_mut();
            assert_ne!(unsafe { GetAce(dacl, i, &mut ace) }, 0);
            let header = unsafe { &*(ace.cast::<ACE_HEADER>()) };
            let allow = unsafe { &*(ace.cast::<ACCESS_ALLOWED_ACE>()) };
            assert_eq!(header.AceType, 0);
            assert_eq!(header.AceFlags, 0);
            assert_eq!(allow.Mask, FILE_ALL_ACCESS);
            let ace_sid = (&allow.SidStart as *const u32)
                .cast_mut()
                .cast::<std::ffi::c_void>();
            if unsafe { EqualSid(ace_sid, user_sid.0) } != 0 {
                user_found = true;
            } else if unsafe { EqualSid(ace_sid, system_sid) } != 0 {
                system_found = true;
            } else {
                panic!("pipe DACL contains an unexpected principal");
            }
        }
        assert!(user_found);
        assert!(system_is_user || system_found);
    }
}
