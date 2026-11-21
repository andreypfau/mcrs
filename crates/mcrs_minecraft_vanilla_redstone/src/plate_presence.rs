use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res, ResMut};
use glam::DVec3;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::block_simulation::{BlockMemory, BlockRules, BlockSimulation};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Bodies;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_redstone::{
    HasRedstone, HasRedstoneMemory, Kind, Presence, RedstoneTable, plate_signal_for,
};
use mcrs_minecraft_simulation::{BlockBehaviour, UpdateRequest};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};
use rustc_hash::FxHashMap;

const ITEM_WIDTH: f32 = 0.25;
const ITEM_HEIGHT: f32 = 0.25;
const SENSING_INSET: f64 = 1.0 / 16.0;
const SENSING_HEIGHT: f64 = 0.25;
const SECTION_EDGE: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Occupant {
    Living,
    Other,
}

fn sensing_box(pos: BlockPos) -> Aabb {
    let origin = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
    Aabb::new(
        origin + DVec3::new(SENSING_INSET, 0.0, SENSING_INSET),
        origin + DVec3::new(1.0 - SENSING_INSET, SENSING_HEIGHT, 1.0 - SENSING_INSET),
    )
}

fn floor(value: f64) -> i32 {
    i32::try_from(value.floor() as i64).unwrap_or(0)
}

fn candidates(body: &Aabb) -> impl Iterator<Item = BlockPos> {
    let xs = floor(body.min.x)..=floor(body.max.x);
    let zs = floor(body.min.z)..=floor(body.max.z);
    let ys = floor(body.min.y - SENSING_HEIGHT)..=floor(body.max.y);
    xs.flat_map(move |x| {
        let zs = zs.clone();
        let ys = ys.clone();
        zs.flat_map(move |z| ys.clone().map(move |y| BlockPos::new(x, y, z)))
    })
}

fn is_plate(table: &RedstoneTable, state: mcrs_voxel::BlockStateId) -> bool {
    matches!(table.kind(state), Some(Kind::Plate(_)))
}

type PlayerItem<'a> = (&'a Transform, &'a PlayerGameMode, PlayerBody);

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, resource and local is its own parameter"
)]
pub(crate) fn publish_plate_presence<B>(
    bodies: Res<Bodies>,
    items: Query<&MotionState, With<ItemEntity>>,
    players: Query<PlayerItem, With<Player>>,
    dims: Query<Entity, With<Dimension>>,
    world: WorldView,
    rules: Res<BlockRules<B>>,
    mut memory: ResMut<BlockMemory<B>>,
    mut simulation: ResMut<BlockSimulation>,
    mut found: Local<FxHashMap<BlockPos, Presence>>,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let Some(view) = dims
        .single()
        .ok()
        .and_then(|dimension| world.get(dimension))
    else {
        return;
    };
    let table = rules.0.redstone().table();
    found.clear();
    let known = &memory.0.redstone().plate_positions;
    if known.is_empty() {
        memory.0.redstone().plates.clear();
        return;
    }
    let mut visit = |body: Aabb, occupant: Occupant| {
        for pos in candidates(&body) {
            if !known.contains(&pos) || !sensing_box(pos).intersects(&body) {
                continue;
            }
            let presence = found.entry(pos).or_default();
            presence.entities = presence.entities.saturating_add(1);
            if occupant == Occupant::Living {
                presence.living = presence.living.saturating_add(1);
            }
        }
    };
    for (_, tile) in bodies.0.tiles() {
        for row in &tile.rows {
            visit(row.bounds(), Occupant::Living);
        }
    }
    for (transform, mode, body) in &players {
        if mode.0 == GameMode::Spectator {
            continue;
        }
        let dimensions = body.dimensions();
        visit(
            Aabb::of_body(transform.translation, dimensions.width, dimensions.height),
            Occupant::Living,
        );
    }
    for state in &items {
        visit(
            Aabb::of_body(state.0.body.position, ITEM_WIDTH, ITEM_HEIGHT),
            Occupant::Other,
        );
    }
    let plates = &mut memory.0.redstone().plates;
    plates.clear();
    for (&pos, &presence) in found.iter() {
        plates.insert(pos, presence);
        let Ok(state) = view.block(pos) else {
            continue;
        };
        let Some(Kind::Plate(kind)) = table.kind(state) else {
            continue;
        };
        let pressed = plate_signal_for(kind, presence) > 0;
        let resting = table.powered(state) || table.power(state) > 0;
        if pressed && !resting {
            simulation.request_update(UpdateRequest::Single {
                target: pos,
                block: table.block_of(state),
            });
        }
    }
}

pub(crate) fn index_loaded_plates<B>(
    mut stages: MessageReader<SectionStageChanged>,
    sections: Query<&SectionIndex>,
    rules: Res<BlockRules<B>>,
    mut memory: ResMut<BlockMemory<B>>,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let table = rules.0.redstone().table();
    for change in stages.read() {
        if change.left() {
            let redstone = memory.0.redstone();
            let section = change.pos;
            redstone
                .plate_positions
                .retain(|pos| SectionPos::from(*pos) != section);
            redstone
                .plates
                .retain(|pos, _| SectionPos::from(*pos) != section);
            continue;
        }
        if !change.landed() {
            continue;
        }
        let Some(blocks) = sections
            .get(change.dim)
            .ok()
            .and_then(|index| index.blocks(change.pos))
        else {
            continue;
        };
        let mut any = false;
        blocks.for_each_distinct(|state| any |= is_plate(table, state));
        if !any {
            continue;
        }
        let origin = change.pos.min_block();
        let positions = &mut memory.0.redstone().plate_positions;
        for y in 0..SECTION_EDGE {
            for z in 0..SECTION_EDGE {
                for x in 0..SECTION_EDGE {
                    if is_plate(table, blocks.get(LocalPos::new(x, y, z))) {
                        positions.insert(BlockPos::new(
                            origin.x + i32::from(x),
                            origin.y + i32::from(y),
                            origin.z + i32::from(z),
                        ));
                    }
                }
            }
        }
    }
}
