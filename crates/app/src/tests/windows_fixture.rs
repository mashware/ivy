//! The small fixture vocabulary used by cross-platform console tests.

// Windows cooked input waits for CR, while the original fixtures send LF. Read the child's
// real console in VT character mode and recognize either terminator ourselves; ReadConsoleW
// preserves Unicode without depending on PowerShell's input code page. Disable echo too,
// so only a fixture that actually reads and writes can produce its output witness. Non-VT
// ReadConsoleW drops LF even with line input off (microsoft/terminal src/host/stream.cpp).
// https://learn.microsoft.com/en-us/windows/console/setconsolemode
// https://learn.microsoft.com/en-us/windows/console/readconsole
const CONSOLE_INPUT: &str = r#"Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class IvyFixtureInput {
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr GetStdHandle(int kind);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetConsoleMode(IntPtr handle, out uint mode);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetConsoleMode(IntPtr handle, uint mode);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    static extern bool ReadConsoleW(IntPtr handle, StringBuilder buffer, uint length,
                                   out uint read, IntPtr control);
    static IntPtr input;
    static bool afterCarriageReturn;
    public static void Prepare() {
        input = GetStdHandle(-10);
        uint mode;
        if (!GetConsoleMode(input, out mode) || !SetConsoleMode(input, (mode & ~6u) | 0x200u))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        PrepareOutput();
    }
    public static void PrepareOutput() {
        // Noninteractive PowerShell does not guarantee VT output processing. The live
        // line-paint fixture needs the console host to execute its clear/home sequences.
        IntPtr output = GetStdHandle(-11);
        uint mode;
        if (!GetConsoleMode(output, out mode) || !SetConsoleMode(output, mode | 5u))
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    public static string ReadLine() {
        var line = new StringBuilder();
        var character = new StringBuilder(2);
        while (true) {
            uint read;
            character.Length = 0;
            if (!ReadConsoleW(input, character, 1, out read, IntPtr.Zero))
                throw new Win32Exception(Marshal.GetLastWin32Error());
            if (read == 0) return line.Length == 0 ? null : line.ToString();
            char value = character[0];
            bool skipLineFeed = afterCarriageReturn && value == '\n';
            afterCarriageReturn = value == '\r';
            if (skipLineFeed) continue;
            if (value == '\r' || value == '\n') return line.ToString();
            line.Append(value);
        }
    }
}
'@
"#;

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
        prefix.push_str(CONSOLE_INPUT);
        prefix.push_str("[IvyFixtureInput]::Prepare(); ");
        "while ($null -ne ($line = [IvyFixtureInput]::ReadLine())) { [Console]::WriteLine($line) }"
            .to_owned()
    } else if program
        == r#"while IFS= read -r line; do printf '\033[?25l\033[2J\033[H%s' "$line"; done"#
    {
        prefix.push_str(CONSOLE_INPUT);
        prefix.push_str("[IvyFixtureInput]::Prepare(); ");
        // The producer owns the visible path, so ConPTY can repaint it after a resize.
        "[Console]::Write(('ivy-fixture-' + 'reading')); while ($null -ne ($line = [IvyFixtureInput]::ReadLine())) { [Console]::Write(([string][char]27 + '[?25l' + [char]27 + '[2J' + [char]27 + '[H' + $line)) }".to_owned()
    } else if let Some(file) = program
        .strip_prefix("cat > '")
        .and_then(|s| s.strip_suffix('\''))
    {
        prefix.push_str(CONSOLE_INPUT);
        prefix.push_str("[IvyFixtureInput]::Prepare(); ");
        // The reader consumes the console's line ending; write UTF-8 plus LF explicitly, without
        // PowerShell's BOM or CRLF, so the file witnesses exactly the input these tests send.
        format!(
            "$file = [IO.File]::Open({}, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite); try {{ while ($null -ne ($line = [IvyFixtureInput]::ReadLine())) {{ $bytes = [Text.Encoding]::UTF8.GetBytes($line + [char]10); $file.Write($bytes, 0, $bytes.Length); $file.Flush() }} }} finally {{ $file.Dispose() }}",
            literal(file),
        )
    } else if let Some(rest) = program.strip_prefix("read -r line; printf ") {
        let (text, seconds) = rest.split_once("; sleep ").expect("input-gated fixture");
        let seconds = seconds.parse::<u64>().expect("valid fixture sleep");
        prefix.push_str(CONSOLE_INPUT);
        prefix.push_str("[IvyFixtureInput]::Prepare(); ");
        format!(
            "[Console]::Write(('ivy-fixture-' + 'reading')); $null = [IvyFixtureInput]::ReadLine(); [Console]::Write({}); Start-Sleep -Seconds {seconds}",
            literal(text),
        )
    } else if let Some(text) = program.strip_prefix("printf ") {
        // A live child must retain its output across ConPTY resize repaints, just as the
        // Unix printf-and-sleep fixture does. Parse only its bounded trailing sleep.
        let (text, sleep) = text
            .rsplit_once("; sleep ")
            .and_then(|(text, seconds)| seconds.parse::<u64>().ok().map(|s| (text, Some(s))))
            .unwrap_or((text, None));
        let text = text
            .strip_prefix('\'')
            .and_then(|t| t.strip_suffix('\''))
            .unwrap_or(text)
            .replace("\\n", "\n")
            .replace("\\r", "\r");
        if text.contains('\u{1b}') {
            prefix.push_str(CONSOLE_INPUT);
            prefix.push_str("[IvyFixtureInput]::PrepareOutput(); ");
        }
        let mut output = format!("[Console]::Write({})", literal(&text));
        if let Some(seconds) = sleep {
            output.push_str(&format!("; Start-Sleep -Seconds {seconds}"));
        }
        output
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
    // Directory preparation must not consume the actual byte-delivery witness's budget.
    let until = Instant::now() + Duration::from_secs(10);
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
