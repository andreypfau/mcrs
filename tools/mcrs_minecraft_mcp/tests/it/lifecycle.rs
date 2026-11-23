use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use crate::support::Sandbox;
use crate::support::fake_workdir;
use crate::support::gone_within;

#[test]
fn two_servers_start_answer_and_stop() {
    let sandbox = Sandbox::new("lifecycle");
    let started: Vec<Value> = (0..2)
        .map(|_| {
            let text = sandbox.ok("server_start", &json!({ "workdir": fake_workdir() }));
            serde_json::from_str(&text).unwrap()
        })
        .collect();
    assert_ne!(started[0]["port"], started[1]["port"]);
    assert_ne!(started[0]["id"], started[1]["id"]);
    let id = started[0]["id"].as_str().unwrap();

    let ready = sandbox.ok(
        "wait",
        &json!({ "id": id, "until": "server_ready", "timeout_s": 10 }),
    );
    assert!(ready.contains("entering Playing state"), "{ready}");

    let tail = sandbox.ok("logs", &json!({ "id": id }));
    assert_eq!(tail.lines().count(), 50, "{tail}");
    assert!(tail.ends_with("entering Playing state\n"), "{tail}");

    let errors = sandbox.ok("logs", &json!({ "id": id, "level": "error" }));
    let port = started[0]["port"].to_string();
    assert_eq!(
        errors.trim(),
        format!("ERROR something broke on port {port}")
    );

    let replies = sandbox.ok(
        "command",
        &json!({ "id": id, "commands": ["say hi", "list"] }),
    );
    let replies: Value = serde_json::from_str(&replies).unwrap();
    assert_eq!(replies[0]["reply"], json!(["say hi"]));
    assert_eq!(replies[1]["reply"], json!(["list"]));

    let timeout = sandbox.run(
        "wait",
        &json!({ "id": id, "until": "never printed", "timeout_s": 1 }),
    );
    assert!(!timeout.status.success());
    let message = String::from_utf8_lossy(&timeout.stderr);
    assert!(message.starts_with("timeout"), "{message}");
    assert!(message.contains("entering Playing state"), "{message}");

    let stopped = sandbox.ok("stop", &json!({ "id": "all" }));
    assert_eq!(stopped.lines().count(), 2, "{stopped}");
    for server in &started {
        let pid = server["pid"].as_u64().unwrap();
        assert!(
            gone_within(pid, Duration::from_secs(1)),
            "{pid} survived stop"
        );
    }
}
