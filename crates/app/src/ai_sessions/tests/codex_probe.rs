#![cfg(test)]
//! Explicitly gated installed-client probe. Console bytes stay in bounded, disposable memory.

use super::*;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const MESSAGE: &str = "Responde solo OK, sin usar herramientas ni modificar archivos";
const INTEGRATION: &str =
    include_str!("../../../../../integrations/session-recovery/codex/hooks.json");
const HOOK_KEY: &str = "/<session-flags>/config.toml:session_start:0:0";

#[test]
#[ignore = "metadata-only installed helper preflight; requires an explicit helper path"]
fn installed_codex_hook_stdout_preflight() {
    assert_eq!(std::env::var("IVY_TEST_PROBE_ISOLATED").as_deref(), Ok("1"));
    let helper = checked_helper();
    helper_stdout_preflight(&helper);
}

#[test]
#[ignore = "metadata-only official hooks/list; no thread or model is started"]
fn installed_codex_hook_trust_preflight() {
    assert_eq!(std::env::var("IVY_TEST_PROBE_ISOLATED").as_deref(), Ok("1"));
    let _helper = checked_helper();
    let cwd = owned_directory("IVY_TEST_PROBE_DIRECTORY");
    let launch = resolve_launch(Client::Codex).unwrap();
    let (overrides, hash) = hook_overrides();
    verify_hook_trust(&launch, &overrides, &hash, &cwd);
}

#[test]
#[ignore = "resume-only diagnosis of this probe's one used message; /status only, no model turn"]
fn installed_codex_resume_only_status_diagnostic() {
    assert_eq!(std::env::var("IVY_TEST_PROBE_ISOLATED").as_deref(), Ok("1"));
    let helper = checked_helper();
    let cwd = owned_directory("IVY_TEST_PROBE_DIRECTORY");
    assert!(
        cwd.file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("codex-smoke-"))
            && cwd
                .parent()
                .is_some_and(|parent| parent
                    .ends_with(".claude/work/19-restore-pane-directories-ai-sessions")),
        "resume-only input must belong to the explicit disposable project fixture"
    );
    let path = owned_destination("IVY_TEST_PROBE_SNAPSHOT");
    assert!(
        std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_file()
    );
    let file = std::fs::File::open(path).unwrap();
    assert!(file.metadata().unwrap().len() <= 4096);
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Capture {
        client: Client,
        session_id: String,
        cwd: PathBuf,
        assistant_ok: bool,
        model_messages: u8,
        resume_turn_submitted: bool,
        transient_config_overrides: u8,
        #[serde(rename = "exact_resume_status")]
        _previous_resume_status: bool,
    }
    let capture: Capture = serde_json::from_reader(file).unwrap();
    assert!(
        capture.client == Client::Codex
            && capture.model_messages == 1
            && capture.assistant_ok
            && !capture.resume_turn_submitted
            && capture.transient_config_overrides == 2
    );
    assert_eq!(capture.cwd, cwd);
    let identity = Identity::new(Client::Codex, capture.session_id).unwrap();
    let launch = resolve_launch(Client::Codex).unwrap();
    let (overrides, _) = hook_overrides();
    let mut resumed = Invocation::start(&launch, &overrides, &helper, &cwd, Some(&identity));
    resumed.ready();
    let initially_loading = resumed
        .screen
        .terminal
        .to_text()
        .contains("Resuming session");
    let previous = resumed.screen.plain().matches("Directory").count();
    resumed.local_status();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut confirmed = false;
    loop {
        resumed.receive();
        let plain = resumed.screen.plain();
        let compact: String = plain
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let current = plain.matches("Directory").count();
        let session_label = plain.contains("Session");
        let id_match = compact.contains(&identity.session_id);
        let composer = resumed.screen.composer_ready();
        let cwd_match = resumed.pty.working_directory().as_deref() == Some(cwd.as_path());
        if current > previous && session_label && id_match && composer && cwd_match {
            confirmed = true;
        }
        if confirmed || Instant::now() >= deadline {
            eprintln!(
                "Codex resume-only metadata: {}",
                json!({"previous_directory_count":previous,
                "current_directory_count":current,"session_label":session_label,"expected_id_match":id_match,
                "live_composer":composer,"native_cwd_match":cwd_match,"confirmed":confirmed,
                "initially_loading":initially_loading,
                "still_loading":resumed.screen.terminal.to_text().contains("Resuming session"),
                "own_status_visible":resumed.screen.terminal.to_text().contains("/status"),
                "waiting_for_startup":resumed.screen.terminal.to_text().contains("Waiting for startup")})
            );
            break;
        }
    }
    assert!(
        confirmed,
        "resume-only local status criteria failed; no model input was sent"
    );
}

