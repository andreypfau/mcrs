//! Asserts that dim-span pairs fire for each spawned dimension and that
//! descendant `lighting::*` spans inherit the `dim` field through the
//! tracing parent chain.
//!
//! Tests in this file compile immediately but FAIL until the dim-span
//! injection is wired in the production SubApp builder — assertions will
//! report "expected dim_extract span for test:overworld but found 0"
//! rather than passing vacuously. That failure mode is intentional.
//!
//! Fixture shape mirrors bus_e2e.rs: `build_app` + `enqueue_*` helpers +
//! `drive_to_playing_and_spawn_subapps` using the production
//! `drain_dim_spawn_queue` entry-point.
//!
//! Parent-chain note: Bevy per-system spans use `parent: None` and do NOT
//! inherit `dim`. Only project-controlled `#[instrument]` spans inherit `dim`
//! through the tracing parent chain. The parent-chain assertion targets
//! `lighting::*` descendants, not `"system"` spans.

#![cfg(feature = "telemetry-tracy")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bridge::partition_main_inbound;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use tracing::subscriber::with_default;
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan, Registry};
use vanilla::block::Block;
use vanilla::enchantment::EnchantmentData;

#[allow(unused_imports)]
use mcrs_vanilla as vanilla;

#[derive(Default)]
struct CapturedSpan {
    name: String,
    fields: HashMap<String, String>,
    parent_dim: Option<String>,
}

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

struct RecordedFields {
    fields: HashMap<String, String>,
}

struct DimCaptureLayer {
    captures: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl<S> tracing_subscriber::Layer<S> for DimCaptureLayer
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

        // Store fields in extensions so descendant spans can read them.
        if let Some(span_ref) = ctx.span(id) {
            span_ref
                .extensions_mut()
                .insert(RecordedFields { fields: fields.clone() });
        }

        // Walk parent chain to find the nearest `dim` field.
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

        self.captures.lock().unwrap().push(CapturedSpan {
            name: attrs.metadata().name().to_string(),
            fields,
            parent_dim,
        });
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span_ref) = ctx.span(id) {
            let mut ext = span_ref.extensions_mut();
            if let Some(recorded) = ext.get_mut::<RecordedFields>() {
                values.record(&mut FieldVisitor(&mut recorded.fields));
            }
        }
    }
}

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn build_app() -> App {
    static SET_ASSET_ROOT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    SET_ASSET_ROOT.get_or_init(|| {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");
        unsafe {
            std::env::set_var("BEVY_ASSET_ROOT", workspace_root);
        }
    });

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<Block>::new());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(TagRegistry::<Block>::default());

    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(Update, partition_main_inbound);

    app
}

fn enqueue_overworld(app: &mut App) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
}

fn enqueue_the_nether(app: &mut App) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:the_nether"),
            type_config: DimensionTypeConfig::default(),
            has_sky: false,
        });
}

fn drive_to_playing_and_spawn_subapps(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    drain_dim_spawn_queue(app);
}

/// Asserts that dim-span pairs (`dim_extract` / `dim_tick`) fire for each
/// spawned dimension, and that descendant `lighting::*` spans carry the
/// `dim` field through the tracing parent chain.
///
/// Uses the production `drain_dim_spawn_queue` entry-point — NOT a custom
/// `set_extract` call — to ensure the bus shuttle closure is preserved
/// inside the extract wrapper.
///
/// The parent-chain assertion targets `lighting::*` spans rather than
/// `"system"` spans because Bevy emits per-system spans with `parent: None`,
/// which breaks the tracing inheritance chain. Project-controlled
/// `#[instrument]` spans (like `lighting::*`) inherit `dim` normally.
#[test]
fn dim_span_pair_fires_and_lighting_inherits_dim_field() {
    let captures: Arc<Mutex<Vec<CapturedSpan>>> = Arc::new(Mutex::new(Vec::new()));
    let layer = DimCaptureLayer {
        captures: captures.clone(),
    };
    let subscriber = Registry::default().with(layer);

    with_default(subscriber, || {
        let mut app = build_app();
        enqueue_overworld(&mut app);
        enqueue_the_nether(&mut app);
        drive_to_playing_and_spawn_subapps(&mut app);
        app.update();
    });

    let captured = captures.lock().unwrap();

    // (a) dim_extract fires for test:overworld
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_extract"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:overworld")),
        "expected dim_extract span with dim = \"test:overworld\" but found 0 emissions"
    );

    // (b) dim_tick fires for test:overworld
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_tick"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:overworld")),
        "expected dim_tick span with dim = \"test:overworld\" but found 0 emissions"
    );

    // (c) dim_extract fires for test:the_nether
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_extract"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:the_nether")),
        "expected dim_extract span with dim = \"test:the_nether\" but found 0 emissions"
    );

    // (d) dim_tick fires for test:the_nether
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_tick"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:the_nether")),
        "expected dim_tick span with dim = \"test:the_nether\" but found 0 emissions"
    );

    // (e) At least one lighting::* span captured during an overworld pump
    //     has parent_dim == "test:overworld" (parent-chain inheritance).
    assert!(
        captured
            .iter()
            .any(|s| s.name.starts_with("lighting::")
                && s.parent_dim.as_deref() == Some("test:overworld")),
        "expected at least one lighting::* span with parent dim = \"test:overworld\" \
         but found 0; note: only #[instrument] spans inherit dim, not Bevy system spans"
    );

    // (f) At least one lighting::* span with parent_dim == "test:the_nether".
    assert!(
        captured
            .iter()
            .any(|s| s.name.starts_with("lighting::")
                && s.parent_dim.as_deref() == Some("test:the_nether")),
        "expected at least one lighting::* span with parent dim = \"test:the_nether\" \
         but found 0; note: only #[instrument] spans inherit dim, not Bevy system spans"
    );
}
