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
    // Keep shell/runtime module initialization outside the strict pipe-upgrade deadline:
    // the former PowerShell fixture first called ConvertFrom-Json after sending its hello.
    let fixture = format!("{}::peer_child_fixture", module_path!())
        .split_once("::")
        .unwrap()
        .1
        .to_owned();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &fixture, "--nocapture"])
        .env("IVY_PEER_TEST_ID", dir.id())
        .env("IVY_PEER_TEST_SERVER", std::process::id().to_string())
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
fn peer_child_fixture() {
    let Ok(id) = std::env::var("IVY_PEER_TEST_ID") else {
        return;
    };
    let server = std::env::var("IVY_PEER_TEST_SERVER")
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(server, std::process::id(), "fixture must be a real child");
    let stream = connect_verified(
        std::path::Path::new(""),
        &id,
        server,
        std::time::Instant::now() + negotiation::DEADLINE,
    )
    .unwrap();
    // Keep the connector and its process alive until the parent closes stdin or kills it.
    let mut release = [0];
    std::io::Read::read(&mut std::io::stdin(), &mut release).unwrap();
    drop(stream);
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