#[test]
#[ignore = "one authorized installed Codex message; root must inspect and explicitly enable"]
fn installed_codex_one_message_capture_and_exact_resume() {
    assert_eq!(std::env::var("IVY_TEST_PROBE_ISOLATED").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("IVY_TEST_PROBE_CODEX_GO").as_deref(),
        Ok("ONE_AUTHORIZED_MESSAGE"),
        "the compiled one-message probe has not been explicitly released"
    );
    let helper = checked_helper();
    // This checks the actual helper's success stdout before any client or message is started.
    helper_stdout_preflight(&helper);
    let cwd = owned_directory("IVY_TEST_PROBE_DIRECTORY");
    let used = owned_destination("IVY_TEST_PROBE_MESSAGE_USED");
    let destination = owned_destination("IVY_TEST_PROBE_SNAPSHOT");
    assert!(!used.exists(), "the single message is already consumed");
    assert!(!destination.exists(), "the probe report already exists");
    let launch = resolve_launch(Client::Codex).expect("official Codex resolution failed");
    verify_official_auth(&launch, &cwd);
    let (overrides, hash) = hook_overrides();
    verify_hook_trust(&launch, &overrides, &hash, &cwd);
    let mut first = Invocation::start(&launch, &overrides, &helper, &cwd, None);
    first.ready();
    first.status(&cwd, None);
    assert_eq!(checked_helper(), helper, "the reviewed helper changed");
    consume_message(&mut first.pty, &used);
    let deadline = Instant::now() + Duration::from_secs(90);
    let identity = loop {
        assert!(
            Instant::now() < deadline,
            "one-message capture or reply failed; do not retry"
        );
        first.receive();
        let snapshot = first.handle.snapshot();
        if own_ok(&first.screen.plain()) && snapshot.status == Status::Registered {
            let identity = snapshot
                .identity
                .expect("native capture supplied no identity");
            assert_eq!(identity.client, Client::Codex);
            assert_eq!(snapshot.cwd.as_deref(), Some(cwd.as_path()));
            break identity;
        }
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&destination)
        .expect("cannot create the allowlisted probe report");
    serde_json::to_writer(
        &mut file,
        &json!({
            "client":"codex", "session_id":identity.session_id, "cwd":cwd,
            "assistant_ok":true, "exact_resume_status":false, "model_messages":1,
            "resume_turn_submitted":false, "transient_config_overrides":2
        }),
    )
    .unwrap();
    file.sync_all().unwrap();
    drop(first);
    let mut resumed = Invocation::start(&launch, &overrides, &helper, &cwd, Some(&identity));
    resumed.ready();
    resumed.status(&cwd, Some(&identity.session_id));
    // Startup may be unreported until a turn; an available report must still name this ID.
    assert!(
        resumed
            .handle
            .snapshot()
            .identity
            .as_ref()
            .is_none_or(|reported| reported == &identity),
        "the resumed client reported a different conversation"
    );
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).unwrap();
    file.set_len(0).unwrap();
    serde_json::to_writer(
        &mut file,
        &json!({
            "client":"codex", "session_id":identity.session_id, "cwd":cwd,
            "assistant_ok":true, "exact_resume_status":true, "model_messages":1,
            "resume_turn_submitted":false, "transient_config_overrides":2
        }),
    )
    .unwrap();
    file.sync_all().unwrap();
    eprintln!(
        "Codex probe message_used=true assistant_ok=true exact_resume_status=true model_messages=1"
    );
}

