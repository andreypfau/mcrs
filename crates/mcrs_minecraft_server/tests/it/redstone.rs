use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine::block_simulation::DimensionBlockRules;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::storage::block_entity::{BlockEntity, BlockEntityPos};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::add_vanilla_block_rules;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::host_app::{self, Arrival, label_of};
use crate::world_server::{
    OVERWORLD, WorldServer, block_state, cell_worlds, lockstep_tick, place, wait_for_generation,
};

const BASE: BlockPos = BlockPos::new(0, 120, 0);
const LAMP: BlockPos = BlockPos::new(1, 120, 0);
const LEVER: BlockPos = BlockPos::new(0, 121, 0);

fn lever(app: &App, powered: bool) -> BlockStateId {
    let blocks = app.world().resource::<Blocks>();
    let lever = blocks.block("minecraft:lever").expect("a lever");
    let state = lever
        .with_text(lever.default_state_id, "face", "floor")
        .expect("a floor lever");
    lever
        .with(state, "powered", &PropertyValue::Bool(powered))
        .expect("a lever state")
}

fn stone_plate(app: &App, powered: bool) -> BlockStateId {
    let blocks = app.world().resource::<Blocks>();
    let plate = blocks
        .block("minecraft:stone_pressure_plate")
        .expect("a stone plate");
    plate
        .with(
            plate.default_state_id,
            "powered",
            &PropertyValue::Bool(powered),
        )
        .expect("a plate state")
}

fn piston_facing_east(app: &App) -> BlockStateId {
    let blocks = app.world().resource::<Blocks>();
    let piston = blocks.block("minecraft:piston").expect("a piston");
    piston
        .with_text(piston.default_state_id, "facing", "east")
        .expect("a piston facing east")
}

fn moving_record_at(app: &mut App, label: Entity, pos: BlockPos) -> Option<GeneratedBlockEntity> {
    for world in cell_worlds(app, label) {
        let mut query = world.query::<(&BlockEntityPos, &BlockEntity)>();
        let found = query
            .iter(world)
            .find(|(at, entity)| {
                at.0 == pos && matches!(entity.0, GeneratedBlockEntity::Piston { .. })
            })
            .map(|(_, entity)| entity.0.clone());
        if found.is_some() {
            return found;
        }
    }
    None
}

fn lit_lamp(app: &App) -> BlockStateId {
    let blocks = app.world().resource::<Blocks>();
    let lamp = blocks.block("minecraft:redstone_lamp").expect("a lamp");
    lamp.with(lamp.default_state_id, "lit", &PropertyValue::Bool(true))
        .expect("a lit lamp")
}

fn block_at(app: &mut App, label: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let section = SectionPos::from(pos);
    for world in cell_worlds(app, label) {
        let owned = world.get_resource::<OwnedColumns>().copied();
        if owned.is_some_and(|owned| !owned.owns(ColumnPos::from(pos))) {
            continue;
        }
        let mut query = world.query::<&SectionIndex>();
        if let Some(blocks) = query.iter(world).find_map(|index| index.blocks(section)) {
            return Some(blocks.get(LocalPos::from(pos)));
        }
    }
    None
}

fn admit(app: &mut App, label: Entity) -> PlayerSession {
    let arrival = Arrival {
        uuid: Uuid::from_u128(0x5265_6473_746f_6e65),
        name: "sparky",
        position: DVec3::new(0.5, 122.0, 2.5),
        view_distance: 2,
    };
    let anchor = host_app::admit(app, label, &arrival);
    host_app::session_of(app, anchor).expect("a placed session")
}

