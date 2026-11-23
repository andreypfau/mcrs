use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use mcrs_minecraft_client_control::Reply;
use regex::Regex;
use serde_json::Value;
use serde_json::json;

use crate::control;
use crate::control::Session;
use crate::handle::Handle;
use crate::handle::Kind;
use crate::handle::Supervisor;
use crate::launch;
use crate::launch::ClientOptions;
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
                    "seed": { "type": "string" }, "gamemode": { "type": "string" }, "env": env,
                    "ops": { "type": "array", "items": { "type": "string" }, "description": "player names made operators (vanilla only)" } }),
            &[]
        ),
        tool(
            "client_start",
            "start the client against a server handle, windowed or headless; then wait for world_ready",
            json!({ "workdir": workdir, "server": id, "username": { "type": "string" },
                    "headless": { "type": "boolean", "description": "no window and no renderer; default false" }, "env": env }),
            &["server"]
        ),
        tool(
            "client_state",
            "read the client state as JSON: ready, player, target (the entity or block under the crosshair), entities [radius], block X Y Z",
            json!({ "id": id, "query": { "type": "string", "description": "for example player, entities 16, block 0 64 0" } }),
            &["id", "query"]
        ),
        tool(
            "client_input",
            "run input actions on the client in order, each returns when done: look YAW PITCH, look_at X Y Z, look_entity ID, hold KEYS TICKS (keys: forward,back,left,right,jump,sneak,sprint), attack [TICKS], slot N, chat TEXT, command TEXT",
            json!({ "id": id, "actions": { "type": "array", "items": { "type": "string" } } }),
            &["id", "actions"]
        ),
        tool(
            "stop",
            "stop one handle of this session, or all its handles with id=all; force=true stops a handle of another session",
            json!({ "id": id, "force": { "type": "boolean" } }),
            &["id"]
        ),
        tool(
            "list",
            "the running handles of this session, or of every session with everyone=true; drops the handles whose process is gone",
            json!({ "everyone": { "type": "boolean" } }),
            &[]
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
            "block until a condition holds, the process exits, or the timeout",
            json!({ "id": id, "until": { "type": "string", "description": "server_ready, client_joined, world_ready (joined, player placed, terrain around it loaded and meshed), on_ground (the own player stands on a block), or a log regex" },
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
            "capture the client window after world_ready; returns the image and the file path",
            json!({ "id": id, "max_px": { "type": "integer", "description": "default 1024" },
                    "wait": { "type": "boolean", "description": "wait for world_ready first; default true" } }),
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
                ops: args["ops"]
                    .as_array()
                    .map(|list| list.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default(),
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
            let options = ClientOptions {
                username: text("username"),
                headless: args["headless"] == true,
                env: launch::env_of(&args["env"]),
            };
            let started = launch::client(supervisor, &workdir, &server, options)?;
            Ok(started_text(&started))
        }
        "stop" => {
            let handles = match text("id") {
                Some("all") => Handle::all()
                    .into_iter()
                    .filter(|handle| supervisor.owns(handle) && handle.alive())
                    .collect(),
                _ => {
                    let handle = handle()?;
                    if !supervisor.owns(&handle) && args["force"] != true {
                        return Err(format!(
                            "{} belongs to the session {:?}; pass \"force\": true to stop it",
                            handle.id, handle.session
                        ));
                    }
                    vec![handle]
                }
            };
            let report: Vec<String> = handles
                .iter()
                .map(|handle| format!("{} {}", handle.id, handle.stop()))
                .collect();
            Ok(Output::Text(report.join("\n")))
        }
        "list" => Ok(listed(supervisor, args["everyone"] == true)),
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
            let timeout = Duration::from_secs(number("timeout_s", 120));
            wait(&handle()?, text("until").unwrap_or_default(), timeout)
        }
        "command" => {
            let window = Duration::from_millis(number("window_ms", 2000));
            run_commands(&handle()?, &args["commands"], window)
        }
        "screenshot" => {
            let max_px = u32::try_from(number("max_px", 1024)).unwrap_or(1024);
            ready_shot(&handle()?, args["wait"] != false, max_px)
        }
        "client_state" => {
            let query = text("query").unwrap_or("player");
            let mut session = Session::open(&handle()?)?;
            session
                .ask(query, Duration::from_secs(10))
                .map(Output::Text)
        }
        "client_input" => run_actions(&handle()?, &args["actions"]),
        other => Err(format!("unknown tool {other}")),
    }
}

fn listed(supervisor: &Supervisor, everyone: bool) -> Output {
    let mut rows = Vec::new();
    for handle in Handle::all() {
        if !everyone && !supervisor.owns(&handle) {
            continue;
        }
        if !handle.alive() {
            handle.forget();
            continue;
        }
        let row = json!({ "id": handle.id, "kind": handle.kind, "port": handle.port, "age_s": handle.age_s(),
                          "workdir": handle.workdir, "session": handle.session });
        rows.push(row.to_string());
    }
    Output::Text(rows.join("\n"))
}

fn wait(handle: &Handle, until: &str, timeout: Duration) -> Result<Output, String> {
    match (handle.kind, until) {
        (Kind::Client, "world_ready") => {
            return control::wait_until(handle, "ready", timeout, control::world_ready)
                .map(Output::Text);
        }
        (Kind::Client, "on_ground") => {
            return control::wait_until(handle, "player", timeout, control::on_ground)
                .map(Output::Text);
        }
        _ => {}
    }
    let pattern = regex(condition(handle.kind, until).unwrap_or(until))?;
    match log_file::wait(handle, &pattern, timeout) {
        Waited::Matched(line) => Ok(Output::Text(line)),
        Waited::TimedOut(tail) => Err(format!("timeout; last lines:\n{}", tail.join("\n"))),
        Waited::Exited(tail) => Err(format!(
            "{} exited; last lines:\n{}",
            handle.id,
            tail.join("\n")
        )),
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

fn ready_shot(handle: &Handle, wait: bool, max_px: u32) -> Result<Output, String> {
    if wait {
        control::wait_until(
            handle,
            "ready",
            Duration::from_secs(120),
            control::world_ready,
        )?;
    }
    screenshot::capture(handle, max_px)
}

fn run_actions(handle: &Handle, actions: &Value) -> Result<Output, String> {
    let mut session = Session::open(handle)?;
    let mut report = Vec::new();
    for action in actions
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let reply = session.ask(action, control::timeout_of(action))?;
        let failed = matches!(serde_json::from_str(&reply), Ok(Reply::Error(_)));
        report.push(format!("{action} -> {reply}"));
        if failed {
            break;
        }
    }
    Ok(Output::Text(report.join("\n")))
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