fn static_guard() -> String {
    let integration: Value = serde_json::from_str(INTEGRATION).unwrap();
    let groups = integration["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["hooks"].as_array().unwrap().len(), 1);
    let command = groups[0]["hooks"][0]["command"].as_str().unwrap();
    assert_eq!(groups[0]["hooks"][0]["type"], "command");
    assert!(command.contains("\"$IVY_SESSION_HELPER\" hook codex"));
    assert!(command.contains("$PANEL_DIR") && command.contains("$PANEL_ID"));
    assert!(!command.chars().any(char::is_control));
    command.to_owned()
}

fn hook_overrides() -> ([String; 2], String) {
    let command = static_guard();
    // 0.167.0 hashes normalized TOML as sorted JSON; None fields disappear in TOML.
    let mut normalized = json!({"event_name":"session_start","hooks":[{
        "type":"command", "command":command, "timeout":600, "async":false
    }]});
    normalized.sort_all_objects();
    let hash = format!(
        "sha256:{}",
        digest_bytes(&serde_json::to_vec(&normalized).unwrap())
    );
    let overrides = [
        format!(
            "hooks.SessionStart=[{{hooks=[{{type=\"command\",command={},timeout=600,async=false}}]}}]",
            serde_json::to_string(&command).unwrap()
        ),
        // 0.167.0 splits override paths on every dot, even inside quotes; quote the key in TOML data.
        format!("hooks.state={{\"{HOOK_KEY}\"={{trusted_hash=\"{hash}\"}}}}"),
    ];
    (overrides, hash)
}

fn checked_helper() -> PathBuf {
    let helper =
        PathBuf::from(std::env::var_os("IVY_TEST_PROBE_HELPER").expect("explicit helper required"));
    assert!(helper.is_absolute() && helper.is_file());
    let helper = std::fs::canonicalize(helper).unwrap();
    let expected =
        std::env::var("IVY_TEST_PROBE_HELPER_SHA256").expect("reviewed helper digest required");
    assert!(expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        digest_bytes(&std::fs::read(&helper).unwrap()),
        expected.to_ascii_lowercase(),
        "helper digest changed"
    );
    helper
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let digest = std::str::from_utf8(&output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    assert!(digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    digest.to_owned()
}

fn helper_stdout_preflight(helper: &Path) {
    let directory = PanelDir::create().unwrap();
    let (sender, _inbox) = sync_channel(0);
    // A reply-translation check only; this callback makes no claim about native foreground capture.
    let reached = Arc::new(AtomicBool::new(false));
    let acknowledge = Arc::clone(&reached);
    let handler: SessionHandler = Arc::new(move |_, request| {
        assert!(matches!(
            request,
            panel_control::session::Request::Observe {
                client: Client::Codex,
                ..
            }
        ));
        acknowledge.store(true, Ordering::Release);
        json!({"ok":true})
    });
    let _channel = Channel::open_with_services(
        directory.path(),
        directory.id(),
        sender,
        None,
        Some(handler),
        Arc::new(|| {}),
    )
    .unwrap();
    let mut child = Command::new("/bin/sh")
        .args(["-c", &static_guard()])
        .env("PANEL_ID", directory.id())
        .env("PANEL_DIR", directory.path())
        .env("IVY_SESSION_HELPER", helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&json!({
        "session_id":UUID, "cwd":"/tmp", "hook_event_name":"SessionStart", "source":"startup"
    })).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "the installed helper preflight failed"
    );
    assert!(
        reached.load(Ordering::Acquire),
        "the reviewed guard did not call the native helper"
    );
    assert!(
        codex_accepts_hook_stdout(&output.stdout),
        "production helper stdout violates Codex 0.167.0 SessionStart schema; no message was consumed"
    );
}

fn codex_accepts_hook_stdout(bytes: &[u8]) -> bool {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return true;
    }
    let Ok(Value::Object(fields)) = serde_json::from_slice(bytes) else {
        return false;
    };
    fields.iter().all(|(key, value)| match key.as_str() {
        "continue" | "suppressOutput" => value.is_boolean(),
        "stopReason" | "systemMessage" => value.is_string() || value.is_null(),
        "hookSpecificOutput" => value.as_object().is_some_and(|output| {
            output
                .get("hookEventName")
                .is_some_and(|event| event == "SessionStart")
                && output.iter().all(|(key, value)| {
                    key == "hookEventName"
                        || key == "additionalContext" && (value.is_string() || value.is_null())
                })
        }),
        _ => false,
    })
}

fn owned_directory(key: &str) -> PathBuf {
    let directory =
        PathBuf::from(std::env::var_os(key).expect("explicit owned fixture directory required"));
    assert!(directory.is_absolute() && directory.is_dir());
    let directory = std::fs::canonicalize(directory).unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkout = manifest
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new(".worktrees")))
        .and_then(Path::parent)
        .unwrap_or_else(|| manifest.parent().unwrap().parent().unwrap());
    let inherited_project = checkout.join(".claude/work/19-restore-pane-directories-ai-sessions");
    let project_fixture = directory.parent() == Some(inherited_project.as_path())
        && directory
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("codex-smoke-"));
    assert!(
        (directory.starts_with(std::env::temp_dir()) && directory != std::env::temp_dir())
            || project_fixture
    );
    directory
}

