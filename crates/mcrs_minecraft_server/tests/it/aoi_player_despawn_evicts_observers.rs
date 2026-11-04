//! Regression for the per-dim player-removal eviction path. Exercises
//! the production two-world topology: the main App emits an
//! InboundPlayerDespawn via PendingInboundLifecycle; the extract closure
//! shuttles it into the per-dim sub-app's Messages buffer; the per-dim
//! drain system resolves the in-dim Player via Owner and retains
//! it out of every column's PlayerObservers; its despawn takes it off the
//! client of each viewer.

use bevy_app::{App, AppLabel, FixedPostUpdate, FixedPreUpdate, SubApp};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_engine_muxer::{Muxer, View, ViewerId};
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::column_view::ColumnView;
use mcrs_minecraft_engine::player_tiles::VIEW_RADIUS;
use mcrs_minecraft_engine::player_tracker::PlayerTrackerPlugin;
use mcrs_minecraft_engine::replicated::Replicated;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_world::aoi::PlayerObservers;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionBundle, DimensionTypeConfig, InDimension,
};
use mcrs_minecraft_engine_world::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundAddEntity, ClientboundRemoveEntities};
use mcrs_minecraft_replication::PlayerCodec;
use mcrs_voxel::ColumnPos;

/// Ad-hoc sub-app label for this test.
#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel;

/// Ad-hoc schedule label that chains FixedPreUpdate → FixedPostUpdate,
/// mirroring the DimTick driver used in production sub-apps.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestDimTick;

/// Regression for the disconnect removal path proving the three eviction truths:
///   1. T's view is taken and no column lists T after the drain.
///   2. After T despawns, the muxer hands O a remove packet carrying T's wire id.
///   3. A later tick hands O nothing of T.
///
/// Reuses the same two-world harness (main App + SubApp, `set_extract` test
/// closure) as the existing topology test above. Observer O does NOT move
/// between warm-up and removal — the fix must work for stationary observers.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the three assertions follow one observer through warm-up, disconnect and eviction in a single world"
)]
fn disconnect_path_evicts_stationary_observer_three_assertions() {
    // --- Main App setup ---
    let mut main_app = App::new();
    main_app.add_message::<InboundPlayerDespawn>();
    main_app.add_message::<OutboundPlayerPacket>();

    let session_o = PlayerSession(1);
    let session_t = PlayerSession(2);

    // --- Per-dim SubApp setup (mirrors the existing test above exactly) ---
    let mut sub_app = SubApp::new();
    sub_app.add_message::<InboundPlayerDespawn>();
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_schedule(Schedule::new(FixedPreUpdate));
    sub_app.add_schedule(Schedule::new(FixedPostUpdate));
    sub_app.add_schedule(Schedule::new(TestDimTick));
    sub_app.update_schedule = Some(TestDimTick.intern());
    sub_app.add_systems(TestDimTick, |world: &mut World| {
        world.run_schedule(FixedPreUpdate);
        world.run_schedule(FixedPostUpdate);
    });
    sub_app.add_plugins(PlayerTrackerPlugin);

    let dim = sub_app
        .world_mut()
        .spawn(DimensionBundle::new(
            mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into(),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id();

    // Both players at the same position, inside the track radius.
    let pos = DVec3::new(0.0, 64.0, 0.0);

    // Observer O: remains stationary throughout.
    let player_o = sub_app
        .world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ColumnView::holding(crate::harness::columns_in_view(pos)),
            InDimension(dim),
            Owner(session_o),
        ))
        .id();

    // Target T: the player to be removed via disconnect.
    let player_t = sub_app
        .world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ColumnView::holding(crate::harness::columns_in_view(pos)),
            InDimension(dim),
            Owner(session_t),
        ))
        .id();

    // Seed column grid with PlayerObservers pre-attached.
    let radius: i32 = 20;
    let columns: Vec<Entity> = {
        let mut entities = Vec::new();
        let mut col_map: rustc_hash::FxHashMap<ColumnPos, ColumnSlot> =
            rustc_hash::FxHashMap::default();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let col_pos = ColumnPos::new(dx, dz);
                let column = sub_app
                    .world_mut()
                    .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                    .id();
                entities.push(column);
                col_map.insert(
                    col_pos,
                    ColumnSlot {
                        entity: column,
                        section_count: 1,
                    },
                );
            }
        }
        sub_app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("DimensionBundle provides ColumnIndex")
            .0
            .extend(col_map);
        entities
    };

    // Extract closure: drains InboundPlayerDespawn from the main world Messages
    // buffer and shuttles them into the sub-app's Messages buffer.
    sub_app.set_extract(|main_world, sub_world| {
        let despawns: Vec<InboundPlayerDespawn> = main_world
            .resource_mut::<Messages<InboundPlayerDespawn>>()
            .drain()
            .collect();
        if !despawns.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<InboundPlayerDespawn>>();
            for msg in despawns {
                sub_msgs.write(msg);
            }
        }
    });

    main_app.insert_sub_app(TestDimLabel, sub_app);

    // --- Tick 1: warm-up: the mirror lists both players on the columns they hold, and the
    // dimension publishes their spawns; O subscribes to the tiles around its own, as the host
    // does when it sees the spawn.
    main_app.update();
    let mut muxer = Muxer::new(PlayerCodec::default());
    publish(&mut main_app, &mut muxer);
    let own = replicated(&main_app, player_o);
    let viewer_o = ViewerId(session_o.0);
    muxer.subscribe(
        viewer_o,
        View {
            center: own.tile,
            radius: VIEW_RADIUS,
            subject: Some(own.id),
        },
    );
    let wire_id_t = mcrs_minecraft_engine::network_id::player_network_id(session_t);
    assert!(
        packets_for(&mut muxer, viewer_o)
            .into_iter()
            .filter(|(id, _)| *id == ClientboundAddEntity::ID)
            .any(
                |(_, body)| ClientboundAddEntity::decode(&mut body.as_slice())
                    .unwrap()
                    .id
                    .0
                    == wire_id_t
            ),
        "precondition: O holds T before removal"
    );

    // Non-vacuous precondition: T must be listed on the columns it holds.
    assert!(
        count_columns_observing_in_sub(&main_app, &columns, player_t) > 0,
        "precondition: T must be listed on the columns it holds before removal"
    );

    // --- Simulate disconnect: push InboundPlayerDespawn for T ---
    main_app
        .world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn { session: session_t });

    // --- Tick 2: the drain takes T's view; T has not despawned yet ---
    main_app.update();
    publish(&mut main_app, &mut muxer);
    {
        let sub = main_app.get_sub_app(TestDimLabel).expect("sub-app");
        assert!(
            sub.world().get::<ColumnView>(player_t).is_none(),
            "T still has a view after eviction"
        );
    }
    let count_after = count_columns_observing_in_sub(&main_app, &columns, player_t);
    assert_eq!(
        count_after, 0,
        "expected zero columns observing T after eviction, got {count_after}"
    );
    packets_for(&mut muxer, viewer_o);

    // --- Tick 3: T despawns, as the disconnect does after the drain (O does NOT move) ---
    main_app
        .get_sub_app_mut(TestDimLabel)
        .expect("sub-app")
        .world_mut()
        .despawn(player_t);
    main_app.update();
    publish(&mut main_app, &mut muxer);

    let removed_for_o = packets_for(&mut muxer, viewer_o)
        .into_iter()
        .filter(|(id, _)| *id == ClientboundRemoveEntities::ID)
        .any(|(_, body)| {
            ClientboundRemoveEntities::decode(&mut body.as_slice())
                .unwrap()
                .entity_ids
                .iter()
                .any(|id| id.0 == wire_id_t)
        });
    assert!(
        removed_for_o,
        "expected a remove packet for O with T's wire id"
    );
    main_app.update();
    publish(&mut main_app, &mut muxer);
    assert!(
        packets_for(&mut muxer, viewer_o).is_empty(),
        "O gets nothing of T once T despawned"
    );
}

