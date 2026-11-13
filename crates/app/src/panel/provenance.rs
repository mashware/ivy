//! Application-owned provenance is adjacent to the document, never part of producer JSON.
use std::time::SystemTime;

use panel_control::{Connection, PublicationOrigin};

use super::{Panel, PanelState, Row, Style, Theme};

const OUTSIDE: &str = "published from a process outside this console";

/// A failed publication cannot replace the current document's observation.
pub(crate) fn deliver_observed(
    state: &mut PanelState,
    connection: Connection,
    source: &str,
    panel: Panel,
    now: SystemTime,
    origin: PublicationOrigin,
) -> Result<(), String> {
    state.deliver(connection, source, panel, now)?;
    state.published_by = origin;
    Ok(())
}

pub(super) fn append_row(rows: &mut Vec<Row>, state: &PanelState, width: usize, theme: &Theme) {
    if state.published_by == PublicationOrigin::Outside {
        super::wrap(
            rows,
            0,
            OUTSIDE,
            theme.panel_color(None, Some(Style::Warn)),
            width,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::{PANEL_FILE, PanelDir, parse_document, parse_ops};
    use std::time::Duration;

    fn panel(done: bool) -> Panel {
        parse_document(if done {
            r#"{"header":false,"attention":"done","lines":[{"text":"Body","style":"section","collapsed":true},"content"]}"#
        } else {
            r#"{"header":false,"lines":["content"]}"#
        }).unwrap()
    }

    fn write(dir: &PanelDir, text: &str, at: SystemTime) {
        let path = dir.path().join(PANEL_FILE);
        std::fs::write(&path, text).unwrap();
        std::fs::File::open(path).unwrap().set_modified(at).unwrap();
    }

    #[test]
    fn observation_survives_disconnect_and_only_a_valid_newer_file_replaces_it() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let dir = PanelDir::create().unwrap();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        assert!(!state.document().unwrap().contains(OUTSIDE));
        state.release(1);
        for (text, stamp) in [
            ("{", at + Duration::from_secs(1)),
            (r#"{"lines":["old"]}"#, at),
            (r#"{"lines":["older"]}"#, at - Duration::from_secs(1)),
        ] {
            write(&dir, text, stamp);
            state.refresh(dir.path());
            assert_eq!(state.published_by, PublicationOrigin::Outside);
        }
        write(&dir, r#"{"lines":["new"]}"#, at + Duration::from_secs(2));
        state.refresh(dir.path());
        assert_eq!(state.published_by, PublicationOrigin::Unknown);
    }

    #[test]
    fn patches_keep_observation_refusals_do_not_change_it_and_full_publications_replace_it() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(false),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        let ops = parse_ops(&serde_json::json!([{"op":"set","field":"attention","value":"done"}]))
            .unwrap();
        state.patch(1, &ops, at).unwrap();
        assert_eq!(state.published_by, PublicationOrigin::Outside);
        assert!(
            deliver_observed(
                &mut state,
                2,
                "other",
                panel(false),
                at,
                PublicationOrigin::Inside
            )
            .is_err()
        );
        assert_eq!(state.published_by, PublicationOrigin::Outside);
        for origin in [
            PublicationOrigin::Inside,
            PublicationOrigin::Unknown,
            PublicationOrigin::Outside,
        ] {
            deliver_observed(&mut state, 1, "agent", panel(false), at, origin).unwrap();
            assert_eq!(state.published_by, origin);
        }
    }

    #[test]
    fn app_row_survives_hidden_header_and_block_folds_and_invalidates_cached_layout() {
        let at = SystemTime::now();
        let theme = Theme::default();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        let (rows, cached) = state
            .lay_out_if_needed("pane", 100, &theme, None, None)
            .unwrap();
        assert!(!cached);
        assert!(rows.iter().any(|row| row.text == OUTSIDE));
        assert!(!rows.iter().any(|row| row.text == "content"));
        assert!(
            state
                .lay_out_if_needed("pane", 100, &theme, None, None)
                .unwrap()
                .1
        );
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Unknown,
        )
        .unwrap();
        let (rows, cached) = state
            .lay_out_if_needed("pane", 100, &theme, None, None)
            .unwrap();
        assert!(!cached);
        assert!(!rows.iter().any(|row| row.text == OUTSIDE));
    }

    #[test]
    fn retirement_clears_observation() {
        let dir = PanelDir::create().unwrap();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            SystemTime::now(),
            PublicationOrigin::Outside,
        )
        .unwrap();
        state
            .retire(
                1,
                PanelState::retirement_baseline(dir.path()).unwrap(),
                super::super::Retirement::default(),
            )
            .unwrap();
        assert_eq!(state.published_by, PublicationOrigin::Unknown);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_pane_labels_outside_socket_and_accepts_its_shell_descendant_without_session_capture()
    {
        use eframe::egui;
        use std::thread;

        let ctx = egui::Context::default();
        let dir = PanelDir::create().expect("panel directory");
        let endpoint = dir.path().to_owned();
        let id = dir.id().to_owned();
        let mut pane = crate::opening::launch_exact(
            &ctx,
            crate::opening::Launch {
                program: "/bin/sh",
                args: &[],
                directory: None,
                path: None,
            },
            Some(dir),
            0,
            crate::control::ReadActivity::default(),
        )
        .expect("open pane through the application path");
        crate::resume::set_capture(&mut pane, false);
        assert!(pane.restoration.sessions.is_none());

        let outside_dir = endpoint.clone();
        let outside_id = id.clone();
        let outside = thread::spawn(move || {
            panel_control::Producer::of(&outside_dir, &outside_id)
                .expect("outside producer")
                .publish("outside", r#"{"header":false,"lines":["outside body"]}"#)
        });
        drive_pane_until(&mut pane, &ctx, |_| outside.is_finished());
        outside
            .join()
            .expect("outside worker")
            .expect("outside publish accepted");
        let (rows, _) = pane
            .panel_state
            .lay_out_if_needed("pane", 100, &Theme::default(), None, None)
            .expect("outside document layout");
        assert!(rows.iter().any(|row| row.text == OUTSIDE));
        assert!(
            pane.panel_state
                .document()
                .unwrap()
                .contains("outside body")
        );
        drive_pane_until(&mut pane, &ctx, |pane| pane.panel_state.held_by().is_none());

        let executable = std::env::current_exe().expect("test executable");
        let executable = executable.to_string_lossy().replace('\'', "'\\''");
        let command = format!(
            "IVY_PROVENANCE_CHILD=1 '{}' --exact panel::provenance_route::tests::publish_from_pane_child --nocapture\n",
            executable
        );
        pane.pty
            .write(command.as_bytes())
            .expect("launch pane descendant");
        drive_pane_until(&mut pane, &ctx, |pane| {
            pane.panel_state
                .last_good
                .as_ref()
                .is_some_and(|panel| panel.lines.iter().any(|line| line.text == "inside"))
        });
        let (rows, _) = pane
            .panel_state
            .lay_out_if_needed("pane", 100, &Theme::default(), None, None)
            .expect("inside document layout");
        assert!(!rows.iter().any(|row| row.text == OUTSIDE));
    }

    #[cfg(target_os = "linux")]
    fn drive_pane_until(
        pane: &mut crate::Pane,
        ctx: &eframe::egui::Context,
        mut ready: impl FnMut(&crate::Pane) -> bool,
    ) {
        use std::{
            thread,
            time::{Duration, Instant},
        };

        let deadline = Instant::now() + Duration::from_secs(8);
        while !ready(pane) {
            pane.refresh_its_panel(ctx);
            assert!(
                Instant::now() < deadline,
                "pane did not reach expected state"
            );
            thread::yield_now();
        }
        pane.refresh_its_panel(ctx);
    }
}
