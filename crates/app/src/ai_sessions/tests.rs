#![cfg(test)]
use super::*;
use panel_contract::PanelDir;
use panel_control::Channel;
use panel_control::session::Client;
use serde_json::{Value, json};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{Receiver, sync_channel};
use term_core::{Pty, Size};

const UUID: &str = "11111111-1111-4111-8111-111111111111";
const CLAUDE_AUTHORIZED_MESSAGE: &str =
    "Responde solo OK, sin usar herramientas ni modificar archivos";
const ROOT_SCRIPT: &str = r#"
import os,sys,socket,json,termios,subprocess
a=termios.tcgetattr(0);a[3]&=~termios.ECHO;termios.tcsetattr(0,termios.TCSANOW,a)
endpoint=sys.argv[1]
helper='import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(sys.argv[2].encode()+b"\\n"); print(s.makefile().readline(),end="")'
def send(req,target):
 s=socket.socket(socket.AF_UNIX);s.connect(target);s.sendall(json.dumps(req).encode()+b'\n');r=json.loads(s.makefile().readline());s.close();return r
print(json.dumps({'ready':os.getpid()}),flush=True)
for line in sys.stdin:
 command=json.loads(line)
 if command.get('exit'):break
 if 'chdir' in command:os.chdir(command['chdir'])
 target=command.get('endpoint',endpoint)
 if command.get('helper'):
  r=subprocess.run(['/usr/bin/python3','-c',helper,endpoint,json.dumps(command['request'])],cwd=command.get('helper_cwd'),capture_output=True,text=True);print(json.dumps({'reply':json.loads(r.stdout)}),flush=True)
 elif command.get('nested'):
  r=subprocess.run([sys.executable,'-c',helper,endpoint,json.dumps(command['request'])],capture_output=True,text=True);print(json.dumps({'reply':json.loads(r.stdout)}),flush=True)
 elif command.get('detached'):
  r=subprocess.run(['/usr/bin/python3','-c',helper,target,json.dumps(command['request'])],start_new_session=True,capture_output=True,text=True);print(json.dumps({'reply':json.loads(r.stdout)}),flush=True)
 elif command.get('nested_detached'):
  r=subprocess.run([sys.executable,'-c',helper,target,json.dumps(command['request'])],start_new_session=True,capture_output=True,text=True);print(json.dumps({'reply':json.loads(r.stdout)}),flush=True)
 elif command.get('background_detached'):
  background='import sys,subprocess; r=subprocess.run(["/usr/bin/python3","-c",sys.argv[1],sys.argv[2],sys.argv[3]],start_new_session=True,capture_output=True,text=True); print(r.stdout,end="")'
  r=subprocess.run([sys.executable,'-c',background,helper,target,json.dumps(command['request'])],process_group=0,capture_output=True,text=True);print(json.dumps({'reply':json.loads(r.stdout)}),flush=True)
 else:print(json.dumps({'reply':send(command['request'],target)}),flush=True)
"#;

const OPENCODE_ROUTE_FIXTURE: &str = r#"
import { readFileSync, writeFileSync, renameSync } from "node:fs";
import { join, isAbsolute } from "node:path";

export default {
 id: "mashware.ivy-disposable-route-fixture",
 async tui(api) {
  const root = process.env.IVY_TEST_OPENCODE_CYCLE;
  if (process.env.IVY_TEST_PROBE_ISOLATED !== "1" || !root || !isAbsolute(root) || api.app.version !== "1.18.31") return;
  let stopped = false, timer, step = "ready";
  api.lifecycle.onDispose(() => { stopped = true; clearInterval(timer); });
  const save = (state) => {
   const temporary = join(root, "state.next");
   writeFileSync(temporary, JSON.stringify(state), { mode: 0o600 });
   renameSync(temporary, join(root, "state.json"));
  };
  const until = async (ready) => {
   const deadline = Date.now() + 10000;
   while (!stopped && !ready()) {
    if (Date.now() >= deadline) throw new Error("fixture_timeout");
    await new Promise(resolve => setTimeout(resolve, 20));
   }
   if (stopped) throw new Error("fixture_stopped");
  };
  void (async () => {
   await until(() => api.state.ready);
   const create = async (title) => {
    // Session creation emits local metadata; no prompt, init, message or model API is called.
    const result = await api.client.session.create({ directory: api.state.path.directory, title });
    const id = result?.data?.id;
    if (!/^ses_[A-Za-z0-9]+$/.test(id ?? "")) throw new Error("fixture_create_failed");
    return id;
   };
   step = "create_first";
   const first = await create("Ivy disposable route fixture one");
   step = "create_second";
   const second = await create("Ivy disposable route fixture two");
   let last;
   save({ first, second, phase: "ready" });
   timer = setInterval(() => {
    let phase;
    try { phase = JSON.parse(readFileSync(join(root, "control.json"), "utf8")).phase; } catch { return; }
    if (phase === last || !["first", "second", "home"].includes(phase)) return;
    if (phase === "home") api.route.navigate("home");
    else api.route.navigate("session", { sessionID: phase === "first" ? first : second });
    const current = api.route.current;
    const valid = phase === "home" ? current.name === "home" :
     current.name === "session" && current.params.sessionID === (phase === "first" ? first : second);
    if (!valid) return;
    last = phase;
    save({ first, second, phase });
   }, 50);
  })().catch(() => { if (!stopped) save({ error: "fixture_setup_failed", step }); });
 }
};
"#;