/// Hands the tile deltas of the last tick of the dimension to `muxer`.
fn publish(app: &mut App, muxer: &mut Muxer<PlayerCodec>) {
    let deltas = app
        .get_sub_app_mut(TestDimLabel)
        .expect("sub-app")
        .world_mut()
        .resource_mut::<TileEvents>()
        .take();
    muxer.publish(&deltas);
}

fn replicated(app: &App, player: Entity) -> Replicated {
    *app.get_sub_app(TestDimLabel)
        .expect("sub-app")
        .world()
        .get::<Replicated>(player)
        .expect("the dimension publishes the player")
}

/// The packets that `muxer` hands `viewer`, each as its id and its body.
fn packets_for(muxer: &mut Muxer<PlayerCodec>, viewer: ViewerId) -> Vec<(i32, Vec<u8>)> {
    let chunks: Vec<Bytes> = muxer
        .take_output()
        .filter(|output| output.viewer == viewer)
        .flat_map(|output| output.chunks)
        .collect();
    crate::host_app::replicated(&[PacketPayload::Replication(chunks)])
}

fn count_columns_observing_in_sub(app: &App, columns: &[Entity], target: Entity) -> usize {
    let sub = app.get_sub_app(TestDimLabel).expect("sub-app not found");
    let world = sub.world();
    let mut count = 0usize;
    for &column in columns {
        if let Some(obs) = world.get::<PlayerObservers>(column)
            && obs.0.contains(&target)
        {
            count += 1;
        }
    }
    count
}
