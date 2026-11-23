use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use regex::Regex;
use serde_json::Value;
use serde_json::json;

use crate::handle::Handle;
use crate::handle::Kind;
use crate::handle::Supervisor;
use crate::launch;
use crate::launch::ServerOptions;
use crate::log_file;
use crate::log_file::Waited;
use crate::screenshot;

pub enum Output {
    Text(String),
    Image { path: PathBuf, png: Vec<u8> },
}

impl Output {
    pub fn to_mcp(&self, is_error: bool) -> Value {
        match self {
            Self::Text(text) => json!({
                "content": [{ "type": "text", "text": text }],
                "isError": is_error,
            }),
            Self::Image { path, png } => json!({
                "content": [
                    { "type": "image", "mimeType": "image/png",
                      "data": base64::engine::general_purpose::STANDARD.encode(png) },
                    { "type": "text", "text": path.display().to_string() },
                ],
                "isError": is_error,
            }),
        }
    }
}

pub fn list() -> Value {
    let workdir = json!({ "type": "string", "description": "absolute path of the worktree; default: where the MCP server started" });
    let id = json!({ "type": "string", "description": "handle id, for example s1" });
    let env = json!({ "type": "object", "description": "extra environment, for example {\"MCRS_VIEW\":\"8\"}" });
    json!([
        tool(
            "build",
            "cargo build --release of the server and the client in workdir; returns ok or the first error lines",
            json!({ "workdir": workdir, "targets": { "type": "array", "items": { "enum": ["server", "client"] } } }),
            &[]
        ),
        tool(
            "server_start",
            "start a server on a free port in its own run directory; then wait for server_ready. kind=vanilla starts the pinned vanilla server (EULA accepted)",
            json!({ "workdir": workdir, "kind": { "enum": ["mcrs", "vanilla"] }, "world": { "type": "string", "description": "world folder (mcrs only)" },
                    "seed": { "type": "string" }, "gamemode": { "type": "string" }, "env": env }),
            &[]
        ),
        tool(
            "client_start",
            "start the windowed client against a server handle; then wait for client_joined",
            json!({ "workdir": workdir, "server": id, "username": { "type": "string" }, "env": env }),
            &["server"]
        ),
        tool(
            "stop",
            "stop one handle, or all handles of this process with id=all",
            json!({ "id": id }),
            &["id"]
        ),
        tool(
            "logs",
            "last lines of a handle's log, optionally filtered; at most 200",
            json!({ "id": id, "grep": { "type": "string", "description": "regex" }, "level": { "enum": ["error", "warn"] },
                    "limit": { "type": "integer", "description": "default 50" } }),
            &["id"]
        ),
        tool(
            "wait",
            "block until a log line matches, the process exits, or the timeout",
            json!({ "id": id, "until": { "type": "string", "description": "server_ready, client_joined, or a regex" },
                    "timeout_s": { "type": "integer", "description": "default 120" } }),
            &["id", "until"]
        ),
        tool(
            "command",
            "run console commands on a server handle in order; returns the reply lines of each",
            json!({ "id": id, "commands": { "type": "array", "items": { "type": "string" } },
                    "window_ms": { "type": "integer", "description": "reply window per command, default 2000" } }),
            &["id", "commands"]
        ),
        tool(
            "screenshot",
            "capture the client window; returns the image and the file path",
            json!({ "id": id, "max_px": { "type": "integer", "description": "default 1024" } }),
            &["id"]
        ),
    ])
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": description,
            "inputSchema": { "type": "object", "properties": properties, "required": required } })
}

