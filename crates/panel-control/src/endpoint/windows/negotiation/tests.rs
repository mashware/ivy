#![cfg(test)]

use super::*;
use crate::endpoint::windows;
use std::sync::Arc;
use std::thread;

fn listener() -> (panel_contract::PanelDir, windows::Listener) {
    let dir = panel_contract::PanelDir::create().unwrap();
    let listener = windows::bind(dir.path(), dir.id()).unwrap();
    (dir, listener)
}

#[test]
fn upgraded_connection_retains_actual_connector_and_clones() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap().negotiate_peer().unwrap();
        let witness = stream
            .verified_peer()
            .expect("verified accepted server stream");
        assert_eq!(witness.pid(), std::process::id());
        assert!(witness.birth() > 0 && witness.is_alive());
        let clone = stream.try_clone().unwrap();
        assert!(Arc::ptr_eq(&witness, &clone.verified_peer().unwrap()));
        let mut request = [0; 5];
        (&stream).read_exact(&mut request).unwrap();
        assert_eq!(&request, b"hello");
        (&stream).write_all(b"reply").unwrap();
    });
    let mut client = connect_verified(
        dir.path(),
        dir.id(),
        std::process::id(),
        Instant::now() + DEADLINE,
    )
    .unwrap();
    client.write_all(b"hello").unwrap();
    let mut reply = [0; 5];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"reply");
    server.join().unwrap();
}

#[test]
fn raw_first_request_and_pipeline_are_replayed_without_a_witness() {
    let (dir, listener) = listener();
    let request = b"{\"tool\":\"panel_get\"}\nsecond\n";
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap().negotiate_peer().unwrap();
        assert!(stream.verified_peer().is_none());
        let mut bytes = vec![0; request.len()];
        (&stream).read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, request);
    });
    windows::connect(dir.path(), dir.id())
        .unwrap()
        .write_all(request)
        .unwrap();
    server.join().unwrap();
}

#[test]
fn bootstrap_pipeline_is_not_promoted_to_the_verified_stream() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let mut stream = listener.accept().unwrap().negotiate_peer().unwrap();
        let mut request = [0; 5];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"fresh");
    });
    let mut bootstrap = windows::connect(dir.path(), dir.id()).unwrap();
    bootstrap.write_all(&[HELLO, b"forged\n"].concat()).unwrap();
    let redirect = read_frame(&bootstrap, Instant::now() + DEADLINE).unwrap();
    let fresh = connect_fresh(&redirect.name.unwrap()).unwrap();
    assert_eq!(
        read_frame(&fresh, Instant::now() + DEADLINE).unwrap().mode,
        "verified"
    );
    (&fresh).write_all(b"fresh").unwrap();
    server.join().unwrap();
}

#[test]
fn invalid_upgrade_version_is_refused_without_downgrade() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        assert!(listener.accept().unwrap().negotiate_peer().is_err());
    });
    let mut client = windows::connect(dir.path(), dir.id()).unwrap();
    client
        .write_all(b"{\"tool\":\"peer_hello\",\"v\":2}\n")
        .unwrap();
    assert!(read_frame(&client, Instant::now() + DEADLINE).is_err());
    server.join().unwrap();
}

#[test]
fn redirected_names_are_components_and_never_paths() {
    assert!(valid_name("ivy-peer-0123456789abcdef0123456789abcdef"));
    for invalid in [
        "\\\\server\\pipe\\anything",
        "ivy-peer-../path",
        "ivy-peer-0123456789ABCDEF0123456789ABCDEF",
        "ivy-peer-0000",
    ] {
        assert!(!valid_name(invalid));
    }
}

#[test]
fn fresh_instance_cannot_be_created_twice_or_accept_after_stop() {
    let stop = Arc::new(event().unwrap());
    let name = wide(std::ffi::OsStr::new(&format!(
        "{PIPE_PATH_PREFIX}{}",
        fresh_name().unwrap()
    )));
    let rendezvous = Rendezvous::create(&name, Arc::clone(&stop)).unwrap();
    assert!(Rendezvous::create(&name, Arc::clone(&stop)).is_err());
    unsafe {
        windows_sys::Win32::System::Threading::SetEvent(stop.0);
    }
    assert!(rendezvous.accept(Instant::now() + DEADLINE).is_err());
}

