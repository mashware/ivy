//! A console-local authenticated transport to the official Codex shared server.
//! Conversation ownership remains upstream; only this integration's runtime MCP route changes.

use crate::codex_route::Registry;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket, accept_hdr_with_config};

const MAX_FRAME: usize = 64 << 20;
const TOKEN_ENV: &str = "IVY_CODEX_RELAY_AUTH";

#[cfg(unix)]
static TERMINATING: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[cfg(unix)]
extern "C" fn console_signal(signal: libc::c_int) {
    // The foreground TUI receives Ctrl+C from the terminal itself. Its parent
    // must stay alive to clean up, without delivering the interrupt twice.
    if signal != libc::SIGINT {
        TERMINATING.store(signal, Ordering::Release);
    }
}

#[cfg(unix)]
struct Signals(Vec<(libc::c_int, libc::sigaction)>);
#[cfg(unix)]
impl Signals {
    fn install() -> io::Result<Self> {
        TERMINATING.store(0, Ordering::Release);
        let mut installed = Self(Vec::new());
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            // A caught disposition resets on exec, so the official child does
            // not inherit an ignored interrupt. The handler only writes an atomic.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = console_signal as *const () as usize;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            if unsafe { libc::sigaction(signal, &action, &mut old) } != 0 {
                return Err(io::Error::last_os_error());
            }
            installed.0.push((signal, old));
        }
        Ok(installed)
    }
}
#[cfg(unix)]
impl Drop for Signals {
    fn drop(&mut self) {
        for (signal, old) in &self.0 {
            unsafe {
                libc::sigaction(*signal, old, std::ptr::null_mut());
            }
        }
    }
}

