use std::io::BufRead;
use std::io::Write;

use serde_json::Value;
use serde_json::json;

const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Builds, starts, waits on and stops the mcrs server, a vanilla server and the mcrs client. \
Start a server, then `wait` for `server_ready` instead of a sleep. Pass `workdir` = your worktree.";

pub fn serve(tools: &Value, mut call: impl FnMut(&str, &Value) -> Value) {
    let mut stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(&line, tools, &mut call)
            && writeln!(stdout, "{reply}")
                .and_then(|()| stdout.flush())
                .is_err()
        {
            break;
        }
    }
}

pub fn handle_line(
    line: &str,
    tools: &Value,
    call: &mut impl FnMut(&str, &Value) -> Value,
) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(error) => return Some(error_reply(&Value::Null, -32700, &error.to_string())),
    };
    let id = message.get("id")?;
    let params = &message["params"];
    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => json!({
            "protocolVersion": negotiate(params),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "mcrs", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools }),
        "tools/call" => call(
            params["name"].as_str().unwrap_or_default(),
            &params["arguments"],
        ),
        method => {
            return Some(error_reply(
                id,
                -32601,
                &format!("Method not found: {method}"),
            ));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn negotiate(params: &Value) -> &'static str {
    let wanted = params["protocolVersion"].as_str();
    VERSIONS
        .into_iter()
        .find(|version| Some(*version) == wanted)
        .unwrap_or(VERSIONS[0])
}

fn error_reply(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_request_gets_its_answer() {
        let tools = json!([{ "name": "logs" }]);
        let mut call = |name: &str, arguments: &Value| json!({ "called": name, "with": arguments });
        let cases = [
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
                Some(json!({ "id": 1, "protocolVersion": "2025-03-26" })),
            ),
            (
                r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#,
                Some(json!({ "id": 2, "protocolVersion": "2025-06-18" })),
            ),
            (
                r#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#,
                Some(json!({ "id": "a", "result": {} })),
            ),
            (
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
                Some(json!({ "id": 3, "result": { "tools": [{ "name": "logs" }] } })),
            ),
            (
                r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"logs","arguments":{"id":"s1"}}}"#,
                Some(json!({ "id": 4, "result": { "called": "logs", "with": { "id": "s1" } } })),
            ),
            (
                r#"{"jsonrpc":"2.0","id":5,"method":"resources/list"}"#,
                Some(json!({ "id": 5, "error": -32601 })),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                None,
            ),
            (
                r#"{"jsonrpc":"2.0","id":6,"#,
                Some(json!({ "id": null, "error": -32700 })),
            ),
        ];
        for (line, expected) in cases {
            let reply = handle_line(line, &tools, &mut call);
            let Some(expected) = expected else {
                assert_eq!(reply, None, "{line}");
                continue;
            };
            let reply = reply.unwrap_or_else(|| panic!("no reply to {line}"));
            assert_eq!(reply["id"], expected["id"], "{line}");
            if let Some(version) = expected.get("protocolVersion") {
                assert_eq!(&reply["result"]["protocolVersion"], version, "{line}");
            }
            if let Some(result) = expected.get("result") {
                assert_eq!(&reply["result"], result, "{line}");
            }
            if let Some(code) = expected.get("error") {
                assert_eq!(&reply["error"]["code"], code, "{line}");
            }
        }
    }
}
