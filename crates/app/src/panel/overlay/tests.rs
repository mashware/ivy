use super::*;
use panel_contract::PanelDir;
use panel_control::session::Client;
use panel_control::{Answer, Channel};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};
use term_core::{Pty, Size};

const CLIENT: &str = r#"
import os,sys,json,socket,subprocess,termios
a=termios.tcgetattr(0);a[3]&=~termios.ECHO;termios.tcsetattr(0,termios.TCSANOW,a)
endpoint=sys.argv[1]
helper='import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(sys.argv[2].encode()+b"\\n"); print(s.makefile().readline(),end="")'
def send(req, path=endpoint, keep=False):
 s=socket.socket(socket.AF_UNIX);s.connect(path);s.sendall(json.dumps(req).encode()+b'\n');r=json.loads(s.makefile().readline())
 if keep:return r,s
 s.close();return r
holder=None
print(json.dumps({'ready':os.getpid()}),flush=True)
for line in sys.stdin:
 c=json.loads(line)
 if c.get('exit'): break
 if c.get('hold'):
  r,holder=send({'tool':'panel_set','source':c.get('source','flow'),'document':{'lines':['holder'],'attention':'wait'}},keep=True)
 elif c.get('publish'):
  req={'tool':'panel_publish','document':{'lines':['hook']}}
  if c.get('target'):
   r=send(req,c['target'])
  elif c.get('mode')=='nested':
   p=subprocess.run([sys.executable,'-c',helper,endpoint,json.dumps(req)],capture_output=True,text=True);r=json.loads(p.stdout)
  elif c.get('mode')=='helper':
   p=subprocess.run(['/usr/bin/python3','-c',helper,endpoint,json.dumps(req)],capture_output=True,text=True);r=json.loads(p.stdout)
  else:r=send(req)
 else:r={'ok':True}
 print(json.dumps({'reply':r}),flush=True)
if holder: holder.close()
"#;

struct ClientFixture {
    pty: Pty,
    output: Receiver<Vec<u8>>,
    buffered: Vec<u8>,
    directory: PanelDir,
    executable: PathBuf,
    authority: Arc<Authority>,
    state: super::super::PanelState,
    inbox: Receiver<Delivery>,
    channel: Channel,
}

impl ClientFixture {
    fn new() -> Self {
        let directory = PanelDir::create().unwrap();
        let executable = directory.path().join("fixture-claude");
        std::fs::copy("/usr/bin/python3", &executable).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let endpoint = directory.path().join(panel_contract::CONTROL_SOCKET);
        let (pty, output) = Pty::spawn(
            executable.to_str().unwrap(),
            &["-c", CLIENT, endpoint.to_str().unwrap()],
            Size::default(),
        )
        .unwrap();
        let authority = Authority::new(pty.foreground_scope().unwrap());
        // The fixture executable is the only recognized client in these isolated process trees.
        let mut authority = authority;
        Arc::get_mut(&mut authority).unwrap().programs =
            OnceLock::from(crate::client_attestation::Programs::supplied(vec![(
                Client::Claude,
                executable.clone(),
            )]));
        let state = super::super::PanelState::default();
        let (sender, inbox) = sync_channel(8);
        let service = service(
            &state,
            Arc::clone(&authority),
            sender.clone(),
            Arc::new(|| {}),
        );
        let channel = Channel::open_with_authorization_services(
            directory.path(),
            directory.id(),
            sender,
            None,
            None,
            Some(service),
            Arc::new(|| {}),
        )
        .unwrap();
        let mut fixture = Self {
            pty,
            output,
            buffered: Vec::new(),
            directory,
            executable,
            authority,
            state,
            inbox,
            channel,
        };
        assert!(fixture.read()["ready"].is_u64());
        fixture
    }

    fn command(&mut self, value: Value) -> Value {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.pty.write(&bytes).unwrap();
        self.read()
    }

    fn read(&mut self) -> Value {
        read(&self.output, &mut self.buffered)
    }

    fn hold(&mut self, source: &str) -> Connection {
        self.hold_as(source, true)
    }

    fn hold_as(&mut self, source: &str, attest: bool) -> Connection {
        self.pty
            .write(format!("{{\"hold\":true,\"source\":{}}}\n", json!(source)).as_bytes())
            .unwrap();
        let delivery = self
            .inbox
            .recv_timeout(Duration::from_secs(3))
            .expect("holder publication reaches the panel inbox");
        let connection = delivery.connection;
        let What::PublishAttested { peer, panel, .. } = delivery.what else {
            panic!("panel_set carries its captured peer witness")
        };
        self.state
            .deliver(connection, source, panel, SystemTime::now())
            .unwrap();
        if attest {
            self.authority.capture(&self.state, connection, &peer);
        }
        delivery.answer.send(Answer::Taken).unwrap();
        assert_eq!(self.read()["reply"]["ok"], true);
        connection
    }