struct Fixture {
    handle: Handle,
    pty: Pty,
    output: Receiver<Vec<u8>>,
    buffered: Vec<u8>,
    wakes: Arc<AtomicUsize>,
    _channel: Channel,
    _executable: FixtureExecutable,
    _directories: FixtureDirectories,
    _directory: PanelDir,
}

struct FixtureExecutable(PathBuf);
struct FixtureDirectories(Vec<PathBuf>);

impl Drop for FixtureDirectories {
    fn drop(&mut self) {
        for directory in self.0.iter().rev() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

impl Drop for FixtureExecutable {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Fixture {
    fn new(client: Client) -> Self {
        Self::build(client, false)
    }

    fn build(client: Client, wrapped: bool) -> Self {
        let directory = PanelDir::create().unwrap();
        let executable = directory.path().join("fixture-client");
        std::fs::copy("/usr/bin/python3", &executable).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cleanup = FixtureExecutable(executable.clone());
        let endpoint = directory.path().join(panel_contract::CONTROL_SOCKET);
        let mut directories = FixtureDirectories(Vec::new());
        let (pty, output) = if wrapped {
            for name in ["shell-cwd", "client-cwd", "closing-cwd"] {
                let path = directory.path().join(name);
                std::fs::create_dir(&path).unwrap();
                directories.0.push(path);
            }
            Pty::spawn_with_env_in(
                "/usr/bin/python3",
                &[
                    "-c",
                    "import subprocess,sys; subprocess.run([sys.argv[1],'-c',sys.argv[4],sys.argv[2]],cwd=sys.argv[3])",
                    executable.to_str().unwrap(),
                    endpoint.to_str().unwrap(),
                    directories.0[1].to_str().unwrap(),
                    ROOT_SCRIPT,
                ],
                Size::default(),
                &[],
                Some(&directories.0[0]),
            )
        } else {
            Pty::spawn(
                executable.to_str().unwrap(),
                &["-c", ROOT_SCRIPT, endpoint.to_str().unwrap()],
                Size::default(),
            )
        }
        .unwrap();
        let wakes = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&wakes);
        let handle = Handle::with_programs(
            pty.foreground_scope().unwrap(),
            Arc::new(move || {
                counting.fetch_add(1, Ordering::Relaxed);
            }),
            vec![(client, executable)],
        );
        let (sender, _inbox) = sync_channel(0);
        let channel = Channel::open_with_services(
            directory.path(),
            directory.id(),
            sender,
            None,
            Some(handle.callback()),
            Arc::new(|| {}),
        )
        .unwrap();
        let mut fixture = Self {
            handle,
            pty,
            output,
            buffered: Vec::new(),
            wakes,
            _channel: channel,
            _executable: cleanup,
            _directories: directories,
            _directory: directory,
        };
        assert!(fixture.read()["ready"].is_u64());
        fixture
    }

    fn read(&mut self) -> Value {
        read(&self.output, &mut self.buffered)
    }
    fn command(&mut self, request: Value, mode: &str) -> Value {
        let mut command = json!({"request":request});
        if !mode.is_empty() {
            command[mode] = json!(true);
        }
        control(&mut self.pty, &self.output, &mut self.buffered, command)
    }
    fn request(&mut self, request: Value) -> Value {
        self.command(request, "")
    }
    fn attach(&mut self, client: &str) -> String {
        self.request(json!({"tool":"session_attach","client":client}))["generation"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn update(&mut self, generation: &str, sequence: u64, id: Option<&str>) -> Value {
        self.request(json!({"tool":"session_update","generation":generation,"sequence":sequence,"session_id":id,"cwd":"/tmp","challenge":null}))
    }
}

fn read(output: &Receiver<Vec<u8>>, buffered: &mut Vec<u8>) -> Value {
    loop {
        if let Some(end) = buffered.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = buffered.drain(..=end).collect();
            if let Ok(json) = serde_json::from_slice(&line) {
                return json;
            }
        } else {
            buffered.extend(output.recv_timeout(Duration::from_secs(5)).unwrap());
        }
    }
}

fn exchange(
    pty: &mut Pty,
    output: &Receiver<Vec<u8>>,
    buffered: &mut Vec<u8>,
    request: Value,
) -> Value {
    control(pty, output, buffered, json!({"request":request}))
}

fn control(
    pty: &mut Pty,
    output: &Receiver<Vec<u8>>,
    buffered: &mut Vec<u8>,
    command: Value,
) -> Value {
    let mut line = serde_json::to_vec(&command).unwrap();
    line.push(b'\n');
    pty.write(&line).unwrap();
    read(output, buffered)["reply"].clone()
}

#[test]
fn claude_null_directory_uses_the_verified_client_not_its_shell_or_helper() {
    let mut f = Fixture::build(Client::Claude, true);
    let shell = f._directory.path().join("shell-cwd");
    let client = f._directory.path().join("client-cwd");
    let closing = f._directory.path().join("closing-cwd");
    assert_eq!(f.pty.working_directory().as_deref(), Some(shell.as_path()));
    f.handle.set_enabled(true);
    let generation = f.attach("claude");
    assert_eq!(
        control(
            &mut f.pty,
            &f.output,
            &mut f.buffered,
            json!({"helper":true,"helper_cwd":shell,"request":{"tool":"session_update","generation":generation,"sequence":1,"session_id":UUID,"cwd":null,"challenge":null}}),
        )["ok"],
        true
    );
    assert_eq!(f.handle.snapshot().cwd.as_deref(), Some(client.as_path()));
    assert_eq!(f.pty.working_directory().as_deref(), Some(shell.as_path()));
    let frozen = std::thread::scope(|scope| {
        let handle = &f.handle;
        let freezing = scope.spawn(move || freeze_all(&[handle], Duration::from_millis(1250)));
        let deadline = Instant::now() + Duration::from_secs(3);
        let challenge = loop {
            assert!(
                Instant::now() < deadline,
                "close challenge was not published"
            );
            let reply = exchange(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"tool":"session_probe","generation":generation}),
            );
            if let Some(challenge) = reply["challenge"].as_str() {
                break challenge.to_owned();
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(
            control(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"chdir":closing,"helper":true,"helper_cwd":shell,"request":{"tool":"session_update","generation":generation,"sequence":2,"session_id":UUID,"cwd":null,"challenge":challenge}}),
            )["ok"],
            true
        );
        freezing.join().unwrap()
    });
    assert_eq!(frozen[0].cwd.as_deref(), Some(closing.as_path()));
    assert!(frozen[0].identity.is_some());
    assert_eq!(f.pty.working_directory().as_deref(), Some(shell.as_path()));
}

#[test]
fn unavailable_claude_root_directory_revokes_the_verified_identity() {
    let mut f = Fixture::build(Client::Claude, true);
    f.handle.set_enabled(true);
    let generation = f.attach("claude");
    let update = |sequence| json!({"tool":"session_update","generation":generation,"sequence":sequence,"session_id":UUID,"cwd":null,"challenge":null});
    assert_eq!(f.request(update(1))["ok"], true);
    std::fs::remove_dir(f._directory.path().join("client-cwd")).unwrap();
    assert_eq!(f.request(update(2))["error"], "unavailable_directory");
    let snapshot = f.handle.snapshot();
    assert!(snapshot.identity.is_none() && !snapshot.sampled);
    assert_eq!(snapshot.status, Status::Waiting);
    assert_eq!(f.request(update(3))["error"], "stale_generation");
}

#[test]
fn native_session_generations_sequences_helpers_and_idle_probes() {
    let mut f = Fixture::new(Client::Claude);
    assert_eq!(
        f.request(json!({"tool":"session_attach","client":"claude"}))["error"],
        "disabled"
    );
    f.handle.set_enabled(true);
    let generation = f.attach("claude");
    assert!(!f.handle.snapshot().sampled, "attach is not a live sample");
    assert_eq!(f.update(&generation, 1, Some(UUID))["ok"], true);
    assert_eq!(f.handle.snapshot().identity.unwrap().session_id, UUID);
    let before = f.wakes.load(Ordering::Relaxed);
    for _ in 0..3 {
        assert!(
            f.request(json!({"tool":"session_probe","generation":generation}))["ok"]
                .as_bool()
                .unwrap()
        );
    }
    assert_eq!(
        f.wakes.load(Ordering::Relaxed),
        before,
        "unchanged probes cannot wake the UI"
    );
    assert_eq!(f.update(&generation, 1, None)["error"], "stale_sequence");
    let probe = json!({"tool":"session_probe","generation":generation});
    assert_eq!(
        f.command(probe.clone(), "helper")["ok"],
        true,
        "a short-lived official helper retains root provenance"
    );
    assert_eq!(
        f.command(probe, "nested")["error"],
        "unauthorized",
        "a second native client cannot report for its parent"
    );
    assert!(
        f.handle.snapshot().identity.is_some(),
        "helper exit must not erase root registration"
    );
    let next = f.attach("claude");
    assert_ne!(generation, next);
    assert!(f.handle.snapshot().identity.is_none());
    assert_eq!(
        f.update(&generation, 2, Some(UUID))["error"],
        "stale_generation"
    );
    let before_sample = f.wakes.load(Ordering::Relaxed);
    let before_revision = f.handle.snapshot().revision;
    assert_eq!(f.update(&next, 1, None)["ok"], true);
    assert!(
        f.handle.snapshot().sampled,
        "an explicit null update is a live sample"
    );
    assert_eq!(f.wakes.load(Ordering::Relaxed), before_sample + 1);
    assert!(
        f.handle
            .snapshot_if_changed(before_revision)
            .unwrap()
            .sampled
    );
    let after_sample = f.wakes.load(Ordering::Relaxed);
    assert_eq!(f.update(&next, 2, None)["ok"], true);
    assert_eq!(
        f.wakes.load(Ordering::Relaxed),
        after_sample,
        "unchanged live samples do not wake the UI"
    );
    f.handle.set_enabled(false);
    assert!(f.handle.snapshot().identity.is_none());
}

#[test]
fn native_root_exit_invalidates_once_and_codex_keeps_last_accepted_report() {
    let mut f = Fixture::new(Client::Codex);
    f.handle.set_enabled(true);
    let observe = |id| json!({"tool":"session_observe","client":"codex","session_id":id,"cwd":"/tmp","event":"SessionStart","source":"resume"});
    assert_eq!(f.request(observe(UUID))["ok"], true);
    let second = "22222222-2222-4222-8222-222222222222";
    assert_eq!(f.request(observe(second))["ok"], true);
    assert_eq!(f.handle.snapshot().identity.unwrap().session_id, second);
    assert_eq!(f.request(json!({"tool":"session_observe","client":"codex","session_id":UUID,"cwd":"/tmp","event":"SessionEnd","source":"exit"}))["ok"], true);
    assert_eq!(
        f.handle.snapshot().identity.unwrap().session_id,
        second,
        "a delayed old SessionEnd is advisory"
    );
    let before = f.wakes.load(Ordering::Relaxed);
    f.pty.write(b"{\"exit\":true}\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while f.handle.snapshot().identity.is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(f.handle.snapshot().identity.is_none());
    assert_eq!(f.wakes.load(Ordering::Relaxed), before + 1);
}

#[test]
fn native_close_uses_one_deadline_and_strips_unacknowledged_live_clients() {
    let mut a = Fixture::new(Client::Claude);
    a.handle.set_enabled(true);
    let generation = a.attach("claude");
    a.update(&generation, 1, Some(UUID));
    let mut b = Fixture::new(Client::Opencode);
    b.handle.set_enabled(true);
    let generation = b.attach("opencode");
    b.update(&generation, 1, Some("ses_exact"));
    let start = Instant::now();
    let frozen = freeze_all(&[&a.handle, &b.handle], Duration::from_millis(80));
    assert!(
        start.elapsed() < Duration::from_millis(145),
        "two panes must share one close deadline"
    );
    assert!(frozen.iter().all(|snapshot| snapshot.identity.is_none()
        && snapshot.cwd.as_deref() == Some(std::path::Path::new("/tmp"))));
}

#[test]
fn native_close_accepts_only_a_post_probe_ack_and_retains_the_frozen_sample() {
    let mut f = Fixture::new(Client::Claude);
    f.handle.set_enabled(true);
    let generation = f.attach("claude");
    f.update(&generation, 1, Some(UUID));
    let second = "22222222-2222-4222-8222-222222222222";
    let frozen = std::thread::scope(|scope| {
        let handle = &f.handle;
        let closing = scope.spawn(move || freeze_all(&[handle], Duration::from_millis(1250)));
        let challenge = loop {
            let reply = exchange(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"tool":"session_probe","generation":generation}),
            );
            if let Some(challenge) = reply["challenge"].as_str() {
                break challenge.to_owned();
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(
            exchange(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"tool":"session_update","generation":generation,"sequence":2,"session_id":second,"cwd":null,"challenge":challenge})
            )["ok"],
            true
        );
        closing.join().unwrap()
    });
    assert_eq!(frozen[0].identity.as_ref().unwrap().session_id, second);
    f.pty.write(b"{\"exit\":true}\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while f.pty.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        f.handle.snapshot().identity.as_ref().unwrap().session_id,
        second
    );
}

