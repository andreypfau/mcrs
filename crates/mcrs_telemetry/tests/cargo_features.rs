//! Workspace-manifest regression guard for unconditional Tracy feature pull.
//!
//! Asserts that the workspace root `Cargo.toml` does NOT enable
//! `bevy_log/tracing-tracy` unconditionally. The check is intentionally
//! skipped when `--features=telemetry-tracy` is active because under that
//! flag the unconditional dependency pull is the expected path.

#![cfg(not(feature = "telemetry-tracy"))]

use std::path::Path;

#[test]
fn workspace_manifest_does_not_unconditionally_enable_bevy_log_tracing_tracy() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");

    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", manifest_path.display()));

    for line in manifest.lines() {
        if line.contains("bevy_log") && line.contains("tracing-tracy") {
            panic!(
                "workspace Cargo.toml must not enable bevy_log/tracing-tracy \
                 unconditionally; offending line: {line}"
            );
        }
    }
}