    fn publish(&mut self, mode: &str) -> Value {
        self.command(json!({"publish":true,"mode":mode}))["reply"].clone()
    }

    fn publish_to(&mut self, endpoint: &std::path::Path) -> Value {
        self.command(json!({"publish":true,"target":endpoint}))["reply"].clone()
    }

    fn overlay(&self) -> Delivery {
        self.inbox
            .recv_timeout(Duration::from_secs(1))
            .expect("accepted hook enters the bounded UI inbox")
    }
}

impl Drop for ClientFixture {
    fn drop(&mut self) {
        let _ = self.pty.write(b"{\"exit\":true}\n");
        let _ = &self.channel;
        let _ = std::fs::remove_file(&self.executable);
    }
}

fn read(output: &Receiver<Vec<u8>>, buffered: &mut Vec<u8>) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(end) = buffered.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = buffered.drain(..=end).collect();
            if let Ok(value) = serde_json::from_slice(&line) {
                return value;
            }
        } else {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "fixture client response timed out");
            buffered.extend(output.recv_timeout(remaining).expect("fixture output"));
        }
    }
}

#[test]
fn same_client_helper_is_admitted_without_waiting_for_window_apply() {
    let mut fixture = ClientFixture::new();
    fixture.hold("flow");
    let started = Instant::now();
    assert_eq!(fixture.publish("helper")["ok"], true);
    assert!(started.elapsed() < Duration::from_secs(1));
    let overlay = fixture.overlay();
    assert!(matches!(overlay.what, What::Overlay { holder, .. } if holder == overlay.connection));
    assert!(fixture.inbox.try_recv().is_err());
}

#[test]
fn nested_client_is_rejected_but_generic_descendant_hook_is_allowed() {
    let mut fixture = ClientFixture::new();
    fixture.hold("flow");
    let nested = fixture.publish("nested");
    assert_eq!(nested["ok"], false);
    assert!(
        nested["message"]
            .as_str()
            .unwrap()
            .contains("does not descend")
    );
    assert!(fixture.inbox.try_recv().is_err());
}

#[test]
fn unheld_and_script_held_refusals_keep_their_contract() {
    let mut unheld = ClientFixture::new();
    let reply = unheld.publish("");
    assert_eq!(reply["code"], "unheld");
    assert_eq!(reply["ok"], false);

    let mut script = ClientFixture::new();
    script.hold_as("shell script", false);
    let reply = script.publish("");
    assert_eq!(reply["ok"], false);
    assert!(
        reply["message"]
            .as_str()
            .unwrap()
            .contains("not an attested agent client")
    );
}

#[test]
fn admitted_proof_survives_helper_exit_but_not_hold_replacement_or_root_death() {
    let mut fixture = ClientFixture::new();
    let holder = fixture.hold("flow");
    assert_eq!(fixture.publish("helper")["ok"], true);
    let delivery = fixture.overlay();
    let What::Overlay {
        authority,
        incarnation,
        ..
    } = delivery.what
    else {
        panic!("accepted publication is an overlay")
    };
    assert!(
        authority.current(),
        "the publishing helper may exit after admission"
    );
    fixture.state.overlay.replace(holder, "flow");
    assert!(!fixture.state.accepts_overlay(holder, incarnation));
}

#[test]
fn foreign_client_root_and_closed_holder_are_refused() {
    let mut holder = ClientFixture::new();
    holder.hold("flow");
    let mut foreign = ClientFixture::new();
    let endpoint = holder.directory.path().join(panel_contract::CONTROL_SOCKET);
    let reply = foreign.publish_to(&endpoint);
    assert_eq!(reply["ok"], false);
    assert!(
        reply["message"]
            .as_str()
            .unwrap()
            .contains("does not descend")
    );
    assert!(holder.inbox.try_recv().is_err());

    assert_eq!(holder.publish("helper")["ok"], true);
    let delivery = holder.overlay();
    let What::Overlay { authority, .. } = delivery.what else {
        panic!("overlay")
    };
    holder.pty.write(b"{\"exit\":true}\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while holder.pty.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !authority.current(),
        "a queued overlay loses authority when its client exits"
    );
}

#[test]
fn full_admission_queue_refuses_without_changing_the_document() {
    let mut fixture = ClientFixture::new();
    fixture.hold("flow");
    let before = fixture.state.document().unwrap();
    for _ in 0..crate::control::MAX_PENDING {
        assert_eq!(fixture.publish("")["ok"], true);
    }
    let reply = fixture.publish("");
    assert_eq!(reply["code"], "queue_full");
    assert_eq!(reply["message"], "this panel's publication queue is full");
    assert_eq!(fixture.state.document().unwrap(), before);
}
