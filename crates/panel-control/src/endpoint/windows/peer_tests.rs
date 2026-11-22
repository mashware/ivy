#![cfg(test)]

use super::*;
use crate::{DiagnosticPeer, session::Peer};
use panel_contract::PanelDir;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::sync_channel;
use std::thread;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn child_connection() -> (Stream, ChildGuard, StopSignal) {
    let dir = PanelDir::create().unwrap();
    let listener = bind(dir.path(), dir.id()).unwrap();
    let (stop, acceptor) = acceptor(listener).unwrap();
    let (accepted, streams) = sync_channel(1);
    let server = thread::spawn(move || {
        let result = match acceptor.wait() {
            Ok(super::super::AcceptorReady::Connection) => {
                acceptor.accept().and_then(Stream::negotiate_peer)
            }
            Ok(super::super::AcceptorReady::Stop) => Err(stopped()),
            Err(problem) => Err(problem),
        };
        let _ = accepted.send(result);
    });
    // A real child speaks the public upgrade, then stays alive until its parent closes stdin.
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
function Connect-Pipe([string]$name) {{
    $s = [System.IO.Pipes.NamedPipeClientStream]::new('.', $name, [System.IO.Pipes.PipeDirection]::InOut, [System.IO.Pipes.PipeOptions]::Asynchronous)
    $s.Connect(2000)
    return $s
}}
function Read-Line([System.IO.Stream]$stream) {{
    $r = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8, $false, 256, $true)
    return $r.ReadLine()
}}
$bootstrap = Connect-Pipe 'ivy-{}'
$hello = [System.Text.Encoding]::UTF8.GetBytes('{{"tool":"peer_hello","v":1}}' + [char]10)
$bootstrap.Write($hello, 0, $hello.Length)
$redirect = (Read-Line $bootstrap) | ConvertFrom-Json
$fresh = Connect-Pipe $redirect.peer.name
$ready = (Read-Line $fresh) | ConvertFrom-Json
if ($ready.peer.mode -ne 'verified') {{ throw 'peer was not verified' }}
[Console]::In.ReadLine() | Out-Null
$fresh.Dispose()
$bootstrap.Dispose()
"#,
        dir.id()
    );
    let child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let child = ChildGuard(child);
    let received = streams.recv_timeout(Duration::from_secs(10));
    if received.is_err() {
        drop(stop);
        server.join().unwrap();
        panic!(
            "child did not complete its peer upgrade: {}",
            received.err().unwrap()
        );
    }
    let stream = received.unwrap().unwrap();
    server.join().unwrap();
    (stream, child, stop)
}

#[test]
fn exited_windows_peer_and_its_clones_never_become_alive_again() {
    let (stream, mut child, _stop) = child_connection();
    let peer = Peer::capture(&stream).unwrap();
    let cloned_peer = peer.clone();
    let diagnostic = DiagnosticPeer::capture(&stream).unwrap();
    let cloned_diagnostic = diagnostic.clone();
    assert_eq!(peer.pid(), child.0.id());
    assert_eq!(diagnostic.pid(), child.0.id());
    assert!(peer.birth() > 0 && peer.is_alive() && diagnostic.is_alive());
    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = creation;
    let mut kernel = creation;
    let mut user = creation;
    assert_ne!(
        unsafe {
            GetProcessTimes(
                child.0.as_raw_handle(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        },
        0
    );
    assert_eq!(
        peer.birth(),
        ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64
    );
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    for witness in [&peer, &cloned_peer] {
        assert!(!witness.is_alive());
    }
    for witness in [&diagnostic, &cloned_diagnostic] {
        assert!(!witness.is_alive());
    }
    assert!(Peer::capture(&stream).is_err());
    assert!(DiagnosticPeer::capture(&stream).is_err());
}

#[test]
fn a_substituted_process_cannot_authorize_the_fresh_connector() {
    let (candidate_stream, _child, _stop) = child_connection();
    let candidate = candidate_stream.verified_peer().unwrap();
    // The endpoint must not predate the candidate: this is the ordering the binding proof needs.
    let dir = PanelDir::create().unwrap();
    let listener = bind(dir.path(), dir.id()).unwrap();
    let _client = connect(dir.path(), dir.id()).unwrap();
    let fresh = listener.accept().unwrap();
    assert!(candidate.is_alive());
    assert_ne!(candidate.pid(), std::process::id());
    assert_eq!(
        candidate.verify_fresh_connector(&fresh).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn raw_windows_stream_cannot_manufacture_a_verified_peer() {
    let dir = PanelDir::create().unwrap();
    let listener = bind(dir.path(), dir.id()).unwrap();
    let _client = connect(dir.path(), dir.id()).unwrap();
    let raw = listener.accept().unwrap();
    assert!(Peer::capture(&raw).is_err());
    assert!(DiagnosticPeer::capture(&raw).is_err());
}
