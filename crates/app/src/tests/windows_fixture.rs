//! The small fixture vocabulary used by cross-platform console tests.

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The declared directory is a precondition of interaction tests, not an OSC7 transport witness.
pub(super) fn directory(program: &str) -> Option<&str> {
    program.strip_prefix("cd '").map(|rest| {
        rest.split_once("' && ")
            .expect("a directory fixture has a quoted path and a command")
            .0
    })
}

/// Translate only fixtures whose behavior is witnessed by the caller, never arbitrary shell code.
pub(super) fn command(program: &str) -> String {
    let mut prefix = "$ErrorActionPreference = 'Stop'; ".to_owned();
    let program = if let Some(rest) = program.strip_prefix("cd '") {
        let (folder, program) = rest
            .split_once("' && ")
            .expect("a directory fixture has a quoted path and a command");
        prefix.push_str(&format!("Set-Location -LiteralPath {}; [Environment]::CurrentDirectory = (Get-Location).ProviderPath; ", literal(folder),));
        program
    } else {
        program
    };
    let program = program.strip_prefix("exec ").unwrap_or(program);
    let body = if let Some(seconds) = program.strip_prefix("sleep ") {
        let seconds = seconds.parse::<f64>().expect("valid fixture sleep");
        assert!(seconds.is_finite() && seconds >= 0.0);
        format!("Start-Sleep -Seconds {seconds}")
    } else if program == "yes" {
        // Repeating an identical line need not change ConPTY's viewport. The counter witnesses
        // continuing child output rather than the pseudoconsole's startup sequences.
        "[long]$counter = 0; while ($true) { [Console]::WriteLine(('ivy-child' + '-ready ' + $counter)); $counter++ }".to_owned()
    } else if program == "true" {
        "exit 0".to_owned()
    } else if program == "cat" {
        "while ($null -ne ($line = [Console]::ReadLine())) { [Console]::WriteLine($line) }"
            .to_owned()
    } else if let Some(file) = program
        .strip_prefix("cat > '")
        .and_then(|s| s.strip_suffix('\''))
    {
        // ReadLine consumes the console's line ending; write UTF-8 plus LF explicitly, without
        // PowerShell's BOM or CRLF, so the file witnesses exactly the input these tests send.
        format!(
            "$file = [IO.File]::Open({}, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite); try {{ while ($null -ne ($line = [Console]::ReadLine())) {{ $bytes = [Text.Encoding]::UTF8.GetBytes($line + [char]10); $file.Write($bytes, 0, $bytes.Length); $file.Flush() }} }} finally {{ $file.Dispose() }}",
            literal(file),
        )
    } else if let Some(rest) = program.strip_prefix("read -r line; printf ") {
        let (text, seconds) = rest.split_once("; sleep ").expect("input-gated fixture");
        let seconds = seconds.parse::<u64>().expect("valid fixture sleep");
        format!(
            "[Console]::Write(('ivy-fixture-' + 'reading')); $null = [Console]::ReadLine(); [Console]::Write({}); Start-Sleep -Seconds {seconds}",
            literal(text),
        )
    } else if let Some(text) = program.strip_prefix("printf ") {
        let text = text
            .strip_prefix('\'')
            .and_then(|t| t.strip_suffix('\''))
            .unwrap_or(text)
            .replace("\\n", "\n")
            .replace("\\r", "\r");
        format!("[Console]::Write({})", literal(&text))
    } else {
        panic!("unsupported Windows test fixture: {program}");
    };
    prefix.push_str(&body);
    prefix
}

#[test]
fn directory_and_input_fixtures_set_cwd_and_record_received_utf8_without_bom() {
    use super::*;
    let dir = panel_contract::PanelDir::create().unwrap();
    let folder = dir.path().join("space ' quote [brackets] ü");
    std::fs::create_dir(&folder).unwrap();
    let filename = format!("received-{}-bytes", dir.id());
    let received = folder.join(&filename);
    let script = format!("cd '{}' && exec cat > '{}'", folder.display(), filename);
    let mut pane = pane_with(&script, 0);
    let until = Instant::now() + Duration::from_secs(10);
    while !received.exists() || pane.screen.working_directory().is_none() {
        pane.pump(Default::default());
        assert!(
            Instant::now() < until,
            "fixture did not prepare its directory or open its witness: {}",
            pane.screen.to_text()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(pane.screen.working_directory(), Some(folder.as_path()));
    assert_eq!(std::fs::read(&received).unwrap(), b"");
    let text = "first ü\nsecond line\n";
    pane.pty.write(text.as_bytes()).unwrap();
    while std::fs::read(&received).unwrap() != text.as_bytes() {
        pane.pump(Default::default());
        assert!(
            Instant::now() < until,
            "child did not receive UTF-8 input: {:?}",
            std::fs::read(&received)
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn gated_output_is_written_only_after_the_child_receives_input() {
    use super::*;
    let mut pane = pane_with("read -r line; printf fixture-ready; sleep 30", 0);
    let ready = Instant::now() + Duration::from_secs(10);
    while !pane.screen.to_text().contains("ivy-fixture-reading") {
        pane.pump(Default::default());
        assert!(Instant::now() < ready, "child did not reach its read gate");
        std::thread::sleep(Duration::from_millis(2));
    }
    let before_input = Instant::now() + Duration::from_millis(200);
    while Instant::now() < before_input {
        pane.pump(Default::default());
        assert!(!pane.screen.to_text().contains("fixture-ready"));
        std::thread::sleep(Duration::from_millis(2));
    }
    pane.pty.write(b"go\n").unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !pane.screen.to_text().contains("fixture-ready") {
        pane.pump(Default::default());
        assert!(
            Instant::now() < until,
            "no input-gated child output: {}",
            pane.screen.to_text()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