#[test]
fn pending_rendezvous_timeout_drains_and_releases_its_name() {
    let stop = Arc::new(event().unwrap());
    let name = wide(std::ffi::OsStr::new(&format!(
        "{PIPE_PATH_PREFIX}{}",
        fresh_name().unwrap()
    )));
    let rendezvous = Rendezvous::create(&name, Arc::clone(&stop)).unwrap();
    let failure = rendezvous
        .accept(Instant::now() + Duration::from_millis(10))
        .err()
        .unwrap();
    assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
    assert!(Rendezvous::create(&name, stop).is_ok());
}

#[test]
fn a_large_raw_request_is_preserved_and_clones_share_replay_bytes() {
    let (dir, listener) = listener();
    let mut request = br#"{"tool":"panel_set","document":{"summary":""#.to_vec();
    request.extend(std::iter::repeat_n(b'x', 32 * 1024));
    request.extend_from_slice(b"\"}}\nnext\n");
    let expected = request.clone();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap().negotiate_peer().unwrap();
        assert!(stream.verified_peer().is_none());
        let mut clone = stream.try_clone().unwrap();
        let mut first = [0; 20];
        (&stream).read_exact(&mut first).unwrap();
        let mut rest = vec![0; expected.len() - first.len()];
        clone.read_exact(&mut rest).unwrap();
        assert_eq!([first.as_slice(), rest.as_slice()].concat(), expected);
    });
    windows::connect(dir.path(), dir.id())
        .unwrap()
        .write_all(&request)
        .unwrap();
    server.join().unwrap();
}

#[test]
fn unavailable_candidate_explicitly_negotiates_an_unverified_stream() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let stream = accept_with_capture(listener.accept().unwrap(), |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "test access refusal",
            ))
        })
        .unwrap();
        assert!(stream.verified_peer().is_none());
        let mut request = [0; 5];
        (&stream).read_exact(&mut request).unwrap();
        assert_eq!(&request, b"hello");
    });
    let mut client = connect_verified(
        dir.path(),
        dir.id(),
        std::process::id(),
        Instant::now() + DEADLINE,
    )
    .unwrap();
    client.write_all(b"hello").unwrap();
    server.join().unwrap();
}

#[test]
fn client_upgrade_uses_its_supplied_total_deadline() {
    let (dir, listener) = listener();
    let (stop, acceptor) = windows::acceptor(listener).unwrap();
    let (release, held) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        if matches!(
            acceptor.wait().unwrap(),
            crate::endpoint::AcceptorReady::Connection
        ) {
            let stream = acceptor.accept().unwrap();
            // Keep the peer open until the client returns. The outer watchdog only releases a
            // broken implementation; EOF must never count as evidence that its deadline worked.
            let _ = held.recv_timeout(DEADLINE * 2);
            drop(stream);
        }
    });
    let started = Instant::now();
    let result = connect_verified(
        dir.path(),
        dir.id(),
        std::process::id(),
        started + Duration::from_millis(20),
    );
    let elapsed = started.elapsed();
    let _ = release.send(());
    drop(stop); // Also releases accept if the deadline expired before the client connected.
    server.join().unwrap();
    let failure = result
        .err()
        .expect("a silent peer must exhaust the supplied deadline");
    assert!(matches!(
        failure.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ));
    assert!(
        elapsed < DEADLINE / 2,
        "client reset its caller's total deadline"
    );
}

#[test]
fn unverified_upgrade_preserves_pipelined_bytes_without_authority() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let stream = accept_with_capture(listener.accept().unwrap(), |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "test refusal",
            ))
        })
        .unwrap();
        assert!(stream.verified_peer().is_none());
        assert!(crate::session::Peer::capture(&stream).is_err());
        let mut data = [0; 5];
        (&stream).read_exact(&mut data).unwrap();
        assert_eq!(&data, b"hello");
    });
    let mut client = windows::connect(dir.path(), dir.id()).unwrap();
    client.write_all(&[HELLO, b"hello"].concat()).unwrap();
    assert_eq!(
        read_frame(&client, Instant::now() + DEADLINE).unwrap().mode,
        "unverified"
    );
    server.join().unwrap();
}

