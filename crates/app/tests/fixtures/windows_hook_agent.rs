//! Standalone child: file witnesses cannot be confused with ConPTY echo.
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

struct Bridge(std::process::Child);
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn result(text: impl AsRef<[u8]>) -> std::io::Result<()> {
    fs::write("result.tmp", text)?;
    fs::rename("result.tmp", "result")
}

fn main() {
    if let Err(error) = run() {
        let _ = fs::write("failure", error.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        "launch-witness",
        format!(
            "{}\n{}",
            std::env::args_os().skip(1).count(),
            std::env::current_dir()?.display()
        ),
    )?;
    let helper = fs::read_to_string("helper-path")?;
    if std::env::var_os("IVY_FIXTURE_NESTED").is_some() {
        print!("{}", publish(helper.trim())?);
        return Ok(());
    }
    let bridge = fs::read_to_string("bridge-path")?;
    let mut bridge = Bridge(
        Command::new(bridge.trim())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut input = bridge.0.stdin.take().unwrap();
    let mut output = BufReader::new(bridge.0.stdout.take().unwrap());
    fs::write("bridge-pid", bridge.0.id().to_string())?;
    writeln!(
        input,
        "{}",
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize"}"#
    )?;
    input.flush()?;
    let mut initialized = String::new();
    output.read_line(&mut initialized)?;
    if !initialized.contains("capabilities") {
        return Err(format!("MCP initialization failed: {initialized}").into());
    }
    fs::write("ready", std::process::id().to_string())?;
    loop {
        let command = match fs::read_to_string("command") {
            Ok(command) => command,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        fs::remove_file("command")?;
        match command.trim() {
            "hold" => {
                writeln!(
                    input,
                    "{}",
                    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"panel_set","arguments":{"source":"flow","document":{"header":false,"milestone":"m","attention":"wait","actions":[{"label":"Approve","send":"yes"}],"lines":["Before"]}}}}"#
                )?;
                input.flush()?;
                let mut reply = String::new();
                output.read_line(&mut reply)?;
                result(reply)?;
            }
            "publish" => {
                result(publish(helper.trim())?)?;
            }
            "nested" => {
                let nested = Command::new(std::env::current_exe()?)
                    .env("IVY_FIXTURE_NESTED", "1")
                    .output()?;
                result(format!(
                    "{}\n{}",
                    nested.status,
                    String::from_utf8_lossy(&nested.stdout)
                ))?;
            }
            "exit" => break,
            _ => return Err("unknown fixture command".into()),
        }
    }
    drop(input);
    Ok(())
}

fn publish(helper: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = Command::new(helper)
        .arg("publish")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().unwrap().write_all(
        br#"{"phase":"build","lines":["After"],"actions":[{"label":"Injected","send":"evil"}]}"#,
    )?;
    let output = child.wait_with_output()?;
    Ok(format!(
        "{}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}