fn wait_for_client(child: &mut Child) -> io::Result<ExitStatus> {
    #[cfg(unix)]
    let mut forwarded = None;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        #[cfg(unix)]
        {
            let signal = TERMINATING.load(Ordering::Acquire);
            if signal != 0 && forwarded.is_none() {
                unsafe {
                    libc::kill(child.id() as libc::pid_t, signal);
                }
                forwarded = Some(Instant::now());
            }
            if forwarded.is_some_and(|at| at.elapsed() > Duration::from_millis(2200)) {
                return Err(error("Codex did not exit after termination"));
            }
            if forwarded.is_some_and(|at| at.elapsed() > Duration::from_secs(2)) {
                let _ = child.kill();
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

// A killed process can be stuck in an uninterruptible kernel wait. Reap it on
// a detached thread instead of making the console wait without a deadline.
fn kill_and_reap(mut child: Child) {
    let _ = child.kill();
    std::thread::spawn(move || {
        // Abandoned on purpose: the detached reaper may wait for a stuck killed child, never the console.
        let _ = child.wait();
    });
}

fn join_if_finished<T>(handle: std::thread::JoinHandle<T>, grace: Duration) -> Option<T> {
    let deadline = Instant::now() + grace;
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if !handle.is_finished() {
        return None;
    }
    // Bounded: is_finished confirms this thread has already completed.
    handle.join().ok()
}

#[derive(Default)]
struct RouteExit {
    closing: bool,
    active: Option<PathBuf>,
}

fn nonce() -> io::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| error(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Unknown versions retain the existing independent-launch path until exercised.
pub fn supported(exe: &OsStr, args: &[OsString]) -> bool {
    if !cfg!(target_os = "linux") || !supported_args(args) {
        return false;
    }
    output(exe, &["--version"], Duration::from_secs(2))
        .is_ok_and(|out| out.trim() == "codex-cli 0.167.2")
}

fn supported_args(args: &[OsString]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let Some(arg) = args[i].to_str() else {
            return false;
        };
        if arg == "--" {
            return true;
        }
        if matches!(arg, "-m" | "--model" | "-C" | "--cd") {
            i += 2;
            if i > args.len() {
                return false;
            }
            continue;
        }
        if arg.starts_with("--model=")
            || arg.starts_with("--cd=")
            || (arg.len() > 2 && (arg.starts_with("-m") || arg.starts_with("-C")))
        {
            i += 1;
            continue;
        }
        if arg.starts_with('-')
            && !matches!(
                arg,
                "--last" | "--all" | "--no-alt-screen" | "--include-non-interactive"
            )
        {
            return false;
        }
        i += 1;
    }
    true
}

/// Bounded capability/lifecycle commands never inherit terminal input.
fn output(exe: &OsStr, args: &[&str], timeout: Duration) -> io::Result<String> {
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // A shared daemon must not inherit the first pane's identity.
    if args == ["app-server", "daemon", "start"] {
        for name in panel_contract::ENV_NAMES {
            command.env_remove(name);
        }
    }
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| error("missing command output"))?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut data = Vec::new();
        let result = stdout.take(1024 * 1024 + 1).read_to_end(&mut data);
        let _ = tx.send(result.map(|_| data));
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            kill_and_reap(child);
            return Err(error("Codex command timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        return Err(error("Codex command failed"));
    }
    let data = rx
        .recv_timeout(Duration::from_secs(1))
        .map_err(|_| error("Codex command output did not finish"))??;
    if data.len() > 1024 * 1024 {
        return Err(error("Codex command output exceeded its limit"));
    }
    String::from_utf8(data).map_err(|_| error("Codex command output was not UTF-8"))
}

fn home() -> io::Result<PathBuf> {
    let path = if let Some(home) = std::env::var_os("CODEX_HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or_else(|| error("cannot locate Codex home"))?,
        )
        .join(".codex")
    };
    if !path.is_absolute() {
        return Err(error("Codex home is not absolute"));
    }
    Ok(path)
}

fn verified_server(start: &Value) -> bool {
    start.get("appServerVersion").and_then(Value::as_str) == Some("0.167.2")
}

/// Does not enable remote control, restart an existing server, or edit user settings.
pub fn run(exe: &OsStr, args: &[OsString]) -> io::Result<ExitStatus> {
    #[cfg(unix)]
    let _signals = Signals::install()?;
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    let (id, dir) = crate::client::whole_identity(id.as_deref(), dir.as_deref())
        .ok_or_else(|| error("shared Codex launch requires this console's identity"))?;
    let codex_home = home()?;
    let registry = Registry::for_home(&codex_home)?;
    let exit_registry = Registry::for_home(&codex_home)?;
    let start: Value = serde_json::from_str(&output(
        exe,
        &["app-server", "daemon", "start"],
        Duration::from_secs(30),
    )?)
    .map_err(|_| error("Codex returned an invalid daemon description"))?;
    let socket = start
        .get("socketPath")
        .and_then(Value::as_str)
        .ok_or_else(|| error("Codex did not report its daemon socket"))?
        .to_owned();
    // A separately installed daemon can differ from the CLI. Do not replace it.
    if !verified_server(&start) {
        return Err(error(
            "the running Codex server is not the verified version 0.167.2; update it explicitly or use --no-daemon",
        ));
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let token = nonce()?;
    let epoch = nonce()?;
    let relay_epoch = epoch.clone();
    let exit_route = Arc::new(Mutex::new(RouteExit::default()));
    let relay_route = Arc::clone(&exit_route);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let exe_owned = exe.to_owned();
    let expected = token.clone();
    let pane_id = id.to_owned();
    let pane_dir = dir.to_owned();
    let relay = std::thread::spawn(move || {
        let result = serve(
            listener,
            &expected,
            &exe_owned,
            &socket,
            registry,
            &pane_id,
            &pane_dir,
            &relay_epoch,
            relay_route,
            &thread_stop,
        );
        match result {
            Ok(thread) => thread,
            Err(failure) => {
                // Protocol payloads and capabilities are deliberately absent from diagnostics.
                eprintln!("ivy-codex: shared connection stopped ({failure})");
                None
            }
        }
    });
    let child = Command::new(exe)
        .arg("--remote")
        .arg(format!("ws://{address}"))
        .arg("--remote-auth-token-env")
        .arg(TOKEN_ENV)
        .args(args)
        .env(TOKEN_ENV, token)
        .spawn();
    let result = match child {
        Ok(mut child) => match wait_for_client(&mut child) {
            Ok(status) => Ok(status),
            Err(failure) => {
                kill_and_reap(child);
                Err(failure)
            }
        },
        Err(failure) => Err(failure),
    };
    stop.store(true, Ordering::Release);
    // Persist cancellation without acquiring the route or registry locks. An MCP
    // operation already holding one of them must not keep this epoch usable.
    let cancellation = exit_registry.cancel_epoch(&epoch);
    if cancellation.is_err() {
        eprintln!("ivy-codex: panel route cancellation could not be persisted on exit");
    }
    // Close the route independently of the relay: its proxy pipe can be stuck.
    // The same mutex guards binding, so no late response can reactivate a route.
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        match exit_route.try_lock() {
            Ok(mut guard) => {
                guard.closing = true;
                if let Some(route) = guard.active.take()
                    && exit_registry.revoke(&route, &epoch).is_err()
                {
                    eprintln!("ivy-codex: panel route could not be revoked on exit");
                }
                break;
            }
            Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => {
                eprintln!("ivy-codex: panel route cleanup could not finish on exit");
                break;
            }
        }
    }
    if let Some(Some(thread)) = join_if_finished(relay, Duration::from_millis(200))
        && thread.len() == 36
        && thread.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        // The native remote endpoint was private to this invocation. Its printed
        // reconnect command cannot outlive it; offer the stable shell entry point.
        eprintln!("ivy-codex: return to this conversation with: codex resume {thread}");
    }
    cancellation?;
    result
}

#[allow(clippy::result_large_err)]
fn authenticated(stream: TcpStream, expected: &str) -> io::Result<WebSocket<TcpStream>> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let wanted = format!("Bearer {expected}");
    let callback = |request: &Request, response: Response| {
        let matches = request
            .headers()
            .get("authorization")
            .is_some_and(|value| value.as_bytes() == wanted.as_bytes());
        if matches {
            Ok(response)
        } else {
            let mut rejection = tungstenite::http::Response::new(Some("Unauthorized".to_owned()));
            *rejection.status_mut() = tungstenite::http::StatusCode::UNAUTHORIZED;
            Err(rejection)
        }
    };
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(MAX_FRAME);
    config.max_frame_size = Some(16 << 20);
    config.max_write_buffer_size = MAX_FRAME + (1 << 20);
    let ws = accept_hdr_with_config(stream, callback, Some(config))
        .map_err(|_| error("local authentication or WebSocket handshake failed"))?;
    ws.get_ref()
        .set_read_timeout(Some(Duration::from_millis(10)))?;
    Ok(ws)
}

// The official proxy carries raw WebSocket bytes, including its HTTP upgrade.
// A bounded pipe reader supplies timeouts without platform-specific pipe polling.
struct ProxyStream {
    child: Option<Child>,
    input: ChildStdin,
    receiver: Option<mpsc::Receiver<Vec<u8>>>,
    buffer: std::io::Cursor<Vec<u8>>,
    reader: Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
}
impl ProxyStream {
    fn open(exe: &OsStr, socket: &str) -> io::Result<Self> {
        let mut child = Command::new(exe)
            .args(["app-server", "proxy", "--sock", socket])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| error("missing official proxy input"))?;
        let mut output = child
            .stdout
            .take()
            .ok_or_else(|| error("missing official proxy output"))?;
        let (tx, rx) = mpsc::sync_channel(16);
        let reader = std::thread::spawn(move || {
            loop {
                let mut chunk = vec![0; 8192];
                match output.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        chunk.truncate(n);
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            child: Some(child),
            input,
            receiver: Some(rx),
            buffer: std::io::Cursor::new(Vec::new()),
            reader: Some(reader),
            timeout: Duration::from_secs(5),
        })
    }
}
impl Read for ProxyStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let n = self.buffer.read(out)?;
        if n != 0 {
            return Ok(n);
        }
        let rx = self
            .receiver
            .as_ref()
            .ok_or_else(|| error("proxy closed"))?;
        match rx.recv_timeout(self.timeout) {
            Ok(chunk) => {
                self.buffer = std::io::Cursor::new(chunk);
                self.buffer.read(out)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::ErrorKind::WouldBlock.into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(0),
        }
    }
}
impl Write for ProxyStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.input.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.input.flush()
    }
}
impl Drop for ProxyStream {
    fn drop(&mut self) {
        drop(self.receiver.take());
        if let Some(child) = self.child.take() {
            kill_and_reap(child);
        }
        if let Some(reader) = self.reader.take() {
            // The reader may remain in OS read if another process owns the pipe.
            let _ = join_if_finished(reader, Duration::from_millis(200));
        }
    }
}
trait Protocol {
    fn send_value(&mut self, value: &Value) -> io::Result<()>;
}
impl<S: Read + Write> Protocol for WebSocket<S> {
    fn send_value(&mut self, value: &Value) -> io::Result<()> {
        self.send(Message::text(value.to_string()))
            .map_err(|_| error("protocol peer disconnected"))
    }
}

