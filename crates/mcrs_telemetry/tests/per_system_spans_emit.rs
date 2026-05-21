//! Asserts that per-system tracing spans fire when `telemetry-tracy` is active.
//!
//! `bevy_ecs/trace` is enabled workspace-wide, so Bevy emits a `"system"` span
//! for every system invocation. This test verifies that the span actually reaches
//! the process-global subscriber, which propagates to TaskPool worker threads.

#![cfg(feature = "telemetry-tracy")]

mod common;

use bevy_app::{App, TaskPoolPlugin, Update};

fn no_op_system() {}

#[test]
fn per_system_spans_emit_under_telemetry_tracy() {
    common::install_global_capture();
    let (_guard, buffer) = common::lock_and_clear();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_systems(Update, no_op_system);
    app.update();

    let captured = buffer.lock().unwrap();
    assert!(
        captured.iter().any(|s| s.name == "system"),
        "expected at least one \"system\" span; captured: {:?}",
        captured.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}