#[test]
fn native_session_endpoint_rejects_a_reporter_from_another_real_pane() {
    let mut a = Fixture::new(Client::Claude);
    a.handle.set_enabled(true);
    let b = Fixture::new(Client::Claude);
    b.handle.set_enabled(true);
    let endpoint = b._directory.path().join(panel_contract::CONTROL_SOCKET);
    let mut line = serde_json::to_vec(
        &json!({"request":{"tool":"session_attach","client":"claude"},"endpoint":endpoint}),
    )
    .unwrap();
    line.push(b'\n');
    a.pty.write(&line).unwrap();
    assert_eq!(a.read()["reply"]["error"], "unauthorized");
    assert!(b.handle.snapshot().identity.is_none());
}

#[test]
fn detached_reporters_require_their_own_current_foreground_client_root() {
    let mut pane = Fixture::new(Client::Claude);
    pane.handle.set_enabled(true);
    let attach = json!({"tool":"session_attach","client":"claude"});
    let attached = pane.command(attach.clone(), "detached");
    assert_eq!(attached["ok"], true);
    let generation = attached["generation"].as_str().unwrap();
    let update = json!({"tool":"session_update","generation":generation,"sequence":1,"session_id":UUID,"cwd":"/tmp","challenge":null});
    assert_eq!(pane.command(update, "detached")["ok"], true);
    assert!(pane.handle.snapshot().sampled && pane.handle.snapshot().identity.is_some());
    assert_eq!(
        pane.command(attach.clone(), "nested_detached")["error"],
        "unauthorized"
    );
    assert_eq!(
        pane.command(attach.clone(), "background_detached")["error"],
        "unauthorized"
    );
    let foreign = Fixture::new(Client::Claude);
    foreign.handle.set_enabled(true);
    let mut request = serde_json::to_vec(&json!({"request":attach,"detached":true,"endpoint":foreign._directory.path().join(panel_contract::CONTROL_SOCKET)})).unwrap();
    request.push(b'\n');
    pane.pty.write(&request).unwrap();
    assert_eq!(pane.read()["reply"]["error"], "unauthorized");
    assert!(foreign.handle.snapshot().identity.is_none());
}