fn owned_destination(key: &str) -> PathBuf {
    let path =
        PathBuf::from(std::env::var_os(key).expect("explicit owned fixture destination required"));
    assert!(path.is_absolute());
    let parent = std::fs::canonicalize(path.parent().unwrap()).unwrap();
    assert!(parent.starts_with(std::env::temp_dir()) && parent != std::env::temp_dir());
    parent.join(path.file_name().unwrap())
}

fn removed_auth_environment() -> Vec<(String, Option<OsString>)> {
    std::env::vars_os()
        .filter_map(|(key, _)| {
            let key = key.to_string_lossy();
            (key.ends_with("_API_KEY")
                || [
                    "OPENAI_ACCESS_TOKEN",
                    "OPENAI_AUTH_TOKEN",
                    "CODEX_AUTH_TOKEN",
                    "CODEX_REMOTE_AUTH_TOKEN",
                ]
                .contains(&key.as_ref()))
            .then(|| (key.into_owned(), None))
        })
        .collect()
}

fn verify_official_auth(launch: &ResolvedLaunch, cwd: &Path) {
    for (arguments, expected) in [
        (vec!["--version"], "codex-cli 0.167.0"),
        (vec!["login", "status"], "Logged in using ChatGPT"),
    ] {
        let mut command = Command::new(&launch.program);
        command
            .args(&launch.prefix_args)
            .args(arguments)
            .env("PATH", &launch.path)
            .current_dir(cwd)
            .stdin(Stdio::null());
        for (key, _) in removed_auth_environment() {
            command.env_remove(key);
        }
        let output = command.output().expect("official metadata command failed");
        assert!(
            output.status.success(),
            "installed version or normal subscription auth check failed"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .chain(String::from_utf8_lossy(&output.stderr).lines())
                .any(|line| line.trim() == expected),
            "installed version or ChatGPT auth does not match"
        );
    }
}

