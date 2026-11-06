//! A column that goes cold keeps its last version in the data layer, and a column that a ticket
//! loads again comes back from there instead of from the generator. A view gets every column
//! past the hot area from the data layer, which generates and lights it outside the dimension.

use std::time::{Duration, Instant};

use bevy_app::{App, Last};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::{ResMut, Resource};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine::column_version::ColumnVersion;
use mcrs_minecraft_engine::muxer::column_views::ColumnViews;
use mcrs_minecraft_engine::muxer::muxer_link::MuxerLink;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::LOADING_RINGS;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::SimulationDistance;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::chunk::LightData;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelChunkWithLight;
use mcrs_minecraft_protocol_game::light_codec::{ColumnLight, RowLight, unpack_light_data};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::host_app::{self, label_of, world as dimension_world};
use crate::world_server::{
    OVERWORLD, WorldServer, block_state, dimension_of, spawn_viewer, tick_until,
    wait_for_generation,
};

const PATIENCE: Duration = Duration::from_secs(120);

/// A server whose overworld can generate, and its label.
fn server() -> (App, Entity) {
    let mut app = WorldServer {
        worlds: mcrs_minecraft_engine_server::Placement::Thread,
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    (app, label)
}

/// Ticks until the muxer has sent the client of `session` what `sent` asks for. Nothing answers
/// the batches, so the test takes each one as the client would. `check` sees the dimension
/// after each tick.
fn tick_until_sent(
    app: &mut App,
    label: Entity,
    session: PlayerSession,
    what: &str,
    sent: impl Fn(&ColumnViews) -> bool,
    check: impl Fn(&mut World),
) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        check(dimension_world(app, label));
        let mut link = app.world_mut().resource_mut::<MuxerLink>();
        link.acknowledge_batch(session, 64.0);
        let stage = link.inline_stage().expect("the views run inline");
        if sent(stage.views()) {
            return;
        }
        assert!(Instant::now() < deadline, "{what} never happened");
        host_app::tick(app);
    }
}

/// Whether the client of `session` holds every column of the view that the muxer serves it.
fn holds_whole_view(views: &ColumnViews, session: PlayerSession) -> bool {
    let Some(view) = views.view(session) else {
        return false;
    };
    let mut whole = true;
    view.each_column(|column| whole &= views.holds(session, column));
    whole
}

fn block_at(world: &mut World, dimension: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let section = world
        .get::<SectionIndex>(dimension)?
        .get(SectionPos::from(pos))?;
    if world.get::<SectionStage>(section) != Some(&SectionStage::Loaded) {
        return None;
    }
    let blocks = world.get::<ChunkBlocks>(section)?;
    blocks.0.get_cell(
        pos.x.rem_euclid(16) as usize,
        pos.y.rem_euclid(16) as usize,
        pos.z.rem_euclid(16) as usize,
    )
}

fn holds_column(world: &mut World, dimension: Entity, column: ColumnPos) -> bool {
    world
        .get::<ColumnIndex>(dimension)
        .is_some_and(|index| index.contains_key(&column))
}

#[test]
fn a_change_survives_the_column_going_cold_and_coming_back() {
    let (mut app, label) = server();
    let glowstone = block_state(&app, "minecraft:glowstone");
    let world = dimension_world(&mut app, label);
    let dimension = dimension_of(world);
    let session = PlayerSession(1);
    let player = spawn_viewer(world, session, DVec3::new(8.5, 100.0, 8.5), 2);
    let pos = BlockPos::new(8, 120, 8);
    let column = ColumnPos::new(0, 0);

    tick_until_sent(
        &mut app,
        label,
        session,
        "the column was sent",
        |views| views.holds(session, column),
        |_| {},
    );
    let world = dimension_world(&mut app, label);
    assert_ne!(block_at(world, dimension, pos), Some(glowstone));
    world
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos,
            new_state: glowstone,
            flags: BlockUpdateFlags::all(),
            recursion_left: 512,
        });
    tick_until(&mut app, label, "the block was placed", |world| {
        block_at(world, dimension, pos) == Some(glowstone)
    });

    let moved = |app: &mut App, x: f64| {
        let world = dimension_world(app, label);
        world
            .get_mut::<Transform>(player)
            .expect("the player stands somewhere")
            .translation
            .x = x;
    };
    moved(&mut app, 16.0 * 40.0);
    tick_until(&mut app, label, "the column unloaded", |world| {
        !holds_column(world, dimension, column)
    });
    moved(&mut app, 8.5);
    tick_until(&mut app, label, "the column loaded again", |world| {
        block_at(world, dimension, pos).is_some()
    });
    assert_eq!(
        block_at(dimension_world(&mut app, label), dimension, pos),
        Some(glowstone),
        "the column came back from the generator, not from the data layer"
    );
}

