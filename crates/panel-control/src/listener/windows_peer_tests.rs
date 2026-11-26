#![cfg(test)]

use super::*;
use panel_contract::PanelDir;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Instant;

struct Pane {
    dir: PanelDir,
    channel: Channel,
    inbox: Receiver<Delivery>,
    _window: crate::windows_services::tests::TestWindow,
}

fn pane(inspector: PublicationInspector) -> Pane {
    let dir = PanelDir::create().unwrap();
    let (sender, inbox) = sync_channel(8);
    let window = crate::windows_services::tests::test_message_window();
    let (services, _, _) = crate::windows_services::WindowsServices::new(window.hwnd()).unwrap();
    let hooks = Arc::new(TestHooks {
        inspector: Some(inspector),
        ..Default::default()
    });
    let channel =
        Channel::open_windows_with_test_hooks(dir.path(), dir.id(), sender, services, hooks)
            .unwrap();
    Pane {
        dir,
        channel,
        inbox,
        _window: window,
    }
}

fn verified(pane: &Pane) -> Stream {
    let stream = endpoint::connect_verified_authenticated(
        pane.dir.path(),
        pane.dir.id(),
        std::process::id(),
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    stream
        .set_recv_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
}

fn reply(stream: &mut Stream) -> serde_json::Value {
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        assert!(line.len() <= crate::wire::MAX_REQUEST);
    }
    serde_json::from_slice(&line).unwrap()
}

#[test]
fn diagnostic_peer_is_the_connected_process() {
    let (observed, peers) = sync_channel(1);
    let mut pane = pane(Arc::new(move |peer| {
        observed.send(peer.clone()).unwrap();
        PublicationOrigin::Inside
    }));
    let _client = verified(&pane);
    let peer = peers.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(peer.pid(), std::process::id());
    assert!(peer.is_alive());
    pane.channel.shutdown();
}

#[test]
fn inspector_runs_once_and_its_observation_reaches_each_delivery() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let mut pane = pane(Arc::new(move |peer| {
        assert_eq!(peer.pid(), std::process::id());
        assert!(peer.is_alive());
        seen.fetch_add(1, Ordering::Relaxed);
        PublicationOrigin::Outside
    }));
    let mut client = verified(&pane);
    for _ in 0..2 {
        client
            .write_all(
                b"{\"tool\":\"panel_set\",\"source\":\"test\",\"document\":{\"lines\":[]}}\n",
            )
            .unwrap();
        let delivery = pane.inbox.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(delivery.origin, PublicationOrigin::Outside);
        delivery.answer.send(Answer::Taken).unwrap();
        assert_eq!(reply(&mut client)["ok"], true);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    pane.channel.shutdown();
}

#[test]
fn raw_windows_publications_remain_unknown_without_a_witness() {
    let mut pane = pane(Arc::new(|_| {
        panic!("raw producer reached verified inspector")
    }));
    let mut client =
        endpoint::connect_authenticated(pane.dir.path(), pane.dir.id(), std::process::id())
            .unwrap();
    client
        .set_recv_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client
        .write_all(b"{\"tool\":\"panel_set\",\"source\":\"test\",\"document\":{\"lines\":[]}}\n")
        .unwrap();
    let delivery = pane.inbox.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(delivery.origin, PublicationOrigin::Unknown);
    assert!(
        matches!(&delivery.what, What::PublishWindows { witness: None, .. }),
        "expected an unverified Windows publication, got {:?}",
        delivery.what
    );
    delivery.answer.send(Answer::Taken).unwrap();
    assert_eq!(reply(&mut client)["ok"], true);
    pane.channel.shutdown();
}