#[test]
fn a_different_process_on_the_second_pipe_is_refused_and_releases_the_name() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let failure = listener.accept().unwrap().negotiate_peer().err().unwrap();
        assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
    });
    // Start the foreign process before the handshake clock, then give it the fresh name.
    // A loaded runner's PowerShell startup time is not part of the behavior under test.
    let script = r#"
$ErrorActionPreference = 'Stop'
[Console]::Out.WriteLine('READY')
$name = [Console]::In.ReadLine()
$s = [System.IO.Pipes.NamedPipeClientStream]::new('.', $name, [System.IO.Pipes.PipeDirection]::InOut, [System.IO.Pipes.PipeOptions]::Asynchronous)
$s.Connect(2000)
$r = [System.IO.StreamReader]::new($s)
$reply = $r.ReadLineAsync()
if (-not $reply.Wait(3000)) { throw 'fresh refusal timed out' }
[Console]::Out.WriteLine($reply.Result)
$s.Dispose()
"#;
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let (started, readiness) = std::sync::mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut ready = String::new();
        let result = stdout.read_line(&mut ready);
        let _ = started.send((stdout, ready, result));
    });
    let (mut stdout, ready, result) = readiness.recv_timeout(Duration::from_secs(10)).unwrap();
    result.unwrap();
    assert_eq!(ready.trim(), "READY");
    reader.join().unwrap();
    let mut bootstrap = windows::connect(dir.path(), dir.id()).unwrap();
    bootstrap.write_all(HELLO).unwrap();
    let redirect = read_frame(&bootstrap, Instant::now() + DEADLINE).unwrap();
    let name = redirect.name.unwrap();
    assert!(valid_name(&name));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{name}\n").as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    let mut output = String::new();
    stdout.read_to_string(&mut output).unwrap();
    let reply: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(reply["ok"], false);
    server.join().unwrap();
    assert!(connect_fresh(&name).is_err());
    let full_name = wide(std::ffi::OsStr::new(&format!("{PIPE_PATH_PREFIX}{name}")));
    assert!(Rendezvous::create(&full_name, Arc::new(event().unwrap())).is_ok());
}

#[test]
fn a_verified_stream_clone_keeps_identity_and_io_after_the_original_is_dropped() {
    let (dir, listener) = listener();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap().negotiate_peer().unwrap();
        let birth = stream.verified_peer().unwrap().birth();
        let mut clone = stream.try_clone().unwrap();
        drop(stream);
        let peer = crate::session::Peer::capture(&clone).unwrap();
        assert_eq!(peer.pid(), std::process::id());
        assert_eq!(peer.birth(), birth);
        assert!(peer.is_alive());
        clone.set_recv_timeout(Some(DEADLINE)).unwrap();
        let mut request = [0; 5];
        clone.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"hello");
        clone.write_all(b"reply").unwrap();
    });
    let mut client = connect_verified(
        dir.path(),
        dir.id(),
        std::process::id(),
        Instant::now() + DEADLINE,
    )
    .unwrap();
    client.set_recv_timeout(Some(DEADLINE)).unwrap();
    client.write_all(b"hello").unwrap();
    let mut reply = [0; 5];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"reply");
    server.join().unwrap();
}

#[test]
fn stopping_after_the_redirect_cancels_and_releases_the_fresh_pipe() {
    let (dir, listener) = listener();
    let (stop, acceptor) = windows::acceptor(listener).unwrap();
    let (finished, completion) = std::sync::mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        assert!(matches!(
            acceptor.wait().unwrap(),
            crate::endpoint::AcceptorReady::Connection
        ));
        let failure = acceptor.accept().unwrap().negotiate_peer().err().unwrap();
        finished.send(failure.kind()).unwrap();
    });
    let mut bootstrap = windows::connect(dir.path(), dir.id()).unwrap();
    bootstrap.write_all(HELLO).unwrap();
    let redirect = read_frame(&bootstrap, Instant::now() + DEADLINE).unwrap();
    let name = redirect.name.unwrap();
    drop(stop);
    assert_eq!(
        completion.recv_timeout(Duration::from_secs(1)).unwrap(),
        io::ErrorKind::ConnectionAborted
    );
    server.join().unwrap();
    assert!(connect_fresh(&name).is_err());
    let full_name = wide(std::ffi::OsStr::new(&format!("{PIPE_PATH_PREFIX}{name}")));
    assert!(Rendezvous::create(&full_name, Arc::new(event().unwrap())).is_ok());
}
