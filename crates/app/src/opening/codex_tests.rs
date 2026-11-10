#![cfg(test)]

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let path =
            std::env::temp_dir().join(format!("ivy-codex-pty-{}", u64::from_ne_bytes(random)));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join("bin")).unwrap();
        write_program(
            &path.join("bash"),
            "#!/bin/sh\nHOME=$(dirname \"$0\") exec /bin/bash \"$@\"\n",
        );
        std::fs::write(path.join(".bashrc"), "export PATH=\"$HOME/bin:$PATH\"\n").unwrap();
        write_program(
            &path.join("helper"),
            "#!/bin/sh\nprintf 'helper\\0' >> \"$HOME/$PANEL_ID.record\"\nexec \"$@\"\n",
        );
        write_program(
            &path.join("bin/codex"),
            "#!/bin/sh\nprintf '%s\\0' \"$PANEL_ID\" \"$PANEL_DIR\" \"$@\" >> \"$HOME/$PANEL_ID.record\"\nexit 37\n",
        );
        Self(path)
    }

    fn open(&self, name: usize) -> Pane {
        let shell = self.0.join("bash");
        let prepared = shell_integration::Prepared::prepare(
            shell.to_str().unwrap(),
            Some(&self.0.join("helper")),
        )
        .unwrap()
        .unwrap();
        launch(
            &egui::Context::default(),
            Launch {
                program: shell.to_str().unwrap(),
                args: &[],
                directory: Some(&self.0),
                path: None,
            },
            Some(PanelDir::create().unwrap()),
            name,
            control::ReadActivity::default(),
            false,
            Some(prepared),
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn write_program(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn launched_shells_keep_their_own_pane_identity_and_survive_repeated_codex_runs() {
    let fixture = Fixture::new();
    let mut panes = [fixture.open(0), fixture.open(1)];
    assert_ne!(
        panes[0].panel.as_ref().unwrap().id(),
        panes[1].panel.as_ref().unwrap().id()
    );
    for pane in &mut panes {
        pane.pty
            .write(b"codex 'one argument' ''; printf 'status:%s\\0' \"$?\" >> \"$HOME/$PANEL_ID.record\"; codex second; printf 'alive\\0' >> \"$HOME/$PANEL_ID.record\"\n")
            .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    for pane in &panes {
        let panel = pane.panel.as_ref().unwrap();
        let file = fixture.0.join(format!("{}.record", panel.id()));
        let bytes = loop {
            let bytes = std::fs::read(&file).unwrap_or_default();
            if bytes.ends_with(b"alive\0") {
                break bytes;
            }
            assert!(
                Instant::now() < deadline,
                "shell did not complete: {bytes:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let fields: Vec<_> = bytes.split(|byte| *byte == 0).collect();
        assert_eq!(
            fields,
            vec![
                &b"helper"[..],
                panel.id().as_bytes(),
                panel.path().as_os_str().as_encoded_bytes(),
                &b"one argument"[..],
                &b""[..],
                &b"status:37"[..],
                &b"helper"[..],
                panel.id().as_bytes(),
                panel.path().as_os_str().as_encoded_bytes(),
                &b"second"[..],
                &b"alive"[..],
                &b""[..],
            ]
        );
    }
}