#[test]
fn native_close_rejects_an_old_challenge_even_when_the_sequence_increases() {
    let mut f = Fixture::new(Client::Claude);
    f.handle.set_enabled(true);
    let generation = f.attach("claude");
    f.update(&generation, 1, Some(UUID));
    let frozen = std::thread::scope(|scope| {
        let handle = &f.handle;
        let closing = scope.spawn(move || freeze_all(&[handle], Duration::from_millis(180)));
        loop {
            let reply = exchange(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"tool":"session_probe","generation":generation}),
            );
            if reply["challenge"].is_string() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            exchange(
                &mut f.pty,
                &f.output,
                &mut f.buffered,
                json!({"tool":"session_update","generation":generation,"sequence":2,"session_id":UUID,"cwd":"/tmp","challenge":"old_challenge"})
            )["ok"],
            true
        );
        closing.join().unwrap()
    });
    assert!(frozen[0].identity.is_none());
    assert_eq!(frozen[0].cwd.as_deref(), Some(std::path::Path::new("/tmp")));
}

#[test]
#[ignore = "requires an explicitly isolated official-client configuration and no model input"]
fn installed_session_client_probe() {
    assert_eq!(std::env::var("IVY_TEST_PROBE_ISOLATED").as_deref(), Ok("1"));
    let client =
        Client::parse(&std::env::var("IVY_TEST_PROBE_CLIENT").expect("explicit client")).unwrap();
    let launch = resolve_launch(client).unwrap();
    let helper =
        PathBuf::from(std::env::var_os("IVY_TEST_PROBE_HELPER").expect("explicit absolute helper"));
    assert!(helper.is_absolute() && helper.is_file());
    let helper = std::fs::canonicalize(helper).unwrap();
    let directory = PanelDir::create().unwrap();
    let mut arguments = launch.prefix_args;
    let mut expected = std::env::var("IVY_TEST_PROBE_SESSION")
        .ok()
        .map(|id| Identity::new(client, id).unwrap());
    let mut expect_session =
        expected.is_some() || std::env::var("IVY_TEST_PROBE_EXPECT_SESSION").as_deref() == Ok("1");
    if let Some(identity) = &expected {
        arguments.extend(crate::resume::arguments(identity));
    } else if client == Client::Codex {
        arguments.push("--no-daemon".into());
    }
    if client == Client::Claude {
        let plugin = PathBuf::from(
            std::env::var_os("IVY_TEST_PROBE_CLAUDE_PLUGIN").expect("explicit Claude probe plugin"),
        );
        assert!(plugin.is_absolute() && plugin.is_dir());
        let plugin = std::fs::canonicalize(plugin).unwrap();
        arguments.extend([
            "--setting-sources".into(),
            String::new(),
            "--plugin-dir".into(),
            plugin.to_str().unwrap().to_owned(),
        ]);
    }
    let cwd = std::env::var_os("IVY_TEST_PROBE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory.path().to_owned());
    assert!(cwd.is_absolute() && cwd.is_dir());
    let cwd = std::fs::canonicalize(cwd).unwrap();
    assert!(cwd.starts_with(std::env::temp_dir()) && cwd != std::env::temp_dir());
    let cycle = std::env::var_os("IVY_TEST_PROBE_OPENCODE_CYCLE").map(PathBuf::from);
    let cycle_config = cycle.as_ref().map(|root| {
        assert!(client == Client::Opencode && expected.is_none());
        prepare_opencode_cycle(root)
    });
    let args: Vec<_> = arguments.iter().map(String::as_str).collect();
    let mut env = vec![
        ("PATH", Some(launch.path.as_os_str())),
        ("PANEL_ID", Some(std::ffi::OsStr::new(directory.id()))),
        ("PANEL_DIR", Some(directory.path().as_os_str())),
        ("IVY_SESSION_HELPER", Some(helper.as_os_str())),
    ];
    if let (Some(root), Some(config)) = (&cycle, &cycle_config) {
        env.extend([
            ("IVY_TEST_OPENCODE_CYCLE", Some(root.as_os_str())),
            ("OPENCODE_TUI_CONFIG", Some(config.as_os_str())),
        ]);
    }
    let (mut pty, output) = Pty::spawn_with_env_in(
        launch.program.to_str().unwrap(),
        &args,
        Size::default(),
        &env,
        Some(&cwd),
    )
    .unwrap();
    let handle = Handle::new(pty.foreground_scope().unwrap(), Arc::new(|| {}));
    handle.set_enabled(true);
    let one_message = std::env::var("IVY_TEST_PROBE_CLAUDE_ONE_MESSAGE").as_deref() == Ok("1");
    let callback = handle.callback();
    let diagnostic_callback: SessionHandler = Arc::new(move |peer, request| {
        let reply = callback(peer, request);
        if one_message && reply["ok"] != true {
            let reason = reply["error"]
                .as_str()
                .filter(|reason| {
                    [
                        "unauthorized",
                        "disabled",
                        "unsupported",
                        "busy",
                        "invalid_request",
                        "stale_generation",
                        "stale_sequence",
                        "closing",
                        "unavailable",
                    ]
                    .contains(reason)
                })
                .unwrap_or("other");
            eprintln!("Claude probe metadata request rejected reason={reason}");
        }
        reply
    });
    let (sender, _inbox) = sync_channel(0);
    let _channel = Channel::open_with_services(
        directory.path(),
        directory.id(),
        sender,
        None,
        Some(diagnostic_callback),
        Arc::new(|| {}),
    )
    .unwrap();
    eprintln!(
        "native probe pane={} endpoint={}",
        directory.id(),
        directory
            .path()
            .join(panel_contract::CONTROL_SOCKET)
            .display()
    );
    if let Some(root) = &cycle {
        expected = Some(drive_opencode_cycle(&handle, &output, root));
        expect_session = true;
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut snapshot = handle.snapshot();
    while (snapshot.status != Status::Registered || expect_session && snapshot.identity.is_none())
        && Instant::now() < deadline
    {
        // The output is drained without interpreting or logging the client's private console.
        let _ = output.recv_timeout(Duration::from_millis(20));
        snapshot = handle.snapshot();
    }
    if one_message {
        eprintln!(
            "Claude probe readiness registered={} sampled={} has_id={} exited={}",
            snapshot.status == Status::Registered,
            snapshot.sampled,
            snapshot.identity.is_some(),
            pty.exit_success().is_some()
        );
        assert!(client == Client::Claude && expected.is_none());
        for (key, _) in std::env::vars_os() {
            let key = key.to_string_lossy();
            assert!(
                !key.ends_with("_API_KEY")
                    && !key.starts_with("ANTHROPIC_")
                    && !key.starts_with("CLAUDE_CODE_USE_")
                    && key != "CLAUDE_CODE_OAUTH_TOKEN",
                "a provider/auth environment override is present"
            );
        }
        assert_eq!(
            std::env::var("IVY_TEST_PROBE_CLAUDE_AUTH").as_deref(),
            Ok("claude.ai")
        );
        assert!(
            snapshot.status == Status::Registered
                && snapshot.sampled
                && snapshot.identity.is_some()
        );
        let used = PathBuf::from(
            std::env::var_os("IVY_TEST_PROBE_MESSAGE_USED").expect("one-message usage ledger"),
        );
        send_one_authorized_claude_message(&mut pty, &output, &used);
    }
    let destination = std::env::var_os("IVY_TEST_PROBE_SNAPSHOT");
    if destination.is_some() {
        snapshot = freeze_all(&[&handle], Duration::from_millis(1250))
            .into_iter()
            .next()
            .expect("one frozen probe snapshot");
    }
    eprintln!(
        "native probe client={} status={:?} declared_session={}",
        client.name(),
        snapshot.status,
        snapshot.identity.is_some()
    );
    if let Some(path) = destination {
        let path = PathBuf::from(path);
        assert!(path.is_absolute());
        let parent = std::fs::canonicalize(path.parent().unwrap()).unwrap();
        assert!(parent.starts_with(std::env::temp_dir()) && parent != std::env::temp_dir());
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        let recorded_cwd = snapshot.cwd.clone().or_else(|| pty.working_directory());
        assert!(
            recorded_cwd
                .as_ref()
                .is_none_or(|recorded| recorded == &cwd),
            "the probe left its disposable directory"
        );
        serde_json::to_writer(file, &json!({"client":client,"session_id":snapshot.identity.as_ref().map(|identity|&identity.session_id),"cwd":recorded_cwd})).unwrap();
    }
    assert_eq!(
        snapshot.status,
        Status::Registered,
        "the installed producer did not register through its native foreground root"
    );
    if expect_session {
        assert!(
            snapshot.identity.is_some(),
            "the installed live getter did not publish a session"
        );
    }
    if let Some(expected) = expected {
        assert!(
            snapshot.identity == Some(expected),
            "the client did not declare the exact requested disposable session"
        );
    }
    drop(pty);
}

fn send_one_authorized_claude_message(
    pty: &mut Pty,
    output: &Receiver<Vec<u8>>,
    used: &std::path::Path,
) {
    use std::io::Write;
    assert!(used.is_absolute());
    let parent = std::fs::canonicalize(used.parent().unwrap()).unwrap();
    assert!(parent.starts_with(std::env::temp_dir()) && parent != std::env::temp_dir());
    let mut ledger = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(used)
        .expect("authorized message has already been used or cannot be marked");
    ledger.write_all(b"message_used=true\n").unwrap();
    ledger.sync_all().unwrap();
    std::fs::File::open(parent).unwrap().sync_all().unwrap();
    let paste = format!("\u{1b}[200~{CLAUDE_AUTHORIZED_MESSAGE}\u{1b}[201~");
    pty.write(paste.as_bytes())
        .expect("the single marked message was not delivered; do not retry");
    std::thread::sleep(Duration::from_millis(100));
    pty.write(b"\r")
        .expect("the single marked message was not submitted; do not retry");
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut own_response = Vec::new();
    loop {
        assert!(
            Instant::now() < deadline,
            "no expected assistant OK observed after the single message; do not send another"
        );
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(50)) {
            own_response.extend(bytes);
            assert!(
                own_response.len() <= 256 * 1024,
                "single-turn probe output ceiling reached; do not retry"
            );
            if own_assistant_ok(&own_response) {
                break;
            }
        }
    }
    eprintln!("authorized Claude message_used=true assistant_ok_observed=true");
}

fn own_assistant_ok(bytes: &[u8]) -> bool {
    let mut plain = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"\x1b[") {
            index += 2;
            while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                index += 1;
            }
            if index < bytes.len() && matches!(bytes[index], b'H' | b'f' | b'E' | b'F') {
                plain.push(b'\n');
            }
        } else if bytes[index..].starts_with(b"\x1b]") {
            index += 2;
            while index < bytes.len() && bytes[index] != 7 && !bytes[index..].starts_with(b"\x1b\\")
            {
                index += 1;
            }
            if bytes.get(index) == Some(&27) {
                index += 1;
            }
        } else {
            plain.push(if bytes[index] == b'\r' {
                b'\n'
            } else {
                bytes[index]
            });
        }
        index += 1;
    }
    let mut marker = false;
    for line in String::from_utf8_lossy(&plain).lines() {
        let line = line.trim();
        if let Some(body) = line.strip_prefix(['⏺', '●', '•']) {
            if body.trim() == "OK" {
                return true;
            }
            marker = body.trim().is_empty();
        } else if marker && line == "OK" {
            return true;
        } else if !line.is_empty() {
            marker = false;
        }
    }
    false
}

