//! Diagnostic publication attribution bound to one pane's revocable process scope.

use std::sync::Arc;

use panel_control::{DiagnosticPeer, PublicationInspector, PublicationOrigin};
use term_core::{Ancestry, ForegroundScope};

/// Build the per-listener observation callback. The captured scope is the pane's original PTY root;
/// its native lifetime check makes every observation unknown once that PTY has been revoked.
pub(crate) fn inspector(scope: ForegroundScope) -> PublicationInspector {
    Arc::new(move |peer: &DiagnosticPeer| {
        if !peer.is_alive() || !scope.is_open() {
            return PublicationOrigin::Unknown;
        }
        let result = match scope.classify_ancestry(peer.pid()) {
            Ancestry::Inside => PublicationOrigin::Inside,
            Ancestry::Outside => PublicationOrigin::Outside,
            Ancestry::Unknown => PublicationOrigin::Unknown,
        };
        if peer.is_alive() && scope.is_open() {
            result
        } else {
            PublicationOrigin::Unknown
        }
    })
}

pub(crate) fn unavailable() -> PublicationInspector {
    Arc::new(|_: &DiagnosticPeer| PublicationOrigin::Unknown)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;
    use std::thread;
    use std::time::Duration;

    use panel_contract::PanelDir;
    use panel_control::{Channel, Producer};
    use term_core::{Pty, Size};

    fn next_publication(
        deliveries: &std::sync::mpsc::Receiver<panel_control::Delivery>,
    ) -> panel_control::Delivery {
        loop {
            let delivery = deliveries
                .recv_timeout(Duration::from_secs(5))
                .expect("publication delivery");
            if matches!(
                delivery.what,
                panel_control::What::Publish { .. } | panel_control::What::PublishAttested { .. }
            ) {
                return delivery;
            }
            let _ = delivery.answer.send(panel_control::Answer::Taken);
        }
    }

    #[test]
    fn publish_from_pane_child() {
        if std::env::var_os("IVY_PROVENANCE_CHILD").is_none() {
            return;
        }
        let mut producer = Producer::of_this_pane().expect("inherited pane identity");
        producer
            .publish("pane child", r#"{"lines":["inside"]}"#)
            .expect("publish from pane child");
    }

    #[test]
    fn listener_observes_outside_peer_and_unknown_after_pty_revocation() {
        let dir = PanelDir::create().expect("panel directory");
        let (sender, deliveries) = sync_channel(4);
        let (id_name, id_value) = dir.env()[0];
        let (dir_name, dir_value) = dir.env()[1];
        let env = [
            (id_name, Some(id_value)),
            (dir_name, Some(dir_value)),
            ("IVY_PROVENANCE_CHILD", Some(std::ffi::OsStr::new("1"))),
        ];
        let (mut pty, _output) =
            Pty::spawn_with_env("/bin/sh", &[], Size::default(), &env).expect("pane root");
        let inspector = inspector(pty.foreground_scope().expect("diagnostic scope"));
        let channel = Channel::open_with_publication_inspector(
            dir.path(),
            dir.id(),
            sender,
            None,
            None,
            None,
            Some(inspector),
            Arc::new(|| {}),
        )
        .expect("listener");

        let outside_dir = dir.path().to_owned();
        let outside_id = dir.id().to_owned();
        let outside = thread::spawn(move || {
            Producer::of(&outside_dir, &outside_id)
                .expect("outside producer")
                .publish("outside", r#"{"lines":["outside"]}"#)
                .expect("outside publish")
        });
        let delivery = next_publication(&deliveries);
        assert_eq!(delivery.origin, PublicationOrigin::Outside);
        delivery
            .answer
            .send(panel_control::Answer::Taken)
            .expect("answer outside");
        outside.join().expect("outside producer thread");

        let executable = std::env::current_exe().expect("test executable");
        let executable = executable.to_string_lossy().replace('\'', "'\\''");
        let command = format!(
            "'{}' --exact panel::provenance_route::tests::publish_from_pane_child --nocapture\n",
            executable
        );
        pty.write(command.as_bytes())
            .expect("start pane descendant");
        let delivery = next_publication(&deliveries);
        assert_eq!(delivery.origin, PublicationOrigin::Inside);
        delivery
            .answer
            .send(panel_control::Answer::Taken)
            .expect("answer descendant");

        drop(pty);
        let after_exit_dir = dir.path().to_owned();
        let after_exit_id = dir.id().to_owned();
        let after_exit = thread::spawn(move || {
            Producer::of(&after_exit_dir, &after_exit_id)
                .expect("second producer")
                .publish("after exit", r#"{"lines":["unknown"]}"#)
                .expect("publish after pane exit")
        });
        let delivery = next_publication(&deliveries);
        assert_eq!(delivery.origin, PublicationOrigin::Unknown);
        delivery
            .answer
            .send(panel_control::Answer::Taken)
            .expect("answer after exit");
        after_exit.join().expect("post-exit producer thread");
        drop(channel);
    }
}