pub fn call(supervisor: &mut Supervisor, name: &str, args: &Value) -> Result<Output, String> {
    let text = |key: &str| args[key].as_str();
    let number = |key: &str, default: u64| args[key].as_u64().unwrap_or(default);
    let workdir = text("workdir").map_or_else(|| supervisor.workdir.clone(), PathBuf::from);
    let handle = || Handle::load(text("id").unwrap_or_default());
    match name {
        "build" => {
            let targets: Vec<&str> = args["targets"]
                .as_array()
                .map_or(vec!["server", "client"], |list| {
                    list.iter().filter_map(Value::as_str).collect()
                });
            launch::build(&workdir, &targets).map(Output::Text)
        }
        "server_start" => {
            let options = ServerOptions {
                world: text("world"),
                seed: text("seed"),
                gamemode: text("gamemode"),
                env: launch::env_of(&args["env"]),
            };
            let started = match text("kind") {
                Some("vanilla") => launch::vanilla(supervisor, &workdir, &options)?,
                _ => launch::server(supervisor, &workdir, options)?,
            };
            Ok(started_text(&started))
        }
        "client_start" => {
            let server = Handle::load(text("server").unwrap_or_default())?;
            let env = launch::env_of(&args["env"]);
            let started = launch::client(supervisor, &workdir, &server, text("username"), env)?;
            Ok(started_text(&started))
        }
        "stop" => {
            let handles = match text("id") {
                Some("all") => Handle::all()
                    .into_iter()
                    .filter(|handle| supervisor.owns(handle) && handle.alive())
                    .collect(),
                _ => vec![handle()?],
            };
            let report: Vec<String> = handles
                .iter()
                .map(|handle| format!("{} {}", handle.id, handle.stop()))
                .collect();
            Ok(Output::Text(report.join("\n")))
        }
        "logs" => {
            let handle = handle()?;
            let pattern = match (text("level"), text("grep")) {
                (Some("error"), _) => Some("ERROR|panicked"),
                (Some("warn"), _) => Some("ERROR|WARN|panicked"),
                (_, grep) => grep,
            };
            let filter = pattern.map(regex).transpose()?;
            let limit = usize::try_from(number("limit", 50).min(200)).unwrap_or(50);
            let lines = log_file::lines(&handle.log(), 0);
            Ok(Output::Text(
                log_file::tail(&lines, filter.as_ref(), limit).join("\n"),
            ))
        }
        "wait" => {
            let handle = handle()?;
            let until = text("until").unwrap_or_default();
            let pattern = regex(condition(handle.kind, until).unwrap_or(until))?;
            let timeout = Duration::from_secs(number("timeout_s", 120));
            match log_file::wait(&handle, &pattern, timeout) {
                Waited::Matched(line) => Ok(Output::Text(line)),
                Waited::TimedOut(tail) => Err(format!("timeout; last lines:\n{}", tail.join("\n"))),
                Waited::Exited(tail) => Err(format!(
                    "{} exited; last lines:\n{}",
                    handle.id,
                    tail.join("\n")
                )),
            }
        }
        "command" => {
            let window = Duration::from_millis(number("window_ms", 2000));
            run_commands(&handle()?, &args["commands"], window)
        }
        "screenshot" => {
            let handle = handle()?;
            let max_px = u32::try_from(number("max_px", 1024)).unwrap_or(1024);
            screenshot::capture(&handle, max_px)
        }
        other => Err(format!("unknown tool {other}")),
    }
}

fn run_commands(handle: &Handle, commands: &Value, window: Duration) -> Result<Output, String> {
    let commands: Vec<&str> = match commands {
        Value::String(one) => vec![one.as_str()],
        list => list
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default(),
    };
    let mut report = Vec::new();
    for command in commands {
        let from = log_file::len(&handle.log());
        handle.send(command)?;
        let replies = log_file::replies(handle, from, |line| reply(handle.kind, line), window);
        report.push(json!({ "command": command, "reply": replies }));
    }
    Ok(Output::Text(Value::Array(report).to_string()))
}

fn started_text(handle: &Handle) -> Output {
    Output::Text(
        json!({ "id": handle.id, "pid": handle.pid, "port": handle.port,
                "dir": handle.dir(), "log": handle.log() })
        .to_string(),
    )
}

fn condition(kind: Kind, until: &str) -> Option<&'static str> {
    match (kind, until) {
        (Kind::Server, "server_ready") => Some("entering Playing state"),
        (Kind::Vanilla, "server_ready") => Some(r"Done \(.*\)! For help"),
        (Kind::Client, "client_joined") => Some("mcrs_minecraft_network::join: configured by"),
        _ => None,
    }
}

fn reply(kind: Kind, line: &str) -> Option<String> {
    let text = match kind {
        Kind::Server => line.split_once("mcrs_console: ")?.1,
        Kind::Vanilla => {
            let text = line.split_once("]: ")?.1;
            text.strip_prefix("System chat: ").unwrap_or(text)
        }
        Kind::Client => line,
    };
    Some(text.to_owned())
}

fn regex(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|error| error.to_string())
}
