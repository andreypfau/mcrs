use std::process::Output;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use crate::support::Sandbox;
use crate::support::alive;
use crate::support::fake_workdir;
use crate::support::gone_within;

fn run(sandbox: &Sandbox, session: &str, tool: &str, arguments: &Value) -> Output {
    sandbox
        .command()
        .args(["--session", session, tool, &arguments.to_string()])
        .output()
        .unwrap()
}

fn start(sandbox: &Sandbox, session: &str) -> Value {
    let output = run(
        sandbox,
        session,
        "server_start",
        &json!({ "workdir": fake_workdir() }),
    );
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn a_session_stops_only_its_own_handles_unless_forced() {
    let sandbox = Sandbox::new("sessions");
    let mine = start(&sandbox, "a");
    let theirs = start(&sandbox, "b");
    let their_pid = theirs["pid"].as_u64().unwrap();

    let listed = run(&sandbox, "a", "list", &json!({}));
    let listed = String::from_utf8(listed.stdout).unwrap();
    assert_eq!(listed.lines().count(), 1, "{listed}");
    assert!(listed.contains(mine["id"].as_str().unwrap()), "{listed}");
    let everyone = run(&sandbox, "a", "list", &json!({ "everyone": true }));
    assert_eq!(
        String::from_utf8(everyone.stdout).unwrap().lines().count(),
        2
    );

    let stopped = run(&sandbox, "a", "stop", &json!({ "id": "all" }));
    let stopped = String::from_utf8(stopped.stdout).unwrap();
    assert_eq!(stopped.lines().count(), 1, "{stopped}");
    assert!(gone_within(
        mine["pid"].as_u64().unwrap(),
        Duration::from_secs(1)
    ));
    assert!(alive(their_pid), "stop all of a stopped the handle of b");

    let refused = run(&sandbox, "a", "stop", &json!({ "id": theirs["id"] }));
    assert!(!refused.status.success());
    assert!(alive(their_pid), "a stopped the handle of b without force");

    let forced = run(
        &sandbox,
        "a",
        "stop",
        &json!({ "id": theirs["id"], "force": true }),
    );
    assert!(forced.status.success(), "{forced:?}");
    assert!(gone_within(their_pid, Duration::from_secs(1)));
}
