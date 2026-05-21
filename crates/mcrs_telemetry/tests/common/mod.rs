//! Process-global tracing capture layer shared by integration tests in this crate.
//!
//! Uses `set_global_default` so spans emitted on Bevy TaskPool worker threads
//! are visible to the test assertions — thread-local defaults do not propagate
//! to threads spawned after the subscriber is installed.
//!
//! Each test acquires the capture lock, clears the shared buffer, runs the
//! fixture, and asserts against the drained buffer. The lock is held across
//! the entire observation window to prevent interleaved captures from
//! concurrent test runs.

#![cfg(feature = "telemetry-tracy")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan};

// ── captured-span type ───────────────────────────────────────────────────────

#[derive(Default)]
pub struct CapturedSpan {
    pub name: String,
    pub fields: HashMap<String, String>,
    /// Field names declared in the span's metadata (includes `Empty` fields).
    pub declared_fields: Vec<String>,
    /// Nearest ancestor span that carried a `dim` field, if any.
    pub parent_dim: Option<String>,
}

// ── field visitor ────────────────────────────────────────────────────────────

struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
}

// ── per-span extension to expose fields to child spans ───────────────────────

struct RecordedFields {
    fields: HashMap<String, String>,
}

// ── capture layer ────────────────────────────────────────────────────────────

pub struct CaptureLayer {
    buffer: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl<S> tracing_subscriber::Layer<S> for CaptureLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = HashMap::new();
        attrs.record(&mut FieldVisitor(&mut fields));

        let declared_fields = attrs
            .metadata()
            .fields()
            .iter()
            .map(|f| f.name().to_string())
            .collect::<Vec<_>>();

        if let Some(span_ref) = ctx.span(id) {
            span_ref
                .extensions_mut()
                .insert(RecordedFields { fields: fields.clone() });
        }

        let mut parent_dim: Option<String> = None;
        if let Some(span_ref) = ctx.span(id) {
            for parent in span_ref.scope().skip(1) {
                if let Some(recorded) = parent.extensions().get::<RecordedFields>() {
                    if let Some(dim_val) = recorded.fields.get("dim") {
                        parent_dim = Some(dim_val.clone());
                        break;
                    }
                }
            }
        }

        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(CapturedSpan {
                name: attrs.metadata().name().to_string(),
                fields,
                declared_fields,
                parent_dim,
            });
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span_ref) = ctx.span(id) {
            let name = span_ref.name().to_string();
            let declared_fields = span_ref
                .metadata()
                .fields()
                .iter()
                .map(|f| f.name().to_string())
                .collect::<Vec<_>>();
            let (fields, parent_dim) = {
                let ext = span_ref.extensions();
                let fields = ext
                    .get::<RecordedFields>()
                    .map(|r| r.fields.clone())
                    .unwrap_or_default();
                let parent_dim = span_ref.scope().skip(1).find_map(|parent| {
                    parent
                        .extensions()
                        .get::<RecordedFields>()
                        .and_then(|r| r.fields.get("dim").cloned())
                });
                (fields, parent_dim)
            };
            self.buffer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(CapturedSpan {
                    name,
                    fields,
                    declared_fields,
                    parent_dim,
                });
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span_ref) = ctx.span(id) {
            // Merge new values into the persistent extension.
            {
                let mut ext = span_ref.extensions_mut();
                if let Some(recorded) = ext.get_mut::<RecordedFields>() {
                    values.record(&mut FieldVisitor(&mut recorded.fields));
                }
            }
            // Push a snapshot with the fully-merged fields so that lazily-set
            // fields (e.g. `iter` recorded at the end of a function body) are
            // visible to post-update assertions.
            let name = span_ref.name().to_string();
            let declared_fields = span_ref
                .metadata()
                .fields()
                .iter()
                .map(|f| f.name().to_string())
                .collect::<Vec<_>>();
            let ext = span_ref.extensions();
            let fields = ext
                .get::<RecordedFields>()
                .map(|r| r.fields.clone())
                .unwrap_or_default();
            let parent_dim = span_ref.scope().skip(1).find_map(|parent| {
                parent
                    .extensions()
                    .get::<RecordedFields>()
                    .and_then(|r| r.fields.get("dim").cloned())
            });
            self.buffer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(CapturedSpan {
                    name,
                    fields,
                    declared_fields,
                    parent_dim,
                });
        }
    }
}

// ── process-global statics ───────────────────────────────────────────────────

static CAPTURE_BUFFER: OnceLock<Arc<Mutex<Vec<CapturedSpan>>>> = OnceLock::new();

/// Separate from `TELEMETRY_TEST_LOCK` in mcrs_minecraft_lighting so capture
/// contention does not extend the critical section of convergence-budget tests.
static CAPTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

static SUBSCRIBER_INSTALLED: OnceLock<()> = OnceLock::new();

// ── public API ───────────────────────────────────────────────────────────────

/// Installs a process-global tracing subscriber (once) and returns the shared
/// capture buffer. Call this before building any `App::new()` so the subscriber
/// is the global default when `TaskPoolPlugin` spawns its worker threads.
pub fn install_global_capture() -> Arc<Mutex<Vec<CapturedSpan>>> {
    let buffer = CAPTURE_BUFFER
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone();

    SUBSCRIBER_INSTALLED.get_or_init(|| {
        let layer = CaptureLayer {
            buffer: buffer.clone(),
        };
        let subscriber = tracing_subscriber::Registry::default().with(layer);
        // .ok() swallows the error returned if the global subscriber was
        // already set by a prior test run in the same process.
        tracing::subscriber::set_global_default(subscriber).ok();
        // Fire a dummy span so the global dispatcher registers itself in the
        // internal DISPATCHERS list before rebuild_interest_cache() runs.
        // Without this priming call, rebuild_interest_cache() may compute
        // MAX_LEVEL = OFF (no registered dispatchers yet), permanently
        // disabling all callsites that evaluated their interest before the
        // subscriber was installed.
        let _ = tracing::info_span!("__warmup__").entered();
        // Force all previously registered callsites to recompute their interest
        // against the new subscriber.
        tracing::callsite::rebuild_interest_cache();
    });

    buffer
}

/// Acquires the capture lock, clears the shared buffer, and returns both the
/// guard (held for the lifetime of the test body) and the buffer handle.
///
/// The lock must remain held across `app.update()` and all assertions to
/// prevent another test from clearing the buffer mid-observation.
pub fn lock_and_clear() -> (MutexGuard<'static, ()>, Arc<Mutex<Vec<CapturedSpan>>>) {
    let lock = CAPTURE_LOCK.get_or_init(|| Mutex::new(()));
    let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let buffer = CAPTURE_BUFFER
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone();
    buffer.lock().unwrap().clear();
    (guard, buffer)
}