#[test]
fn single_claude_reply_classifier_requires_an_assistant_marker_not_the_prompt_echo() {
    assert!(!own_assistant_ok(
        format!("❯ {CLAUDE_AUTHORIZED_MESSAGE}\r\n").as_bytes()
    ));
    assert!(own_assistant_ok("\u{1b}[1m⏺\u{1b}[0m OK\r\n".as_bytes()));
    assert!(own_assistant_ok("●\u{1b}[3;1HOK\r\n".as_bytes()));
    assert!(!own_assistant_ok("❯ OK\r\n".as_bytes()));
}

fn prepare_opencode_cycle(root: &std::path::Path) -> PathBuf {
    assert!(root.is_absolute() && root.is_dir());
    let root = std::fs::canonicalize(root).unwrap();
    assert!(root.starts_with(std::env::temp_dir()) && root != std::env::temp_dir());
    let fixture = root.join("route-fixture.js");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&fixture)
        .unwrap();
    std::io::Write::write_all(&mut file, OPENCODE_ROUTE_FIXTURE.as_bytes()).unwrap();
    let production = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/session-recovery/opencode/index.js"),
    )
    .unwrap();
    let config = root.join("tui.json");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&config)
        .unwrap();
    serde_json::to_writer(file, &json!({"plugin":[production,fixture]})).unwrap();
    config
}

