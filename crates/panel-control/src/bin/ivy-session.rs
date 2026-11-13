//! Metadata-only adapter for official client reporters. Authentication stays inherited.

use panel_control::session::{self, Client, Request};
use std::io::{Read, Write};

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.as_slice() == [std::ffi::OsString::from("publish")] {
        publish(std::io::stdin().lock());
        return;
    }
    let result = if panel_contract::ENV_NAMES
        .iter()
        .any(|name| std::env::var_os(name).is_none_or(|v| v.is_empty()))
    {
        Ok(serde_json::json!({"ok":false,"error":"outside_ivy"}))
    } else {
        request(args, std::io::stdin().lock()).and_then(|request| {
            session::exchange(&request)
                .map_err(|_| "cannot reach this pane's session service".into())
        })
    };
    let reply = match result {
        Ok(reply) => reply,
        Err(message) => {
            eprintln!("ivy-session: {message}");
            serde_json::json!({"ok":false,"error":"helper_error","message":message})
        }
    };
    let succeeded = reply["ok"] == true;
    let written = writeln!(std::io::stdout().lock(), "{reply}");
    if !succeeded || written.is_err() {
        std::process::exit(1);
    }
}

fn publish(mut input: impl Read) {
    use panel_control::client::whole_identity;
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    if !panel_control::endpoint::BINDS || whole_identity(id.as_deref(), dir.as_deref()).is_none() {
        return;
    }
    let bytes = match read_input(&mut input, panel_contract::MAX_BYTES as usize) {
        Ok(bytes) => bytes,
        Err(message) => {
            eprintln!("ivy-session: {message}");
            std::process::exit(2);
        }
    };
    let document = match session::publish_document(&bytes) {
        Ok(document) => document,
        Err(message) => {
            eprintln!("ivy-session: {message}");
            std::process::exit(2);
        }
    };
    let line = format!("{{\"tool\":\"panel_publish\",\"document\":{document}}}\n");
    let reply = match session::exchange_line(line.as_bytes()) {
        Ok(reply) => reply,
        Err(_) => {
            eprintln!("ivy-session: cannot reach this pane's publication service");
            std::process::exit(1);
        }
    };
    if reply["ok"] == true {
        println!("{{\"ok\":true}}");
        return;
    }
    if reply["code"] == "unheld" {
        println!("{{\"ok\":false,\"code\":\"unheld\",\"message\":\"this panel has no holder\"}}");
        return;
    }
    let message = reply["message"]
        .as_str()
        .or_else(|| reply["error"].as_str())
        .unwrap_or("publication refused");
    eprintln!("ivy-session: {message}");
    std::process::exit(1);
}

fn request(args: Vec<std::ffi::OsString>, mut input: impl Read) -> Result<Request, String> {
    let args: Vec<&str> = args
        .iter()
        .map(|arg| arg.to_str().ok_or("invalid helper arguments"))
        .collect::<Result<_, _>>()?;
    let request = match args.as_slice() {
        ["hook", "codex"] => {
            let bytes = read_input(&mut input, session::MAX_HOOK_INPUT)?;
            session::hook_request(&bytes)?
        }
        ["attach", client] => Request::Attach {
            client: Client::parse(client)?,
        },
        ["probe", generation] => Request::Probe {
            generation: (*generation).into(),
        },
        ["update", generation] => {
            let bytes = read_input(&mut input, session::MAX_REQUEST)?;
            session::update_request((*generation).into(), &bytes)?
        }
        _ => return Err(
            "usage: ivy-session publish | hook codex | attach CLIENT | probe GENERATION | update GENERATION"
                .into(),
        ),
    };
    request.validate()?;
    Ok(request)
}

fn read_input(input: &mut impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    input
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read session metadata")?;
    if bytes.len() > limit {
        return Err("session metadata goes over the ceiling".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
        values.iter().map(std::ffi::OsString::from).collect()
    }

    #[test]
    fn inherited_endpoint_has_no_command_line_override() {
        for values in [
            vec!["attach", "codex"],
            vec!["attach", "claude", "--endpoint", "/tmp/other"],
            vec!["probe", "g", "--pane", "other"],
            vec!["hook", "claude"],
            vec!["sh", "-c", "anything"],
        ] {
            assert!(request(args(&values), &b""[..]).is_err());
        }
        assert_eq!(
            request(args(&["attach", "claude"]), &b""[..]).unwrap(),
            Request::Attach {
                client: Client::Claude
            }
        );
        assert!(request(args(&["probe", "g"]), &b""[..]).is_ok());
    }

    #[test]
    fn input_is_bounded_before_json_parsing() {
        assert!(
            request(
                args(&["hook", "codex"]),
                &vec![b' '; session::MAX_HOOK_INPUT + 1][..]
            )
            .is_err()
        );
        assert!(
            request(
                args(&["update", "g"]),
                &vec![b' '; session::MAX_REQUEST + 1][..]
            )
            .is_err()
        );
        let update = br#"{"sequence":1,"session_id":null,"cwd":null,"challenge":"close-1"}"#;
        assert!(request(args(&["update", "g"]), &update[..]).is_ok());
        assert!(
            request(
                args(&["update", "g"]),
                &b"{\"sequence\":1,\"pid\":1234}"[..]
            )
            .is_err()
        );
    }
}