#[test]
fn a_view_wider_than_the_hot_area_is_sent_whole_and_loads_nothing_past_it() {
    let (mut app, label) = server();
    let world = dimension_world(&mut app, label);
    world.insert_resource(SimulationDistance {
        horizontal: 1,
        vertical: u8::MAX,
    });
    let dimension = dimension_of(world);
    let session = PlayerSession(1);
    let player = spawn_viewer(world, session, DVec3::new(8.5, 100.0, 8.5), 5);
    // The simulation area one ring wider, and the rings that load with each of its columns.
    let loaded_reach = 2 + LOADING_RINGS;
    let loaded_past_the_hot_area = |world: &mut World| {
        world.get::<ColumnIndex>(dimension).is_some_and(|index| {
            index
                .keys()
                .any(|column| column.x.abs().max(column.z.abs()) > loaded_reach)
        })
    };

    tick_until_sent(
        &mut app,
        label,
        session,
        "the whole view was sent",
        |views| holds_whole_view(views, session),
        |world| {
            assert!(
                !loaded_past_the_hot_area(world),
                "a column past the hot area was loaded into the dimension"
            );
        },
    );
    let view = dimension_world(&mut app, label)
        .get::<SentChunkView>(player)
        .and_then(|sent| sent.0)
        .expect("the view was applied");
    assert!(
        i32::from(view.distance) > loaded_reach,
        "the view reaches past the loaded columns"
    );

    let world = dimension_world(&mut app, label);
    let late = PlayerSession(2);
    spawn_viewer(world, late, DVec3::new(8.5, 100.0, 8.5), 5);
    tick_until_sent(
        &mut app,
        label,
        late,
        "the late view was sent",
        |views| holds_whole_view(views, late),
        |world| {
            let generating = world
                .query::<&SectionStage>()
                .iter(world)
                .any(|stage| *stage == SectionStage::Generating);
            assert!(
                !generating,
                "a column the data layer holds was generated again"
            );
            assert!(
                !loaded_past_the_hot_area(world),
                "a cold column was loaded instead of sent from the data layer"
            );
        },
    );
}

/// What the dimension handed the muxer for the columns it subscribes to, in the order the
/// muxer applies it.
#[derive(Resource, Default)]
struct Published(Vec<PublishedColumn>);