struct Attachment {
    route: Option<PathBuf>,
    fresh: bool,
    servers: Vec<String>,
    needs_inventory: bool,
}

enum Internal {
    ReadThread(Value),
    ReadConfig(Value),
    Inventory {
        attachment: Attachment,
        response: Value,
    },
    Proof {
        attachment: Attachment,
        response: Value,
        remaining: Vec<String>,
    },
}

struct Routing {
    registry: Registry,
    pending: HashMap<String, Attachment>,
    internal: HashMap<String, Internal>,
    active: Option<PathBuf>,
    exit_route: Arc<Mutex<RouteExit>>,
    last_thread: Option<String>,
    epoch: String,
    pane_id: String,
    pane_dir: PathBuf,
    launch_dir: PathBuf,
    serial: u64,
}

impl Drop for Routing {
    fn drop(&mut self) {
        if let Some(route) = &self.active {
            let _ = self.registry.revoke(route, &self.epoch);
        }
        for a in self.pending.values() {
            if a.fresh
                && let Some(route) = &a.route
            {
                let _ = std::fs::remove_file(route);
            }
        }
        for stage in self.internal.values() {
            if let Internal::Proof { attachment: a, .. } = stage
                && a.fresh
                && let Some(route) = &a.route
            {
                let _ = std::fs::remove_file(route);
            }
        }
    }
}

fn write_upstream(output: &mut impl Protocol, value: &Value) -> io::Result<()> {
    output.send_value(value)
}
fn send(ws: &mut impl Protocol, value: &Value) -> io::Result<()> {
    ws.send_value(value)
}
fn reject(ws: &mut impl Protocol, id: &Value, message: &str) -> io::Result<()> {
    send(
        ws,
        &json!({"id":id,"error":{"code":-32001,"message":message}}),
    )
}

// Start must use the invoking console's directory; saved sessions retain their own
// cwd when it is not explicitly supplied. The latter needs an upstream thread/read.
fn discovery_cwd(message: &mut Value, launch: &Path) -> io::Result<Option<PathBuf>> {
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let params = message
        .get_mut("params")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("conversation request has no parameters"))?;
    match params.get("cwd") {
        Some(Value::String(cwd)) => Ok(Some(PathBuf::from(cwd))),
        None | Some(Value::Null) if method == "thread/start" => {
            params.insert("cwd".into(), json!(launch));
            Ok(Some(launch.to_path_buf()))
        }
        None | Some(Value::Null) => Ok(None),
        _ => Err(error("invalid conversation directory")),
    }
}

