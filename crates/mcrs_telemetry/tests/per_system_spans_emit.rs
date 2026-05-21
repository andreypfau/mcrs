//! Asserts that per-system tracing spans fire when `telemetry-tracy` is active.
//!
//! `bevy_ecs/trace` is enabled workspace-wide, so Bevy emits a `"system"` span
//! for every system invocation. This test verifies that span actually reaches a
//! per-test subscriber — covering TELEMETRY-03.

#![cfg(feature = "telemetry-tracy")]

use std::sync::{Arc, Mutex};

use bevy_app::{App, TaskPoolPlugin, Update};
use tracing::subscriber::with_default;
use tracing_subscriber::{layer::SubscriberExt, Registry};

struct SpanCaptureLayer {
    names: Arc<Mutex<Vec<String>>>,
}

impl<S> tracing_subscriber::Layer<S> for SpanCaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.names
            .lock()
            .unwrap()
            .push(attrs.metadata().name().to_string());
    }
}

fn no_op_system() {}

#[test]
fn per_system_spans_emit_under_telemetry_tracy() {
    let names: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let layer = SpanCaptureLayer {
        names: names.clone(),
    };
    let subscriber = Registry::default().with(layer);

    with_default(subscriber, || {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_systems(Update, no_op_system);
        app.update();
    });

    let captured = names.lock().unwrap();
    assert!(
        captured.iter().any(|n| n == "system"),
        "expected at least one \"system\" span; captured: {captured:?}"
    );
}
