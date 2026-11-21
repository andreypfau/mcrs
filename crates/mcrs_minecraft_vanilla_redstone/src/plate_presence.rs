use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res, ResMut};
use glam::DVec3;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::block_simulation::{
    BlockMemory, BlockRules, BlockShared, BlockSimulation,
};
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
    HasRedstone, HasRedstoneMemory, HasRedstoneSnapshot, Kind, Presence, RedstoneTable, Sensor,
    plate_signal_for,
};
use mcrs_minecraft_simulation::{BlockBehaviour, UpdateRequest};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};
use rustc_hash::FxHashMap;

const ITEM_WIDTH: f32 = 0.25;
const ITEM_HEIGHT: f32 = 0.25;
const SENSING_INSET: f64 = 1.0 / 16.0;
const SENSING_HEIGHT: f64 = 0.25;
const TRIPWIRE_HEIGHT: f64 = 0.5;
const SECTION_EDGE: u8 = 16;
const BODY_REACH: f64 = 2.0;
const COLUMN_REACH: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Occupant {
    Living,
    Other,
}

fn sensing_box(pos: BlockPos, sensor: Sensor) -> Option<Aabb> {
    let origin = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
    match sensor {
        Sensor::Plate => Some(Aabb::new(
            origin + DVec3::new(SENSING_INSET, 0.0, SENSING_INSET),
            origin + DVec3::new(1.0 - SENSING_INSET, SENSING_HEIGHT, 1.0 - SENSING_INSET),
        )),
        Sensor::Tripwire => Some(Aabb::new(
            origin,
            origin + DVec3::new(1.0, TRIPWIRE_HEIGHT, 1.0),
        )),
        Sensor::Daylight => None,
    }
}

fn floor(value: f64) -> i32 {
    i32::try_from(value.floor() as i64).unwrap_or(0)
}

fn column_of(position: DVec3) -> (i32, i32) {
    (floor(position.x), floor(position.z))
}

fn sensor_of(table: &RedstoneTable, state: BlockStateId) -> Option<Sensor> {
    match table.kind(state)? {
        Kind::Plate(_) => Some(Sensor::Plate),
        Kind::Tripwire => Some(Sensor::Tripwire),
        Kind::DaylightDetector => Some(Sensor::Daylight),
        _ => None,
    }
}

fn count(presence: &mut Presence, occupant: Occupant) {
    presence.entities = presence.entities.saturating_add(1);
    if occupant == Occupant::Living {
        presence.living = presence.living.saturating_add(1);
    }
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
    memory: Res<BlockMemory<B>>,
    mut shared: ResMut<BlockShared<B>>,
    mut simulation: ResMut<BlockSimulation>,
    mut nearby: Local<FxHashMap<(i32, i32), Vec<(Aabb, Occupant)>>>,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some(view) = dims
        .single()
        .ok()
        .and_then(|dimension| world.get(dimension))
    else {
        return;
    };
    let table = rules.0.redstone().table();
    let known = &memory.0.redstone_ref().sensors;
    let plates = &mut shared.0.redstone_mut().plates;
    plates.clear();
    if known.is_empty() {
        return;
    }
    for list in nearby.values_mut() {
        list.clear();
    }
    for (transform, mode, body) in &players {
        if mode.0 == GameMode::Spectator {
            continue;
        }
        let dimensions = body.dimensions();
        let body = Aabb::of_body(transform.translation, dimensions.width, dimensions.height);
        nearby
            .entry(column_of(transform.translation))
            .or_default()
            .push((body, Occupant::Living));
    }
    for state in &items {
        let position = state.0.body.position;
        let body = Aabb::of_body(position, ITEM_WIDTH, ITEM_HEIGHT);
        nearby
            .entry(column_of(position))
            .or_default()
            .push((body, Occupant::Other));
    }
    for (&pos, &sensor) in known {
        let Some(sensing) = sensing_box(pos, sensor) else {
            continue;
        };
        let mut presence = Presence::default();
        let reach = DVec3::new(BODY_REACH, BODY_REACH, BODY_REACH);
        for row in bodies.0.in_box(sensing.min - reach, sensing.max + reach) {
            if row.bounds().intersects(&sensing) {
                count(&mut presence, Occupant::Living);
            }
        }
        for dx in -COLUMN_REACH..=COLUMN_REACH {
            for dz in -COLUMN_REACH..=COLUMN_REACH {
                let Some(list) = nearby.get(&(pos.x.saturating_add(dx), pos.z.saturating_add(dz)))
                else {
                    continue;
                };
                for (body, occupant) in list {
                    if body.intersects(&sensing) {
                        count(&mut presence, *occupant);
                    }
                }
            }
        }
        if presence.entities == 0 {
            continue;
        }
        plates.insert(pos, presence);
        let Ok(state) = view.block(pos) else {
            continue;
        };
        let pressed = match table.kind(state) {
            Some(Kind::Plate(kind)) => plate_signal_for(kind, presence) > 0,
            Some(Kind::Tripwire) => !table.disarmed(state),
            _ => false,
        };
        let resting = table.powered(state) || table.power(state) > 0;
        if pressed && !resting {
            simulation.request_update(UpdateRequest::Single {
                target: pos,
                block: table.block_of(state),
            });
        }
    }
}

pub(crate) fn index_loaded_sensors<B>(
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
            let section = change.pos;
            memory
                .0
                .redstone()
                .sensors
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
        blocks.for_each_distinct(|state| any |= sensor_of(table, state).is_some());
        if !any {
            continue;
        }
        let origin = change.pos.min_block();
        let sensors = &mut memory.0.redstone().sensors;
        for y in 0..SECTION_EDGE {
            for z in 0..SECTION_EDGE {
                for x in 0..SECTION_EDGE {
                    if let Some(sensor) = sensor_of(table, blocks.get(LocalPos::new(x, y, z))) {
                        sensors.insert(
                            BlockPos::new(
                                origin.x + i32::from(x),
                                origin.y + i32::from(y),
                                origin.z + i32::from(z),
                            ),
                            sensor,
                        );
                    }
                }
            }
        }
    }
}