impl Routing {
    fn reset_connection(&mut self) {
        if let Ok(mut exit) = self.exit_route.lock()
            && let Some(route) = self.active.take()
            && self.registry.revoke(&route, &self.epoch).is_ok()
        {
            exit.active = None;
        }
        for (_, a) in self.pending.drain() {
            if a.fresh
                && let Some(route) = a.route
            {
                let _ = std::fs::remove_file(route);
            }
        }
        for (_, stage) in self.internal.drain() {
            if let Internal::Proof { attachment: a, .. } = stage
                && a.fresh
                && let Some(route) = a.route
            {
                let _ = std::fs::remove_file(route);
            }
        }
    }
    fn private(&mut self, stage: Internal) -> String {
        self.serial += 1;
        let id = format!("ivy-private-{}", self.serial);
        self.internal.insert(id.clone(), stage);
        id
    }
    fn config_request(
        &mut self,
        request: Value,
        cwd: &Path,
        upstream: &mut impl Protocol,
    ) -> io::Result<()> {
        let id = self.private(Internal::ReadConfig(request));
        write_upstream(
            upstream,
            &json!({"id":id,"method":"config/read","params":{"includeLayers":false,"cwd":cwd}}),
        )
    }
    fn request(
        &mut self,
        mut message: Value,
        upstream: &mut impl Protocol,
        _ws: &mut impl Protocol,
    ) -> io::Result<()> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return write_upstream(upstream, &message);
        };
        if !matches!(method, "thread/start" | "thread/resume" | "thread/fork")
            || message.get("id").is_none()
        {
            return write_upstream(upstream, &message);
        }
        if let Some(cwd) = discovery_cwd(&mut message, &self.launch_dir)? {
            return self.config_request(message, &cwd, upstream);
        }
        let thread = message
            .pointer("/params/threadId")
            .and_then(Value::as_str)
            .ok_or_else(|| error("saved conversation has no thread ID"))?
            .to_owned();
        let id = self.private(Internal::ReadThread(message));
        write_upstream(
            upstream,
            &json!({"id":id,"method":"thread/read","params":{"threadId":thread,"includeTurns":false}}),
        )
    }
    fn inject(message: &mut Value, route: &Path, servers: &[String]) -> io::Result<()> {
        let params = message
            .get_mut("params")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("conversation request has no parameters"))?;
        let config = params.entry("config").or_insert_with(|| json!({}));
        if config.is_null() {
            *config = json!({});
        }
        let config = config
            .as_object_mut()
            .ok_or_else(|| error("invalid conversation config"))?;
        // A parent-table override replaces preceding app-server CLI overrides.
        // Dotted leaves retain a daemon's explicit command/env/policy choices.
        // This target version splits paths literally and cannot address a server
        // whose name itself contains a dot without replacing the whole table.
        for name in servers {
            if name.is_empty() || name.contains('.') {
                return Err(error(
                    "this Codex version cannot safely override an Ivy MCP server name containing a dot",
                ));
            }
            config.insert(
                format!("mcp_servers.{name}.env.{}", crate::codex_route::ENV),
                json!(route),
            );
        }
        Ok(())
    }
    fn start_attachment(
        &mut self,
        mut request: Value,
        config: &Value,
        upstream: &mut impl Protocol,
    ) -> io::Result<()> {
        let id = request
            .get("id")
            .cloned()
            .ok_or_else(|| error("missing request ID"))?;
        let servers = configured_servers(config);
        if servers.is_empty() {
            eprintln!(
                "ivy-codex: configure the bundled panel-mcp server to use the Ivy panel in this conversation"
            );
            self.pending.insert(
                id.to_string(),
                Attachment {
                    route: None,
                    fresh: false,
                    servers,
                    needs_inventory: request.get("method").and_then(Value::as_str)
                        != Some("thread/start"),
                },
            );
            return write_upstream(upstream, &request);
        }
        let existing = if request.get("method").and_then(Value::as_str) == Some("thread/resume") {
            request
                .pointer("/params/threadId")
                .and_then(Value::as_str)
                .map(|thread| self.registry.route_for(thread))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let fresh = existing.is_none();
        let route = match existing {
            Some(route) => route,
            None => self.registry.pending()?,
        };
        if let Err(e) = Self::inject(&mut request, &route, &servers) {
            if fresh {
                let _ = std::fs::remove_file(&route);
            }
            return Err(e);
        }
        self.pending.insert(
            id.to_string(),
            Attachment {
                route: Some(route),
                fresh,
                servers,
                needs_inventory: false,
            },
        );
        write_upstream(upstream, &request)
    }
    fn proof_request(
        &mut self,
        attachment: Attachment,
        response: Value,
        mut remaining: Vec<String>,
        upstream: &mut impl Protocol,
    ) -> io::Result<()> {
        let server = remaining
            .pop()
            .ok_or_else(|| error("no MCP proof target"))?;
        let thread = response
            .pointer("/result/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("missing thread ID"))?
            .to_owned();
        let id = self.private(Internal::Proof {
            attachment,
            response,
            remaining,
        });
        write_upstream(
            upstream,
            &json!({"id":id,"method":"mcpServer/tool/call","params":{"threadId":thread,"server":server,"tool":"panel_get","arguments":{}}}),
        )
    }
    fn finish(
        &mut self,
        attachment: Attachment,
        response: Value,
        ws: &mut impl Protocol,
    ) -> io::Result<()> {
        let id = response.get("id").cloned().unwrap_or(Value::Null);
        let mut exit = self
            .exit_route
            .lock()
            .map_err(|_| error("route exit state unavailable"))?;
        if exit.closing {
            drop(exit);
            return reject(ws, &id, "The console is closing; no panel was rebound.");
        }
        if let Some(route) = &attachment.route {
            let thread = response
                .pointer("/result/thread/id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("missing thread ID"))?;
            if self
                .registry
                .bind(thread, route, &self.pane_id, &self.pane_dir, &self.epoch)
                .is_err()
            {
                if attachment.fresh {
                    let _ = std::fs::remove_file(route);
                }
                drop(exit);
                return reject(
                    ws,
                    &id,
                    "The conversation opened, but Ivy could not bind its panel safely.",
                );
            }
        }
        self.last_thread = response
            .pointer("/result/thread/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        exit.active = attachment.route.clone();
        let old = std::mem::replace(&mut self.active, attachment.route);
        if let Some(old) = old
            && self.active.as_ref() != Some(&old)
        {
            self.registry.revoke(&old, &self.epoch)?;
        }
        drop(exit);
        send(ws, &response)
    }
    fn response(
        &mut self,
        message: Value,
        upstream: &mut impl Protocol,
        ws: &mut impl Protocol,
    ) -> io::Result<()> {
        // Server requests and notifications are never responses even if IDs coincide.
        if message.get("method").is_some() {
            return send(ws, &message);
        }
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if let Some(stage) = id.as_str().and_then(|id| self.internal.remove(id)) {
            match stage {
                Internal::ReadThread(request) => {
                    let original_id = request.get("id").cloned().unwrap_or(Value::Null);
                    let Some(cwd) = message
                        .pointer("/result/thread/cwd")
                        .and_then(Value::as_str)
                    else {
                        return reject(
                            ws,
                            &original_id,
                            "Ivy could not determine the saved conversation directory.",
                        );
                    };
                    self.config_request(request, Path::new(cwd), upstream)
                }
                Internal::ReadConfig(request) => {
                    let original_id = request.get("id").cloned().unwrap_or(Value::Null);
                    if message.get("result").is_none() {
                        return reject(
                            ws,
                            &original_id,
                            "Ivy could not read the conversation configuration.",
                        );
                    }
                    self.start_attachment(request, &message, upstream)
                }
                Internal::Inventory {
                    attachment,
                    response,
                } => {
                    let original_id = response.get("id").cloned().unwrap_or(Value::Null);
                    if !inventory_excludes_ivy(&message) {
                        return reject(
                            ws,
                            &original_id,
                            "The conversation may still use an earlier Ivy panel connection. Restore its panel configuration or leave the old session before resuming; no panel was rebound.",
                        );
                    }
                    self.finish(attachment, response, ws)
                }
                Internal::Proof {
                    attachment,
                    response,
                    remaining,
                } => {
                    let original_id = response.get("id").cloned().unwrap_or(Value::Null);
                    if !proof_matches(attachment.route.as_deref(), &message) {
                        if attachment.fresh
                            && let Some(route) = &attachment.route
                        {
                            let _ = std::fs::remove_file(route);
                        }
                        return reject(
                            ws,
                            &original_id,
                            "The conversation opened, but its panel connection could not be verified. No panel was rebound.",
                        );
                    }
                    if remaining.is_empty() {
                        self.finish(attachment, response, ws)
                    } else {
                        self.proof_request(attachment, response, remaining, upstream)
                    }
                }
            }
        } else if let Some(attachment) = self.pending.remove(&id.to_string()) {
            if message
                .pointer("/result/thread/id")
                .and_then(Value::as_str)
                .is_none()
            {
                if attachment.fresh
                    && let Some(route) = &attachment.route
                {
                    let _ = std::fs::remove_file(route);
                }
                return send(ws, &message);
            }
            if attachment.route.is_none() && !attachment.needs_inventory {
                return self.finish(attachment, message, ws);
            }
            if attachment.route.is_none() {
                let thread = message
                    .pointer("/result/thread/id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| error("missing thread ID"))?
                    .to_owned();
                let internal = self.private(Internal::Inventory {
                    attachment,
                    response: message,
                });
                return write_upstream(
                    upstream,
                    &json!({"id":internal,"method":"mcpServerStatus/list","params":{"threadId":thread,"detail":"toolsAndAuthOnly","limit":10000}}),
                );
            }
            let servers = attachment.servers.clone();
            self.proof_request(attachment, message, servers, upstream)
        } else {
            send(ws, &message)
        }
    }
}

// A missing page, failed discovery, or server still initializing cannot establish
// that a previously loaded panel bridge is absent. Do not query third-party tools.
fn inventory_excludes_ivy(message: &Value) -> bool {
    let Some(result) = message.get("result").and_then(Value::as_object) else {
        return false;
    };
    if result.get("nextCursor") != Some(&Value::Null) {
        return false;
    }
    let Some(entries) = result.get("data").and_then(Value::as_array) else {
        return false;
    };
    entries.iter().all(|entry| {
        let Some(obj) = entry.as_object() else {
            return false;
        };
        let Some(name) = obj
            .get("serverInfo")
            .and_then(|v| v.get("name"))
            .and_then(Value::as_str)
        else {
            return false;
        };
        if matches!(name, "ivy panel" | "agent-terminal panel") {
            return false;
        }
        if obj.get("runtimeStatus").and_then(Value::as_str) != Some("connected")
            || obj.get("toolsError") != Some(&Value::Null)
        {
            return false;
        }
        let Some(tools) = obj.get("tools").and_then(Value::as_object) else {
            return false;
        };
        ![
            "panel_get",
            "panel_set",
            "split_open",
            "split_read",
            "split_close",
        ]
        .iter()
        .all(|name| tools.contains_key(*name))
    })
}

fn configured_servers(config: &Value) -> Vec<String> {
    config
        .pointer("/result/config/mcp_servers")
        .and_then(Value::as_object)
        .map(|servers| {
            servers
                .iter()
                .filter(|(_, v)| ivy_server(v))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn proof_matches(route: Option<&Path>, message: &Value) -> bool {
    let expected = route
        .and_then(|p| crate::codex_route::lock_route(p).ok())
        .map(|g| g.identity.proof.clone());
    let actual = message
        .pointer("/result/_meta/ivy~1routeProof")
        .and_then(Value::as_str);
    expected
        .as_deref()
        .is_some_and(|expected| actual == Some(expected))
}

fn ivy_server(server: &Value) -> bool {
    server.get("enabled") != Some(&Value::Bool(false))
        && server.get("url").is_none_or(Value::is_null)
        && server
            .get("command")
            .and_then(Value::as_str)
            .and_then(|command| Path::new(command).file_name())
            .and_then(OsStr::to_str)
            .is_some_and(|name| matches!(name, "panel-mcp" | "panel-mcp.exe"))
}

#[derive(Debug, PartialEq)]
enum HealthAction {
    None,
    Probe,
    Expired,
}
struct ConnectionHealth {
    heard_at: Instant,
    probe_at: Option<Instant>,
}
impl ConnectionHealth {
    fn new(now: Instant) -> Self {
        Self {
            heard_at: now,
            probe_at: None,
        }
    }
    fn heard(&mut self, now: Instant) {
        self.heard_at = now;
        self.probe_at = None;
    }
    fn poll(&mut self, now: Instant) -> HealthAction {
        if self
            .probe_at
            .is_some_and(|at| now.duration_since(at) >= Duration::from_secs(5))
        {
            return HealthAction::Expired;
        }
        if self.probe_at.is_none() && now.duration_since(self.heard_at) >= Duration::from_secs(5) {
            self.probe_at = Some(now);
            return HealthAction::Probe;
        }
        HealthAction::None
    }
}

#[allow(clippy::too_many_arguments)]
fn serve(
    listener: TcpListener,
    token: &str,
    exe: &OsStr,
    socket: &str,
    registry: Registry,
    pane_id: &str,
    pane_dir: &Path,
    epoch: &str,
    exit_route: Arc<Mutex<RouteExit>>,
    stop: &AtomicBool,
) -> io::Result<Option<String>> {
    let mut routing = Routing {
        registry,
        pending: HashMap::new(),
        internal: HashMap::new(),
        active: None,
        exit_route,
        last_thread: None,
        epoch: epoch.to_owned(),
        pane_id: pane_id.to_owned(),
        pane_dir: pane_dir.to_owned(),
        launch_dir: std::env::current_dir()?,
        serial: 0,
    };
    while !stop.load(Ordering::Acquire) {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => return Err(e),
        };
        let Ok(mut ws) = authenticated(stream, token) else {
            continue;
        };
        let stream = match ProxyStream::open(exe, socket) {
            Ok(stream) => stream,
            Err(_) => {
                let _ = ws.close(None);
                continue;
            }
        };
        let (mut upstream, _) = match tungstenite::client("ws://localhost/", stream) {
            Ok(value) => value,
            Err(_) => {
                let _ = ws.close(None);
                continue;
            }
        };
        upstream.get_mut().timeout = Duration::from_millis(10);
        let mut health = ConnectionHealth::new(Instant::now());
        let result = (|| {
            while !stop.load(Ordering::Acquire) {
                // A continuous stream must still leave the user's input a turn.
                for _ in 0..64 {
                    let incoming = upstream.read();
                    if incoming.is_ok() {
                        health.heard(Instant::now());
                    }
                    match incoming {
                        Ok(Message::Text(text)) => {
                            let message: Value = serde_json::from_str(&text)
                                .map_err(|_| error("invalid official protocol message"))?;
                            routing.response(message, &mut upstream, &mut ws)?;
                        }
                        Ok(Message::Ping(_) | Message::Pong(_)) => {
                            upstream
                                .flush()
                                .map_err(|_| error("official proxy disconnected"))?;
                        }
                        Ok(Message::Close(_)) => {
                            return Err(error("official server closed its connection"));
                        }
                        Ok(_) => return Err(error("unsupported official protocol frame")),
                        Err(tungstenite::Error::Io(e))
                            if matches!(
                                e.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                            ) =>
                        {
                            break;
                        }
                        Err(_) => return Err(error("official proxy disconnected")),
                    }
                }
                match health.poll(Instant::now()) {
                    HealthAction::Probe => upstream
                        .send(Message::Ping(Vec::new().into()))
                        .map_err(|_| error("official proxy disconnected"))?,
                    HealthAction::Expired => {
                        return Err(error("official server stopped responding"));
                    }
                    HealthAction::None => {}
                }
                match ws.read() {
                    Ok(Message::Text(text)) => {
                        let value: Value = serde_json::from_str(&text)
                            .map_err(|_| error("invalid local protocol message"))?;
                        if value.get("method").is_some()
                            && value
                                .get("id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| id.starts_with("ivy-private-"))
                        {
                            return Err(error("local request uses a reserved correlation ID"));
                        }
                        routing.request(value, &mut upstream, &mut ws)?;
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(_) | Message::Pong(_)) => {
                        ws.flush().map_err(|_| error("local client disconnected"))?;
                    }
                    Ok(_) => return Err(error("unsupported local protocol frame")),
                    Err(tungstenite::Error::Io(e))
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(
                        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed,
                    ) => break,
                    Err(_) => return Err(error("local protocol connection failed")),
                }
            }
            Ok(())
        })();
        drop(upstream);
        if result.is_err() {
            let _ = ws.close(None);
        }
        routing.reset_connection();
    }
    Ok(routing.last_thread.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_proxy_requires_a_live_server_response() {
        let now = Instant::now();
        let mut health = ConnectionHealth::new(now);
        assert_eq!(
            health.poll(now + Duration::from_secs(4)),
            HealthAction::None
        );
        assert_eq!(
            health.poll(now + Duration::from_secs(5)),
            HealthAction::Probe
        );
        assert_eq!(
            health.poll(now + Duration::from_secs(10)),
            HealthAction::Expired
        );
        health.heard(now + Duration::from_secs(10));
        assert_eq!(
            health.poll(now + Duration::from_secs(11)),
            HealthAction::None
        );
    }

    #[test]
    fn cwd_discovery_keeps_saved_thread_semantics() {
        let mut start = json!({"method":"thread/start","params":{"cwd":null}});
        assert_eq!(
            discovery_cwd(&mut start, Path::new("/shell")).unwrap(),
            Some(PathBuf::from("/shell"))
        );
        assert_eq!(start["params"]["cwd"], "/shell");
        let mut explicit = json!({"method":"thread/start","params":{"cwd":"/project"}});
        assert_eq!(
            discovery_cwd(&mut explicit, Path::new("/shell")).unwrap(),
            Some(PathBuf::from("/project"))
        );
        let mut resume = json!({"method":"thread/resume","params":{"cwd":null}});
        assert_eq!(
            discovery_cwd(&mut resume, Path::new("/shell")).unwrap(),
            None
        );
        assert!(resume["params"]["cwd"].is_null());
    }
    #[test]
    fn normalized_readback_is_not_reinserted_as_input_configuration() {
        let mut message = json!({"params":{"config":{"features":{"existing":true}}}});
        Routing::inject(
            &mut message,
            Path::new("/route"),
            &["custom-ivy-name".into()],
        )
        .unwrap();
        assert_eq!(
            message["params"]["config"],
            json!({"features":{"existing":true},"mcp_servers.custom-ivy-name.env.IVY_CODEX_ROUTE":"/route"})
        );
    }
    #[test]
    fn route_override_addresses_only_the_leaf_and_rejects_ambiguous_names() {
        let mut message = json!({"params":{"config":{}}});
        Routing::inject(&mut message, Path::new("/route"), &["ivy".into()]).unwrap();
        let config = message["params"]["config"].as_object().unwrap();
        assert_eq!(config.len(), 1);
        assert_eq!(config["mcp_servers.ivy.env.IVY_CODEX_ROUTE"], "/route");
        assert!(!config.contains_key("mcp_servers"));
        for name in ["", "ivy.other"] {
            assert!(Routing::inject(&mut message, Path::new("/route"), &[name.into()]).is_err());
        }
    }
    #[test]
    fn proof_must_match_current_route() {
        let home = std::env::temp_dir().join(format!("ivy-proof-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let registry = Registry::for_home(&home).unwrap();
        let route = registry.pending().unwrap();
        let proof = crate::codex_route::lock_route(&route)
            .unwrap()
            .identity
            .proof
            .clone();
        assert!(!proof_matches(Some(&route), &json!({"result":{}})));
        assert!(!proof_matches(
            Some(&route),
            &json!({"result":{"_meta":{"ivy/routeProof":"wrong"}}})
        ));
        assert!(!proof_matches(
            None,
            &json!({"result":{"_meta":{"ivy/routeProof":proof}}})
        ));
        assert!(proof_matches(
            Some(&route),
            &json!({"result":{"_meta":{"ivy/routeProof":proof}}})
        ));
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn first_run_without_panel_configuration_requires_no_injection() {
        assert!(configured_servers(&json!({"result":{"config":{"mcp_servers":{}}}})).is_empty());
        assert_eq!(
            configured_servers(
                &json!({"result":{"config":{"mcp_servers":{"one":{"command":"/bin/panel-mcp","tool_timeout_sec":null},"other":{"command":"/bin/other"}}}}})
            ),
            vec!["one"]
        );
    }
    #[derive(Default)]
    struct Peer(Vec<Value>);
    impl Protocol for Peer {
        fn send_value(&mut self, value: &Value) -> io::Result<()> {
            self.0.push(value.clone());
            Ok(())
        }
    }

    fn routing(home: &Path) -> Routing {
        Routing {
            registry: Registry::for_home(home).unwrap(),
            pending: HashMap::new(),
            internal: HashMap::new(),
            active: None,
            exit_route: Arc::new(Mutex::new(RouteExit::default())),
            last_thread: None,
            epoch: "new-epoch".into(),
            pane_id: "new-pane".into(),
            pane_dir: home.join("new"),
            launch_dir: home.to_path_buf(),
            serial: 0,
        }
    }

    #[test]
    fn loaded_resume_success_is_held_until_the_running_mcp_proves_its_route() {
        let home = std::env::temp_dir().join(format!("ivy-proof-state-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let mut state = routing(&home);
        let route = state.registry.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        state
            .registry
            .bind(thread, &route, "old", &home.join("old"), "old-epoch")
            .unwrap();
        let original = json!({"id":17,"result":{"thread":{"id":thread}}});
        let mut upstream = Peer::default();
        let mut client = Peer::default();
        let prepare = || Attachment {
            route: Some(route.clone()),
            fresh: false,
            servers: vec!["ivy".into()],
            needs_inventory: false,
        };
        state.pending.insert("17".into(), prepare());
        // A colliding server request must stay a server request, not commit a binding.
        let server_request = json!({"id":17,"method":"permission/request","params":{}});
        state
            .response(server_request.clone(), &mut upstream, &mut client)
            .unwrap();
        assert_eq!(client.0.pop(), Some(server_request));
        state
            .response(original.clone(), &mut upstream, &mut client)
            .unwrap();
        assert!(client.0.is_empty(), "no successful attach before proof");
        let probe = upstream.0.pop().unwrap();
        assert_eq!(probe["method"], "mcpServer/tool/call");
        assert_eq!(probe["params"]["threadId"], thread);
        // This is the race: upstream resume succeeded, but an old runtime ignored
        // our fresh configuration and reports a different (or absent) proof.
        state.response(json!({"id":probe["id"],"result":{"content":[],"_meta":{"ivy/routeProof":"stale"}}}),&mut upstream,&mut client).unwrap();
        assert!(client.0.pop().unwrap().get("error").is_some());
        assert_eq!(
            crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .pane_id,
            "old"
        );
        assert!(state.active.is_none());
        state.pending.insert("17".into(), prepare());
        state
            .response(original.clone(), &mut upstream, &mut client)
            .unwrap();
        let probe = upstream.0.pop().unwrap();
        let proof = crate::codex_route::lock_route(&route)
            .unwrap()
            .identity
            .proof
            .clone();
        state
            .response(
                json!({"id":probe["id"],"result":{"content":[],"_meta":{"ivy/routeProof":proof}}}),
                &mut upstream,
                &mut client,
            )
            .unwrap();
        assert_eq!(client.0.pop(), Some(original));
        assert_eq!(
            crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .pane_id,
            "new-pane"
        );
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn ordinary_and_saved_requests_discover_configuration_in_the_right_directory() {
        let home = std::env::temp_dir().join(format!("ivy-config-state-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let mut state = routing(&home);
        let mut server = Peer::default();
        let mut client = Peer::default();
        state
            .request(
                json!({"id":4,"method":"thread/start","params":{"cwd":null}}),
                &mut server,
                &mut client,
            )
            .unwrap();
        let read = server.0.pop().unwrap();
        assert_eq!(read["method"], "config/read");
        assert_eq!(read["params"]["cwd"], json!(home));
        state
            .response(
                json!({"id":read["id"],"result":{"config":{"mcp_servers":{"slow":{"command":"/unrelated/mcp"}}}}}),
                &mut server,
                &mut client,
            )
            .unwrap();
        let start = server.0.pop().unwrap();
        assert_eq!(start["method"], "thread/start");
        assert_eq!(start["params"]["cwd"], json!(home));
        state
            .response(
                json!({"id":4,"result":{"thread":{"id":"01234567-89ab-cdef-0123-456789abcdef"}}}),
                &mut server,
                &mut client,
            )
            .unwrap();
        assert!(
            server.0.is_empty(),
            "a new conversation cannot inherit a loaded MCP"
        );
        assert!(client.0.pop().unwrap().get("result").is_some());
        let saved = home.join("saved-project");
        state.request(json!({"id":5,"method":"thread/resume","params":{"threadId":"01234567-89ab-cdef-0123-456789abcdef","cwd":null}}),&mut server,&mut client).unwrap();
        let read = server.0.pop().unwrap();
        assert_eq!(read["method"], "thread/read");
        state
            .response(
                json!({"id":read["id"],"result":{"thread":{"cwd":saved}}}),
                &mut server,
                &mut client,
            )
            .unwrap();
        let read = server.0.pop().unwrap();
        assert_eq!(read["method"], "config/read");
        assert_eq!(read["params"]["cwd"], json!(saved));
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn empty_config_requires_complete_inventory_before_accepting_resume() {
        let home = std::env::temp_dir().join(format!("ivy-inventory-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let mut state = routing(&home);
        let route = state.registry.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        state
            .registry
            .bind(thread, &route, "old-pane", &home.join("old"), "old-epoch")
            .unwrap();
        let mut upstream = Peer::default();
        let mut client = Peer::default();
        let request =
            json!({"id":17,"method":"thread/resume","params":{"threadId":thread,"cwd":"/saved"}});
        let config = json!({"result":{"config":{"mcp_servers":{}}}});
        let original = json!({"id":17,"result":{"thread":{"id":thread}}});
        state
            .start_attachment(request.clone(), &config, &mut upstream)
            .unwrap();
        assert_eq!(upstream.0.pop().unwrap(), request);
        state
            .response(original.clone(), &mut upstream, &mut client)
            .unwrap();
        assert!(client.0.is_empty());
        let inventory = upstream.0.pop().unwrap();
        assert_eq!(inventory["method"], "mcpServerStatus/list");
        let collision = json!({"id":inventory["id"],"method":"permission/request","params":{}});
        state
            .response(collision.clone(), &mut upstream, &mut client)
            .unwrap();
        assert_eq!(client.0.pop(), Some(collision));
        state.response(json!({"id":inventory["id"],"result":{"data":[{"serverInfo":{"name":"ivy panel"},"runtimeStatus":"connected","tools":{},"toolsError":null}],"nextCursor":null}}), &mut upstream, &mut client).unwrap();
        assert!(client.0.pop().unwrap().get("error").is_some());
        let identity = crate::codex_route::lock_route(&route)
            .unwrap()
            .identity
            .clone();
        assert!(identity.active);
        assert_eq!(identity.pane_id, "old-pane");
        for bad in [
            json!({}),
            json!({"result":{"data":[]}}),
            json!({"result":{"data":[],"nextCursor":"more"}}),
            json!({"result":{"data":[{"serverInfo":null,"runtimeStatus":"starting","tools":{},"toolsError":null}],"nextCursor":null}}),
        ] {
            state
                .start_attachment(request.clone(), &config, &mut upstream)
                .unwrap();
            upstream.0.pop();
            state
                .response(original.clone(), &mut upstream, &mut client)
                .unwrap();
            let inv = upstream.0.pop().unwrap();
            let mut bad = bad;
            bad["id"] = inv["id"].clone();
            state.response(bad, &mut upstream, &mut client).unwrap();
            assert!(client.0.pop().unwrap().get("error").is_some());
            assert_eq!(
                crate::codex_route::lock_route(&route)
                    .unwrap()
                    .identity
                    .pane_id,
                "old-pane"
            );
        }
        state
            .start_attachment(request.clone(), &config, &mut upstream)
            .unwrap();
        upstream.0.pop();
        state
            .response(original.clone(), &mut upstream, &mut client)
            .unwrap();
        let inv = upstream.0.pop().unwrap();
        state
            .response(
                json!({"id":inv["id"],"result":{"data":[],"nextCursor":null}}),
                &mut upstream,
                &mut client,
            )
            .unwrap();
        assert_eq!(client.0.pop(), Some(original));
        assert!(
            crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .active
        );
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn inventory_detects_legacy_name_and_complete_ivy_tool_set() {
        let inventory = |name: &str, tools: Value| json!({"result":{"data":[{"serverInfo":{"name":name},"runtimeStatus":"connected","tools":tools,"toolsError":null}],"nextCursor":null}});
        assert!(!inventory_excludes_ivy(&inventory(
            "agent-terminal panel",
            json!({})
        )));
        assert!(!inventory_excludes_ivy(&inventory(
            "renamed",
            json!({"panel_set":{},"panel_get":{},"split_open":{},"split_read":{},"split_close":{}})
        )));
        assert!(inventory_excludes_ivy(&inventory(
            "other",
            json!({"panel_get":{}})
        )));
    }

    #[test]
    fn exit_cancels_epoch_without_waiting_for_a_held_route() {
        let home = std::env::temp_dir().join(format!("ivy-cancel-exit-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let registry = Registry::for_home(&home).unwrap();
        let route = registry.pending().unwrap();
        let epoch = nonce().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        registry
            .bind(thread, &route, "old", &home.join("pane"), &epoch)
            .unwrap();
        let held = crate::codex_route::lock_route(&route).unwrap();
        let start = Instant::now();
        registry.cancel_epoch(&epoch).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "cancellation waited for the route"
        );
        drop(held);
        assert!(
            !crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .active
        );
        assert!(
            registry
                .bind(thread, &route, "late", &home.join("pane"), &epoch)
                .is_err()
        );
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn closing_console_cannot_reactivate_route_after_relay_is_abandoned() {
        let home = std::env::temp_dir().join(format!("ivy-exit-state-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let mut state = routing(&home);
        let route = state.registry.pending().unwrap();
        state.exit_route.lock().unwrap().closing = true;
        let mut peer = Peer::default();
        let original =
            json!({"id":9,"result":{"thread":{"id":"01234567-89ab-cdef-0123-456789abcdef"}}});
        state
            .finish(
                Attachment {
                    route: Some(route.clone()),
                    fresh: true,
                    servers: vec!["ivy".into()],
                    needs_inventory: false,
                },
                original,
                &mut peer,
            )
            .unwrap();
        assert!(peer.0[0].get("error").is_some());
        assert!(
            !crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .active
        );
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn unfinished_cleanup_thread_does_not_block_console() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        });
        let start = Instant::now();
        assert!(join_if_finished(handle, Duration::from_millis(20)).is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(tx);
    }

    #[test]
    fn server_version_must_be_exact() {
        assert!(verified_server(&json!({"appServerVersion":"0.167.2"})));
        for value in [
            json!({}),
            json!({"appServerVersion":null}),
            json!({"appServerVersion":17}),
            json!({"appServerVersion":"0.167.1"}),
        ] {
            assert!(!verified_server(&value));
        }
    }

    #[test]
    fn a_local_peer_without_the_bearer_cannot_complete_the_handshake() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            authenticated(stream, "private-test-capability").is_err()
        });
        let stream = TcpStream::connect(address).unwrap();
        let result = tungstenite::client(format!("ws://{address}/"), stream);
        assert!(
            matches!(result,Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(ref response))) if response.status()==401)
        );
        assert!(server.join().unwrap());
    }
    #[test]
    fn only_verified_invocations_use_the_shared_bridge() {
        for args in [
            vec![],
            vec!["resume", "--last"],
            vec!["fork", "--last", "-m", "model"],
            vec!["-C", "/tmp/work"],
        ] {
            assert!(supported_args(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        for args in [
            vec!["--no-daemon"],
            vec!["--remote", "unix://"],
            vec!["-c", "mcp_servers.x.command=foo"],
            vec!["-s", "read-only"],
            vec!["--profile", "other"],
        ] {
            assert!(!supported_args(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
    }
    #[test]
    fn only_the_existing_local_ivy_bridge_receives_a_route() {
        assert!(ivy_server(
            &json!({"command":"/opt/ivy/panel-mcp","env":{"existing":"kept"}})
        ));
        for server in [
            json!({"command":"/bin/other"}),
            json!({"command":"panel-mcp","enabled":false}),
            json!({"url":"https://example.org/mcp"}),
        ] {
            assert!(!ivy_server(&server));
        }
    }
}
