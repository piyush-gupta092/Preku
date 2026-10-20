//! `preku` — command-line helper for the Preku desktop app (PRD M7).
//!
//!     preku watch cargo build --release
//!
//! Runs the command with your terminal attached, and tells the Preku island when it starts
//! and finishes (the island pops out with ✓ or ✗ and the duration). If Preku isn't running,
//! the command still runs normally. The exit code is passed through unchanged.
//!
//! The channel must match `desktop/src-tauri/src/modules/monitor/platform/*.rs`.

use std::{
    env,
    io::Write,
    process::{Command, ExitCode},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;

const USAGE: &str = "usage: preku watch <command> [args...]\n\n\
Runs the command and notifies the Preku island when it finishes.";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.split_first() {
        Some((sub, rest)) if sub == "watch" && !rest.is_empty() => watch(rest),
        Some((flag, _)) if flag == "--version" || flag == "-V" => {
            println!("preku {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn watch(args: &[String]) -> ExitCode {
    let display = args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
    let id = format!("{}-{}", std::process::id(), now_ms());

    let mut channel = channel::connect();
    if channel.is_none() {
        eprintln!("preku: the Preku app isn't running; running without notifications.");
    }
    send(
        &mut channel,
        &json!({ "event": "started", "id": id, "command": display }),
    );

    let started = Instant::now();
    let code = match command(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("preku: couldn't run `{}`: {err}", args[0]);
            127
        }
    };
    send(
        &mut channel,
        &json!({
            "event": "finished",
            "id": id,
            "code": code,
            "durationMs": started.elapsed().as_millis() as u64,
        }),
    );
    ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1))
}

/// On Windows, run through `cmd /C` so `npm`, `yarn` and other `.cmd` shims work.
/// Elsewhere, execute the program directly (wrap in `sh -c` yourself for shell syntax).
fn command(args: &[String]) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Pass the line to cmd verbatim: Rust's own argument quoting would escape our quotes
        // again. `/S /C "…"` makes cmd strip exactly the outer pair and run the rest as typed.
        let line = args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
        let mut cmd = Command::new("cmd");
        cmd.raw_arg(format!("/S /C \"{line}\""));
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        cmd
    }
}

/// Quotes an argument that contains spaces or quotes, for display and `cmd /C`.
fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    format!("\"{}\"", arg.replace('"', "\\\""))
}

/// Best effort: a failed notification must never affect the command or its exit code.
fn send(channel: &mut Option<Box<dyn Write>>, message: &serde_json::Value) {
    let Some(stream) = channel else {
        return;
    };
    let ok = writeln!(stream, "{message}")
        .and_then(|()| stream.flush())
        .is_ok();
    if !ok {
        *channel = None;
    }
}

#[cfg(windows)]
mod channel {
    use std::{fs::OpenOptions, io::Write};

    pub fn connect() -> Option<Box<dyn Write>> {
        let user: String = std::env::var("USERNAME")
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let pipe = format!(r"\\.\pipe\preku-watch-{user}");
        let file = OpenOptions::new().write(true).open(pipe).ok()?;
        Some(Box::new(file))
    }
}

#[cfg(unix)]
mod channel {
    use std::{io::Write, os::unix::net::UnixStream, path::PathBuf};

    pub fn connect() -> Option<Box<dyn Write>> {
        let path = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(dir) => PathBuf::from(dir).join("preku-watch.sock"),
            None => {
                let user = std::env::var("USER").unwrap_or_default();
                std::env::temp_dir().join(format!("preku-watch-{user}.sock"))
            }
        };
        Some(Box::new(UnixStream::connect(path).ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote("cargo"), "cargo");
        assert_eq!(quote("my file.txt"), "\"my file.txt\"");
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quote(""), "\"\"");
    }

    #[test]
    fn missing_channel_is_harmless() {
        let mut channel = None;
        send(&mut channel, &json!({ "event": "started" }));
        assert!(channel.is_none());
    }

    #[test]
    fn exit_codes_pass_through() {
        let args: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/c".into(), "exit 3".into()]
        } else {
            vec!["sh".into(), "-c".into(), "exit 3".into()]
        };
        let status = command(&args).status().unwrap();
        assert_eq!(status.code(), Some(3));
    }
}
