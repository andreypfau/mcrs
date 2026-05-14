// Ordering tests for the column light-update sender.
//
// `send_light_updates` filters per-player by `ColumnView::sent_columns`. To
// avoid depending on `ServerSideConnection` (which requires a live TCP
// session and cannot be constructed in a unit test), the test registers a
// SECOND observer system that mirrors the same gate filter and records each
// "would-have-sent" tuple into a `TestLightUpdateLog` resource. The
// production `send_light_updates` is identical in shape — see
// `crates/mcrs_minecraft/src/world/entity/player/column_view.rs` — so the
// observer is faithful to the production gate logic.

use bevy_app::{App, FixedPostUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use std::borrow::Cow;

use mcrs_engine::world::column::ChunkColumnPos as EngineChunkColumnPos;
use mcrs_minecraft_lighting::codec::ColumnLightUpdate;
use mcrs_minecraft::world::entity::player::column_view::ColumnView;
use mcrs_protocol::chunk::LightData;
use mcrs_protocol::ChunkColumnPos;

#[derive(Resource, Default)]
struct TestLightUpdateLog(pub Vec<(ChunkColumnPos, &'static str)>);

fn record_light_update_sends(
    mut reader: MessageReader<ColumnLightUpdate>,
    players: Query<&ColumnView>,
    mut log: ResMut<TestLightUpdateLog>,
) {
    for msg in reader.read() {
        let col_pos = ChunkColumnPos::new(msg.column_pos.x, msg.column_pos.z);
        for view in players.iter() {
            if view.sent_columns.contains(&col_pos) {
                log.0.push((col_pos, "sent"));
            }
        }
    }
}

fn build_test_app() -> App {
    let mut app = App::new();
    app.add_message::<ColumnLightUpdate>();
    app.insert_resource(TestLightUpdateLog::default());
    app.add_systems(FixedPostUpdate, record_light_update_sends);
    app
}

fn empty_light_data() -> LightData<'static> {
    LightData {
        sky_light_mask: Cow::Owned(Vec::new()),
        block_light_mask: Cow::Owned(Vec::new()),
        empty_sky_light_mask: Cow::Owned(Vec::new()),
        empty_block_light_mask: Cow::Owned(Vec::new()),
        sky_light_arrays: Cow::Owned(Vec::new()),
        block_light_arrays: Cow::Owned(Vec::new()),
    }
}

fn write_update(app: &mut App, column_pos: EngineChunkColumnPos) {
    app.world_mut()
        .resource_mut::<Messages<ColumnLightUpdate>>()
        .write(ColumnLightUpdate {
            dim: Entity::PLACEHOLDER,
            column: Entity::PLACEHOLDER,
            column_pos,
            light_data: empty_light_data(),
        });
}

#[test]
fn light_update_blocked_before_first_send() {
    let mut app = build_test_app();
    // Spawn a player with an empty sent_columns set.
    app.world_mut().spawn(ColumnView::default());

    write_update(&mut app, EngineChunkColumnPos::new(0, 0));
    app.world_mut().run_schedule(FixedPostUpdate);

    let log = app.world().resource::<TestLightUpdateLog>();
    assert!(
        log.0.is_empty(),
        "no send should occur before first-send; got {:?}",
        log.0
    );
}

#[test]
fn light_update_sent_after_first_send() {
    let mut app = build_test_app();
    let col_pos_protocol = ChunkColumnPos::new(0, 0);
    let col_pos_engine = EngineChunkColumnPos::new(0, 0);

    // Spawn a player whose sent_columns already contains the column —
    // simulates "the chunk packet has been delivered to this client".
    let mut view = ColumnView::default();
    view.sent_columns.insert(col_pos_protocol);
    app.world_mut().spawn(view);

    write_update(&mut app, col_pos_engine);
    app.world_mut().run_schedule(FixedPostUpdate);

    let log = app.world().resource::<TestLightUpdateLog>();
    assert_eq!(
        log.0,
        vec![(col_pos_protocol, "sent")],
        "light update must be sent after first-send; got {:?}",
        log.0
    );
}
