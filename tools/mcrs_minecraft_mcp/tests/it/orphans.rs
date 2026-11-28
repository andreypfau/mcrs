use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use crate::support::Sandbox;
use crate::support::alive;
use crate::support::fake_workdir;
use crate::support::gone_within;

#[test]
fn killing_the_mcp_server_stops_its_children() {
    let sandbox = Sandbox::new("orphans");
    let mut server = sandbox
        .command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = server.stdin.take().unwrap();
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    let start = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "server_start", "arguments": { "workdir": fake_workdir() } } });
    writeln!(stdin, "{start}").unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    let pid = serde_json::from_str::<Value>(text).unwrap()["pid"]
        .as_u64()
        .unwrap();
    assert!(alive(pid));

    server.kill().unwrap();
    server.wait().unwrap();
    assert!(
        gone_within(pid, Duration::from_secs(5)),
        "{pid} outlived the MCP server"
    );
}