fn verify_hook_trust(launch: &ResolvedLaunch, overrides: &[String; 2], hash: &str, cwd: &Path) {
    // 0.167.0 catalog_processor::hooks_list_response lists config without creating a thread.
    let script = r#"
import json,os,select,signal,subprocess,sys,time
x=json.load(sys.stdin); p=subprocess.Popen(x['argv'],cwd=x['cwd'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,start_new_session=True,bufsize=0)
deadline=time.monotonic()+15; total=0
def rpc(i,method,params):
 global total
 p.stdin.write((json.dumps({'id':i,'method':method,'params':params})+'\n').encode()); p.stdin.flush()
 while True:
  line=bytearray()
  while not line.endswith(b'\n'):
   remaining=deadline-time.monotonic()
   if remaining<=0 or not select.select([p.stdout],[],[],remaining)[0]: raise RuntimeError('timeout')
   byte=os.read(p.stdout.fileno(),1); total+=len(byte)
   if not byte or total>1048576: raise RuntimeError('response limit')
   line+=byte
  value=json.loads(line)
  if value.get('id')==i:
   if 'error' in value: raise RuntimeError('metadata error')
   return value['result']
try:
 rpc(1,'initialize',{'clientInfo':{'name':'ivy-hook-metadata-probe','version':'1'},'capabilities':{'experimentalApi':True}})
 p.stdin.write(b'{"method":"initialized"}\n'); p.stdin.flush()
 result=rpc(2,'hooks/list',{'cwds':[x['cwd']]})
 own=[h for entry in result.get('data',[]) if entry.get('cwd')==x['cwd'] for h in entry.get('hooks',[]) if h.get('key')==x['key']]
 h=own[0] if len(own)==1 else {}
 valid=len(own)==1 and h.get('currentHash')==x['hash'] and h.get('trustStatus')=='trusted' and h.get('enabled') is True and h.get('command')==x['command']
 print(json.dumps({'own_hook_trusted':valid,'own_count':len(own),'key_found':bool(own),'hash_matches':h.get('currentHash')==x['hash'],'trust_status':h.get('trustStatus'),'enabled':h.get('enabled'),'command_matches':h.get('command')==x['command'],'own_hash':h.get('currentHash')}))
finally:
 try: os.killpg(p.pid,signal.SIGKILL)
 except ProcessLookupError: pass
 p.wait()
"#;
    let mut argv = vec![launch.program.to_str().unwrap().to_owned()];
    argv.extend(launch.prefix_args.clone());
    argv.extend([
        "-c".into(),
        overrides[0].clone(),
        "-c".into(),
        overrides[1].clone(),
        "app-server".into(),
        "--stdio".into(),
    ]);
    let mut command = Command::new("/usr/bin/python3");
    command
        .args(["-c", script])
        .env("PATH", &launch.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, _) in removed_auth_environment() {
        command.env_remove(key);
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&json!({
                "argv":argv,"cwd":cwd,"key":HOOK_KEY,"command":static_guard(),
                "hash":hash
            }))
            .unwrap(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "metadata-only hooks/list preflight failed before input"
    );
    let filtered: Value =
        serde_json::from_slice(&output.stdout).expect("invalid filtered hook metadata");
    eprintln!("Codex own-hook metadata: {filtered}");
    assert_eq!(
        filtered["own_hook_trusted"], true,
        "Codex did not report the exact own hook hash as trusted/enabled; no message sent"
    );
}

struct Invocation {
    pty: Pty,
    output: Receiver<Vec<u8>>,
    handle: Handle,
    screen: Screen,
    _channel: Channel,
    _directory: PanelDir,
}

impl Invocation {
    fn start(
        launch: &ResolvedLaunch,
        overrides: &[String; 2],
        helper: &Path,
        cwd: &Path,
        identity: Option<&Identity>,
    ) -> Self {
        let directory = PanelDir::create().unwrap();
        let mut arguments = launch.prefix_args.clone();
        arguments.extend([
            "--no-daemon".into(),
            "-c".into(),
            overrides[0].clone(),
            "-c".into(),
            overrides[1].clone(),
        ]);
        if let Some(identity) = identity {
            arguments.extend(["resume".into(), identity.session_id.clone()]);
        }
        let args: Vec<_> = arguments.iter().map(String::as_str).collect();
        let mut environment = removed_auth_environment();
        environment.extend([
            ("PATH".into(), Some(launch.path.clone())),
            ("PANEL_ID".into(), Some(directory.id().into())),
            (
                "PANEL_DIR".into(),
                Some(directory.path().as_os_str().to_owned()),
            ),
            (
                "IVY_SESSION_HELPER".into(),
                Some(helper.as_os_str().to_owned()),
            ),
        ]);
        let environment: Vec<_> = environment
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_deref()))
            .collect();
        let (pty, output) = Pty::spawn_with_env_in(
            launch.program.to_str().unwrap(),
            &args,
            Size {
                cols: 160,
                rows: 44,
            },
            &environment,
            Some(cwd),
        )
        .unwrap();
        let handle = Handle::new(pty.foreground_scope().unwrap(), Arc::new(|| {}));
        handle.set_enabled(true);
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
        Self {
            pty,
            output,
            handle,
            screen: Screen::default(),
            _channel: channel,
            _directory: directory,
        }
    }

    fn receive(&mut self) {
        if let Ok(bytes) = self.output.recv_timeout(Duration::from_millis(50)) {
            self.screen.push(&bytes);
        }
        assert!(
            self.pty.exit_success().is_none(),
            "Codex exited; no automatic retry is allowed"
        );
    }

    fn ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(
                Instant::now() < deadline,
                "composer readiness unproven; no trust decision or message sent"
            );
            self.receive();
            let plain = self.screen.plain();
            assert!(
                !plain.contains("Folder access"),
                "directory trust would persist configuration; stopped before input"
            );
            if self.screen.composer_ready()
                && !self.screen.terminal.to_text().contains("Resuming session")
            {
                return;
            }
        }
    }

    fn status(&mut self, cwd: &Path, identity: Option<&str>) {
        let previous = self.screen.plain().matches("Directory").count();
        self.local_status();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                Instant::now() < deadline,
                "own local status did not confirm the expected metadata"
            );
            self.receive();
            let plain = self.screen.plain();
            let compact: String = plain
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            // /status is inline history in 0.167.0; its path is home-relative and may wrap.
            if plain.matches("Directory").count() > previous
                && self.screen.composer_ready()
                && identity.is_none_or(|id| plain.contains("Session") && compact.contains(id))
            {
                assert_eq!(
                    self.pty.working_directory().as_deref(),
                    Some(cwd),
                    "native foreground cwd differs from the owned fixture"
                );
                return;
            }
        }
    }

    fn local_status(&mut self) {
        self.pty
            .write(b"/status")
            .expect("local status input failed");
        // 0.167.0 folds an Enter within a fast input burst into the pasted text.
        std::thread::sleep(Duration::from_millis(200));
        self.pty
            .write(b"\r")
            .expect("local status submission failed");
    }
}