enum PublishedColumn {
    Version(ColumnVersion),
    Light(ColumnPos, LightData<'static>),
    Block(ColumnPos),
}

fn capture_published(
    mut versions: MessageReader<ColumnVersion>,
    mut changes: MessageReader<ColumnChange>,
    mut published: ResMut<Published>,
) {
    for version in versions.read() {
        published.0.push(PublishedColumn::Version(version.clone()));
    }
    for change in changes.read() {
        match &change.payload {
            PacketPayload::LightUpdate(update) => published.0.push(PublishedColumn::Light(
                change.column,
                update.light_data.clone(),
            )),
            PacketPayload::BlockUpdate(_) | PacketPayload::SectionBlocksUpdate(_) => {
                published.0.push(PublishedColumn::Block(change.column));
            }
            _ => {}
        }
    }
}

fn light_rows(light: &ColumnLight) -> impl Iterator<Item = &RowLight> {
    light.sky.iter().chain(&light.block)
}

/// Plays what the dimension published onto the light rows of every column it sent, as the
/// clients hold them, and counts the light updates that changed no row.
fn light_updates_changing_nothing(rows: usize, published: &[PublishedColumn]) -> usize {
    let mut held: FxHashMap<ColumnPos, Vec<[u8; 2048]>> = FxHashMap::default();
    let mut changing_nothing = 0;
    for entry in published {
        match entry {
            PublishedColumn::Version(version) => {
                let mut packet = &version.packet[..];
                VarInt::decode(&mut packet).expect("the version starts with its packet id");
                let chunk = ClientboundLevelChunkWithLight::decode(&mut packet)
                    .expect("the version is a chunk packet");
                let light = unpack_light_data(&chunk.light_data, rows).expect("the light unpacks");
                let rows = light_rows(&light)
                    .map(|row| match row {
                        RowLight::Filled(chunk) => chunk.0,
                        RowLight::Unchanged | RowLight::Empty => [0; 2048],
                    })
                    .collect();
                held.insert(version.column, rows);
            }
            PublishedColumn::Light(column, data) => {
                let update = unpack_light_data(data, rows).expect("the update unpacks");
                let Some(held) = held.get_mut(column) else {
                    continue;
                };
                let mut changed = false;
                for (row, next) in held.iter_mut().zip(light_rows(&update)) {
                    let next = match next {
                        RowLight::Unchanged => continue,
                        RowLight::Empty => [0; 2048],
                        RowLight::Filled(chunk) => chunk.0,
                    };
                    changed |= *row != next;
                    *row = next;
                }
                if !changed {
                    changing_nothing += 1;
                }
            }
            PublishedColumn::Block(_) => {}
        }
    }
    changing_nothing
}

/// The light updates of a column that the clients hold while no block within one column of it
/// changed. The light of such a column is the light it went out with, so each of them is light
/// the clients draw only until the next one takes it back.
fn light_updates_without_a_block_change(published: &[PublishedColumn]) -> Vec<ColumnPos> {
    let changed: Vec<ColumnPos> = published
        .iter()
        .filter_map(|entry| match entry {
            PublishedColumn::Block(column) => Some(*column),
            _ => None,
        })
        .collect();
    let mut held = Vec::new();
    let mut unprompted = Vec::new();
    for entry in published {
        match entry {
            PublishedColumn::Version(version) => held.push(version.column),
            PublishedColumn::Light(column, _)
                if held.contains(column)
                    && !column.square(1).any(|near| changed.contains(&near)) =>
            {
                unprompted.push(*column);
            }
            PublishedColumn::Light(..) | PublishedColumn::Block(_) => {}
        }
    }
    unprompted
}

#[test]
fn a_column_that_turns_hot_sends_no_light_its_viewers_already_hold() {
    let (mut app, label) = server();
    let sub_app = host_app::sub_app(&mut app, label);
    sub_app.init_resource::<Published>();
    sub_app.add_systems(Last, capture_published);
    let world = dimension_world(&mut app, label);
    world.insert_resource(SimulationDistance {
        horizontal: 1,
        vertical: u8::MAX,
    });
    let rows = world
        .query::<&DimensionTypeConfig>()
        .single(world)
        .expect("the dimension's shape")
        .section_count as usize
        + 2;
    let session = PlayerSession(1);
    let player = spawn_viewer(world, session, DVec3::new(8.5, 100.0, 8.5), 5);
    tick_until_sent(
        &mut app,
        label,
        session,
        "the whole view was sent",
        |views| holds_whole_view(views, session),
        |_| {},
    );

    // Each step moves a ring of columns that the view holds with the light of the data layer
    // into the area that the dimension lights.
    for step in 1..=4 {
        dimension_world(&mut app, label)
            .get_mut::<Transform>(player)
            .expect("the player stands somewhere")
            .translation
            .x = 8.5 + 16.0 * f64::from(step);
        tick_until_sent(
            &mut app,
            label,
            session,
            "the moved view was sent",
            |views| holds_whole_view(views, session),
            |_| {},
        );
        for _ in 0..40 {
            host_app::tick(&mut app);
        }
    }

    let published = &dimension_world(&mut app, label).resource::<Published>().0;
    let lit = published
        .iter()
        .filter(|entry| matches!(entry, PublishedColumn::Light(..)))
        .count();
    assert_eq!(
        light_updates_changing_nothing(rows, published),
        0,
        "of {lit} light updates, some repeated the light that the viewers held"
    );
    assert_eq!(
        light_updates_without_a_block_change(published),
        [],
        "a column goes out with its final light, so only a changed block changes it"
    );
}