fn read_cycle_state(root: &std::path::Path) -> Option<Value> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(root.join("state.json"))
        .ok()?
        .take(8193)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8192 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn drive_opencode_cycle(
    handle: &Handle,
    output: &Receiver<Vec<u8>>,
    root: &std::path::Path,
) -> Identity {
    let deadline = Instant::now() + Duration::from_secs(15);
    let state = loop {
        if let Some(state) = read_cycle_state(root) {
            if state.get("error").is_some() {
                let step = state["step"]
                    .as_str()
                    .filter(|step| {
                        ["ready", "create_first", "create_second", "store_sync"].contains(step)
                    })
                    .unwrap_or("unknown");
                panic!("the official SDK fixture could not initialize at {step}");
            }
            if state["phase"] == "ready" {
                break state;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the official SDK fixture did not publish its own disposable metadata"
        );
        let _ = output.recv_timeout(Duration::from_millis(20));
    };
    let first = Identity::new(
        Client::Opencode,
        state["first"].as_str().expect("first fixture identity"),
    )
    .unwrap();
    let second = Identity::new(
        Client::Opencode,
        state["second"].as_str().expect("second fixture identity"),
    )
    .unwrap();
    assert!(
        first != second,
        "the official SDK must create two distinct disposable sessions"
    );
    for (phase, expected) in [
        ("first", Some(&first)),
        ("second", Some(&second)),
        ("home", None),
        ("first", Some(&first)),
    ] {
        std::fs::write(
            root.join("control.json"),
            serde_json::to_vec(&json!({"phase":phase})).unwrap(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let snapshot = handle.snapshot();
            let selected = read_cycle_state(root).is_some_and(|state| state["phase"] == phase);
            if selected
                && snapshot.sampled
                && snapshot.status == Status::Registered
                && snapshot.identity.as_ref() == expected
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the production reporter did not follow the official disposable route change"
            );
            let _ = output.recv_timeout(Duration::from_millis(20));
        }
        eprintln!("native OpenCode route fixture accepted phase={phase}");
    }
    first
}

mod codex_probe;