#[test]
fn a_lever_on_a_block_lights_and_darkens_the_lamp_beside_it() {
    let mut app = WorldServer {
        rules: DimensionBlockRules(add_vanilla_block_rules),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let actor = admit(&mut app, label);
    let stone = block_state(&app, "minecraft:stone");
    let lamp = block_state(&app, "minecraft:redstone_lamp");
    let lit = lit_lamp(&app);
    let pulled = lever(&app, true);
    let released = lever(&app, false);
    let mut seen = Vec::new();
    for tick in 0..70 {
        match tick {
            25 => {
                place(&mut app, label, actor, BASE, stone);
                place(&mut app, label, actor, LAMP, lamp);
            }
            30 => place(&mut app, label, actor, LEVER, released),
            35 => place(&mut app, label, actor, LEVER, pulled),
            50 => place(&mut app, label, actor, LEVER, released),
            _ => {}
        }
        lockstep_tick(&mut app, label);
        seen.push((tick, block_at(&mut app, label, LAMP)));
    }
    let lamp_at = |tick: usize| seen.get(tick).and_then(|(_, state)| *state);
    assert_eq!(
        lamp_at(34),
        Some(lamp),
        "the lamp is dark while the lever is open"
    );
    assert_eq!(
        lamp_at(38),
        Some(lit),
        "the lever on the block lights the lamp beside it"
    );
    assert_eq!(
        lamp_at(51),
        Some(lit),
        "the lamp waits four ticks after the lever opens"
    );
    assert_eq!(lamp_at(60), Some(lamp), "the lamp goes dark");
}

#[test]
fn a_player_standing_on_a_stone_plate_presses_it() {
    let mut app = WorldServer {
        rules: DimensionBlockRules(add_vanilla_block_rules),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let actor = admit(&mut app, label);
    let stone = block_state(&app, "minecraft:stone");
    let released = stone_plate(&app, false);
    let pressed = stone_plate(&app, true);
    let under_feet = BlockPos::new(0, 121, 2);
    let plate = BlockPos::new(0, 122, 2);
    let mut seen = Vec::new();
    for tick in 0..60 {
        match tick {
            25 => place(&mut app, label, actor, under_feet, stone),
            30 => place(&mut app, label, actor, plate, released),
            _ => {}
        }
        lockstep_tick(&mut app, label);
        seen.push(block_at(&mut app, label, plate));
    }
    assert_eq!(
        seen.get(30).copied().flatten(),
        Some(released),
        "the plate is placed under the player and reads the player from the next tick"
    );
    assert_eq!(
        seen.get(40).copied().flatten(),
        Some(pressed),
        "a player is a living entity and presses the stone plate"
    );
}

#[test]
fn a_pushed_block_leaves_a_moving_record_and_arrives() {
    let mut app = WorldServer {
        rules: DimensionBlockRules(add_vanilla_block_rules),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let actor = admit(&mut app, label);
    let stone = block_state(&app, "minecraft:stone");
    let piston = piston_facing_east(&app);
    let source = block_state(&app, "minecraft:redstone_block");
    let base = BlockPos::new(0, 110, 0);
    let pushed = BlockPos::new(1, 110, 0);
    let destination = BlockPos::new(2, 110, 0);
    let mut records = Vec::new();
    let mut arrived = None;
    for tick in 0..60 {
        match tick {
            25 => {
                place(&mut app, label, actor, base, piston);
                place(&mut app, label, actor, pushed, stone);
            }
            30 => place(&mut app, label, actor, BlockPos::new(0, 111, 0), source),
            _ => {}
        }
        lockstep_tick(&mut app, label);
        if let Some(record) = moving_record_at(&mut app, label, destination) {
            records.push((tick, record));
        }
        if arrived.is_none() && block_at(&mut app, label, destination) == Some(stone) {
            arrived = Some(tick);
        }
    }
    let (_, record) = records
        .first()
        .expect("the moving block leaves a record in the block entities");
    let GeneratedBlockEntity::Piston {
        block_state,
        extending,
        source: is_piston_part,
        ..
    } = record
    else {
        panic!("a piston record");
    };
    assert_eq!(block_state.get_string("Name"), Some("minecraft:stone"));
    assert!(*extending, "an extension carries the stone");
    assert!(!is_piston_part, "the stone is not a part of the piston");
    assert!(arrived.is_some(), "the stone arrives at its destination");
    assert!(
        moving_record_at(&mut app, label, destination).is_none(),
        "the record goes with the moving block"
    );
}