fn consume_message(pty: &mut Pty, used: &Path) {
    let mut ledger = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(used)
        .expect("single-message ledger exists or cannot be committed; do not retry");
    ledger
        .write_all(b"client=codex\nmessage_used=true\n")
        .unwrap();
    ledger.sync_all().unwrap();
    std::fs::File::open(used.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
    pty.write(format!("\x1b[200~{MESSAGE}\x1b[201~").as_bytes())
        .expect("message marked used but delivery failed; do not retry");
    std::thread::sleep(Duration::from_millis(100));
    pty.write(b"\r")
        .expect("message marked used but submission failed; do not retry");
}

struct Screen {
    terminal: term_core::Screen,
    received: usize,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            terminal: term_core::Screen::new(Size {
                cols: 160,
                rows: 44,
            }),
            received: 0,
        }
    }
}

impl Screen {
    fn push(&mut self, bytes: &[u8]) {
        assert!(
            self.received + bytes.len() <= 256 * 1024,
            "own fixture output ceiling exceeded; do not retry"
        );
        self.received += bytes.len();
        self.terminal.advance(bytes);
    }

    fn plain(&self) -> String {
        self.terminal.scrollback_text()
    }

    fn composer_ready(&self) -> bool {
        let view = self.terminal.view();
        let Some(cursor) = view.cursor() else {
            return false;
        };
        if cursor.shape == term_core::CursorShape::Hidden {
            return false;
        }
        let prefix: String = (0..cursor.col)
            .filter_map(|col| view.cell(cursor.row, col))
            .map(|cell| cell.c)
            .collect();
        prefix.trim_start().starts_with('›')
    }
}

fn own_ok(plain: &str) -> bool {
    plain.lines().any(|line| {
        line.trim()
            .strip_prefix('•')
            .is_some_and(|body| body.trim() == "OK")
    })
}

#[test]
fn codex_probe_filters_reply_schema_and_does_not_count_prompt_echo_as_ok() {
    assert!(!codex_accepts_hook_stdout(br#"{"ok":true}"#));
    assert!(codex_accepts_hook_stdout(b""));
    assert!(codex_accepts_hook_stdout(br#"{"continue":true}"#));
    assert!(!own_ok(MESSAGE));
    assert!(!own_ok("› OK"));
    assert!(own_ok("• OK"));
}

#[test]
fn codex_probe_readiness_requires_the_live_composer_cursor_not_history() {
    let mut screen = Screen::default();
    screen.push("\x1b[1;1H› old prompt\x1b[40;1H\x1b[?25h".as_bytes());
    assert!(!screen.composer_ready());
    screen.push("› ".as_bytes());
    assert!(screen.composer_ready());
    screen.push(b"\x1b[?25l");
    assert!(!screen.composer_ready());
}
