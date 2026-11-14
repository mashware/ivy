//! A console-local authenticated transport to the official Codex shared server.
//! Conversation ownership remains upstream; only this integration's runtime MCP route changes.

use crate::codex_route::Registry;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket, accept_hdr_with_config};

const MAX_FRAME: usize = 64 << 20;
const TOKEN_ENV: &str = "IVY_CODEX_RELAY_AUTH";
const CONFIG_ID: &str = "ivy-private-config";

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
            if forwarded.is_some_and(|at| at.elapsed() > Duration::from_secs(2)) {
                child.kill()?;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn nonce() -> io::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| error(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Unknown versions retain the existing independent-launch path until exercised.
pub fn supported(exe: &OsStr, args: &[OsString]) -> bool {
    if !supported_args(args) {
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
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
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
            let _ = child.kill();
            let _ = child.wait();
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

/// Does not enable remote control, restart an existing server, or edit user settings.
pub fn run(exe: &OsStr, args: &[OsString]) -> io::Result<ExitStatus> {
    #[cfg(unix)]
    let _signals = Signals::install()?;
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    let (id, dir) = crate::client::whole_identity(id.as_deref(), dir.as_deref())
        .ok_or_else(|| error("shared Codex launch requires this console's identity"))?;
    let registry = Registry::for_home(&home()?)?;
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
    if start
        .get("appServerVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.167.2")
    {
        return Err(error(
            "the running Codex server is not the verified version 0.167.2; update it explicitly or use --no-daemon",
        ));
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let token = nonce()?;
    let epoch = nonce()?;
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
            &epoch,
            &thread_stop,
        );
        if let Err(failure) = result {
            // Protocol payloads and capabilities are deliberately absent from diagnostics.
            eprintln!("ivy-codex: shared connection stopped ({failure})");
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
                let _ = child.kill();
                let _ = child.wait();
                Err(failure)
            }
        },
        Err(failure) => Err(failure),
    };
    stop.store(true, Ordering::Release);
    let _ = relay.join();
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
    child: Child,
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
            child,
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
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
type Upstream = WebSocket<ProxyStream>;

struct Attachment {
    route: PathBuf,
    fresh: bool,
}

struct Routing {
    registry: Registry,
    servers: Option<Map<String, Value>>,
    waiting: Vec<Value>,
    pending: HashMap<String, Attachment>,
    unknown_resumes: HashMap<String, Value>,
    active: Option<PathBuf>,
    epoch: String,
    pane_id: String,
    pane_dir: PathBuf,
    serial: u64,
}

impl Drop for Routing {
    fn drop(&mut self) {
        if let Some(route) = &self.active {
            let _ = self.registry.revoke(route, &self.epoch);
        }
        for attachment in self.pending.values().filter(|a| a.fresh) {
            let _ = std::fs::remove_file(&attachment.route);
        }
    }
}

fn write_upstream(output: &mut Upstream, value: &Value) -> io::Result<()> {
    output
        .send(Message::text(value.to_string()))
        .map_err(|_| error("official proxy disconnected"))
}

fn send(ws: &mut WebSocket<TcpStream>, value: &Value) -> io::Result<()> {
    ws.send(Message::text(value.to_string()))
        .map_err(|_| error("local client disconnected"))
}

fn reject(ws: &mut WebSocket<TcpStream>, id: &Value, message: &str) -> io::Result<()> {
    send(
        ws,
        &json!({"id":id,"error":{"code":-32001,"message":message}}),
    )
}

impl Routing {
    fn request(
        &mut self,
        mut message: Value,
        upstream: &mut Upstream,
        _ws: &mut WebSocket<TcpStream>,
    ) -> io::Result<()> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return write_upstream(upstream, &message);
        };
        if !matches!(method, "thread/start" | "thread/resume" | "thread/fork") {
            return write_upstream(upstream, &message);
        }
        let Some(id) = message.get("id").cloned() else {
            return write_upstream(upstream, &message);
        };
        if self.servers.is_none() {
            if self.waiting.len() >= 8 {
                return Err(error("too many pending conversation starts"));
            }
            self.waiting.push(message);
            return Ok(());
        }
        let thread = message.pointer("/params/threadId").and_then(Value::as_str);
        let existing = if method == "thread/resume" {
            thread
                .map(|thread| self.registry.route_for(thread))
                .transpose()?
                .flatten()
        } else {
            None
        };
        if method == "thread/resume" && existing.is_none() {
            // A pre-integration loaded conversation can ignore our new configuration.
            // Establish that it is cold before assigning any new capability.
            self.serial += 1;
            let internal = format!("ivy-private-loaded-{}", self.serial);
            self.unknown_resumes.insert(internal.clone(), message);
            return write_upstream(
                upstream,
                &json!({"id":internal,"method":"thread/loaded/list","params":{"limit":10000}}),
            );
        }
        let fresh = existing.is_none();
        let route = match existing {
            Some(route) => route,
            None => self.registry.pending()?,
        };
        self.inject(&mut message, &route)?;
        self.pending
            .insert(id.to_string(), Attachment { route, fresh });
        write_upstream(upstream, &message)
    }

    fn inject(&self, message: &mut Value, route: &Path) -> io::Result<()> {
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
        // config/read is a normalized read schema, not a valid input config
        // (for example optional timeout fields serialize as null). Merge only our
        // own leaf; Codex retains each existing command, environment and policy.
        let servers = config.entry("mcp_servers").or_insert_with(|| json!({}));
        let servers = servers
            .as_object_mut()
            .ok_or_else(|| error("invalid MCP overrides"))?;
        let mut found = false;
        for (name, server) in self.servers.as_ref().into_iter().flatten() {
            if !ivy_server(server) {
                continue;
            }
            let entry = servers.entry(name.clone()).or_insert_with(|| json!({}));
            let entry = entry
                .as_object_mut()
                .ok_or_else(|| error("invalid MCP server override"))?;
            let env = entry.entry("env").or_insert_with(|| json!({}));
            let env = env
                .as_object_mut()
                .ok_or_else(|| error("invalid MCP environment override"))?;
            env.insert(crate::codex_route::ENV.to_owned(), json!(route));
            found = true;
        }
        if !found {
            return Err(error(
                "configure the bundled panel-mcp stdio server before using shared Codex panel routing",
            ));
        }
        Ok(())
    }

    fn finish_attachment(&mut self, id: &Value, message: &Value) -> io::Result<()> {
        let Some(attachment) = self.pending.remove(&id.to_string()) else {
            return Ok(());
        };
        let Some(thread) = message.pointer("/result/thread/id").and_then(Value::as_str) else {
            if attachment.fresh {
                let _ = std::fs::remove_file(attachment.route);
            }
            return Ok(());
        };
        self.registry.bind(
            thread,
            &attachment.route,
            &self.pane_id,
            &self.pane_dir,
            &self.epoch,
        )?;
        let old = self.active.replace(attachment.route);
        if let Some(old) = old
            && self.active.as_ref() != Some(&old)
        {
            self.registry.revoke(&old, &self.epoch)?;
        }
        Ok(())
    }

    fn response(
        &mut self,
        mut message: Value,
        upstream: &mut Upstream,
        ws: &mut WebSocket<TcpStream>,
    ) -> io::Result<()> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if id == CONFIG_ID {
            self.servers = Some(
                message
                    .pointer("/result/config/mcp_servers")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or_else(|| error("Codex did not return the configured MCP servers"))?,
            );
            for request in std::mem::take(&mut self.waiting) {
                self.request(request, upstream, ws)?;
            }
            return Ok(());
        }
        if let Some(original) = id.as_str().and_then(|id| self.unknown_resumes.remove(id)) {
            let original_id = original.get("id").cloned().unwrap_or(Value::Null);
            let thread = original
                .pointer("/params/threadId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let data = message.pointer("/result/data").and_then(Value::as_array);
            let complete = message
                .pointer("/result/nextCursor")
                .is_none_or(Value::is_null);
            if data.is_none()
                || !complete
                || data.is_some_and(|data| data.iter().any(|id| id.as_str() == Some(thread)))
            {
                return reject(
                    ws,
                    &original_id,
                    "This conversation was opened before Ivy's shared-panel integration. Leave its active desktop session before resuming it here; no panel binding was changed.",
                );
            }
            let route = self.registry.pending()?;
            let mut original = original;
            self.inject(&mut original, &route)?;
            self.pending
                .insert(original_id.to_string(), Attachment { route, fresh: true });
            return write_upstream(upstream, &original);
        }
        if self.finish_attachment(&id, &message).is_err() {
            return reject(
                ws,
                &id,
                "The conversation opened, but Ivy could not bind its panel safely. No fallback panel was selected.",
            );
        }
        // Only a successful initialize response opens our private configuration read.
        if message.get("result").is_some() && message.get("ivyInitialize").is_some() {
            if let Some(object) = message.as_object_mut() {
                object.remove("ivyInitialize");
            }
            write_upstream(
                upstream,
                &json!({"id":CONFIG_ID,"method":"config/read","params":{"includeLayers":false,"cwd":std::env::current_dir()?}}),
            )?;
        }
        send(ws, &message)
    }
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
    stop: &AtomicBool,
) -> io::Result<()> {
    let mut routing = Routing {
        registry,
        servers: None,
        waiting: Vec::new(),
        pending: HashMap::new(),
        unknown_resumes: HashMap::new(),
        active: None,
        epoch: epoch.to_owned(),
        pane_id: pane_id.to_owned(),
        pane_dir: pane_dir.to_owned(),
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
        let stream = ProxyStream::open(exe, socket)?;
        let (mut upstream, _) = tungstenite::client("ws://localhost/", stream)
            .map_err(|_| error("official proxy WebSocket handshake failed"))?;
        upstream.get_mut().timeout = Duration::from_millis(10);
        let mut initialize_id = None;
        let result = (|| {
            while !stop.load(Ordering::Acquire) {
                loop {
                    match upstream.read() {
                        Ok(Message::Text(text)) => {
                            let mut message: Value = serde_json::from_str(&text)
                                .map_err(|_| error("invalid official protocol message"))?;
                            if initialize_id
                                .as_ref()
                                .is_some_and(|id| message.get("id") == Some(id))
                            {
                                initialize_id = None;
                                message["ivyInitialize"] = Value::Bool(true);
                            }
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
                match ws.read() {
                    Ok(Message::Text(text)) => {
                        let value: Value = serde_json::from_str(&text)
                            .map_err(|_| error("invalid local protocol message"))?;
                        if value
                            .get("id")
                            .and_then(Value::as_str)
                            .is_some_and(|id| id.starts_with("ivy-private-"))
                        {
                            return Err(error("local request uses a reserved correlation ID"));
                        }
                        if value.get("method").and_then(Value::as_str) == Some("initialize") {
                            initialize_id = value.get("id").cloned();
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
        result?;
        // The official TUI can reconnect to its local endpoint; the daemon stays alive.
        routing.servers = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routing() -> (PathBuf, Routing) {
        let home = std::env::temp_dir().join(format!("ivy-relay-test-{}", nonce().unwrap()));
        std::fs::create_dir(&home).unwrap();
        let state = Routing {
            registry: Registry::for_home(&home).unwrap(),
            servers: Some(json!({
                "custom-ivy-name": {"command":"/opt/ivy/panel-mcp","tool_timeout_sec":null,"env":{"KEEP":"value"}},
                "other": {"command":"/opt/other/server","tool_timeout_sec":null}
            }).as_object().unwrap().clone()),
            waiting: Vec::new(), pending:HashMap::new(), unknown_resumes:HashMap::new(),
            active:None, epoch:"test-epoch".to_owned(), pane_id:"new-console".to_owned(),
            pane_dir:home.join("new-pane"), serial:0,
        };
        (home, state)
    }

    #[test]
    fn normalized_readback_is_not_reinserted_as_input_configuration() {
        let (home, state) = routing();
        let route = state.registry.pending().unwrap();
        let mut message = json!({"params":{"config":{"features":{"existing":true}}}});
        state.inject(&mut message, &route).unwrap();
        assert_eq!(
            message["params"]["config"],
            json!({
                "features":{"existing":true},
                "mcp_servers":{"custom-ivy-name":{"env":{"IVY_CODEX_ROUTE":route}}}
            })
        );
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_failed_resume_preserves_the_existing_console_binding() {
        let (home, mut state) = routing();
        let route = state.registry.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        state
            .registry
            .bind(thread, &route, "original", &home.join("old"), "old-epoch")
            .unwrap();
        state.pending.insert(
            "17".to_owned(),
            Attachment {
                route: route.clone(),
                fresh: false,
            },
        );
        state
            .finish_attachment(
                &json!(17),
                &json!({"id":17,"error":{"code":-1,"message":"resume failed"}}),
            )
            .unwrap();
        let guard = crate::codex_route::lock_route(&route).unwrap();
        assert!(guard.identity.active);
        assert_eq!(guard.identity.pane_id, "original");
        drop(guard);
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn only_a_correlated_success_authorizes_the_pending_capability() {
        let (home, mut state) = routing();
        let route = state.registry.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        state.pending.insert(
            "17".to_owned(),
            Attachment {
                route: route.clone(),
                fresh: true,
            },
        );
        let response = json!({"result":{"thread":{"id":thread}}});
        state.finish_attachment(&json!(18), &response).unwrap();
        assert!(
            !crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .active
        );
        assert!(state.registry.route_for(thread).unwrap().is_none());
        state.finish_attachment(&json!(17), &response).unwrap();
        assert_eq!(
            state.registry.route_for(thread).unwrap(),
            Some(route.clone())
        );
        assert_eq!(
            crate::codex_route::lock_route(&route)
                .unwrap()
                .identity
                .pane_id,
            "new-console"
        );
        drop(state);
        std::fs::remove_dir_all(home).unwrap();
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
