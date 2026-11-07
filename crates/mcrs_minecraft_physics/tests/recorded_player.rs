//! Parity of the own player with the client ticks of a vanilla client, recorded by
//! `tools/mcrs_minecraft_client_recorder` into `tests/fixtures/player`.

use std::fs;
use std::path::Path;

use glam::DVec3;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_physics::MotionAttributes;
use mcrs_minecraft_physics::MotionEffects;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Relative;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::Teleport;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::apply_teleport;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

#[test]
#[ignore = "no client ticks are recorded yet; tools/mcrs_minecraft_client_recorder records E9 to E14 and E21 to E23"]
fn own_player_follows_the_recorded_client_ticks() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/player");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .unwrap_or_else(|error| {
            panic!(
                "no recorded client ticks in {}: {error}",
                directory.display()
            )
        })
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no recordings in {}",
        directory.display()
    );
    for path in paths {
        let name = path.display().to_string();
        let recording: Recording = fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(recording.header.version, VERSION.id, "{name}");
        replay(&name, &recording).unwrap();
    }
}

/// Runs the kernel over the client ticks and compares each move packet bit for bit.
fn replay(name: &str, recording: &Recording) -> Result<(), String> {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    for fill in &recording.header.fills {
        let [x0, y0, z0] = fill.min;
        let [x1, y1, z1] = fill.max;
        let state = test_state(&blocks, &fill.state)?;
        world.fill(BlockPos::new(x0, y0, z0), BlockPos::new(x1, y1, z1), state);
    }
    let mut state = PlayerState::new(DVec3::ZERO);
    let mut input = PlayerInput::default();
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    for (tick, events) in recording.ticks().enumerate() {
        let mut sent = None;
        for event in events {
            match event {
                Event::Teleport(teleport) => apply_teleport(
                    &tables.sine,
                    &mut state,
                    &mut input.rotation,
                    &Teleport {
                        position: DVec3::from_array(teleport.position),
                        velocity: DVec3::from_array(teleport.velocity),
                        yaw: teleport.rotation.yaw,
                        pitch: teleport.rotation.pitch,
                        relative: Relative::from_bits_truncate(
                            u16::try_from(teleport.relative).map_err(|error| error.to_string())?,
                        ),
                    },
                ),
                Event::Motion(velocity) => state.velocity = DVec3::from_array(*velocity),
                Event::Explosion(impulse) => state.velocity += DVec3::from_array(*impulse),
                Event::Abilities(abilities) => {
                    state.may_fly = abilities.may_fly;
                    state.flight0 = abilities.flying;
                    state.flight_speed = abilities.flying_speed;
                }
                Event::Attribute(attribute) => set_attribute(&mut state.attributes, attribute),
                Event::Effect(effect) => {
                    let amplifier =
                        u8::try_from(effect.amplifier).map_err(|error| error.to_string())?;
                    set_effect(&mut state.effects, &effect.name, Some(amplifier));
                }
                Event::RemoveEffect(effect) => set_effect(&mut state.effects, effect, None),
                Event::Block(change) => {
                    let [x, y, z] = change.position;
                    world.set(BlockPos::new(x, y, z), test_state(&blocks, &change.state)?);
                }
                Event::ChunkForgotten([x, z]) => world.unload_chunk(*x, *z),
                Event::Input(keys) => {
                    input.keys.forward = keys.forward;
                    input.keys.back = keys.backward;
                    input.keys.left = keys.left;
                    input.keys.right = keys.right;
                    input.keys.jump = keys.jump;
                    input.keys.shift = keys.shift;
                    input.keys.sprint = keys.sprint;
                }
                Event::Move(step) => {
                    if let Some(rotation) = step.rotation {
                        input.rotation = PlayerRotation {
                            yaw: rotation.yaw,
                            pitch: rotation.pitch,
                        };
                    }
                    sent = Some(*step);
                }
                _ => {}
            }
        }
        let view = View {
            world: &world,
            bodies: &NoBodies,
            tables: &tables,
        };
        let start = state;
        tick_player(
            view,
            Side::OwnClient,
            &start,
            &input,
            &mut state,
            &mut actions,
            &mut scratch,
        );
        let Some(step) = sent else { continue };
        if let Some(position) = step.position {
            assert_eq!(state.position.to_array(), position, "{name}, tick {tick}");
        }
        assert_eq!(state.flags.on_ground, step.on_ground, "{name}, tick {tick}");
        assert_eq!(
            state.flags.horizontal_collision, step.horizontal_collision,
            "{name}, tick {tick}"
        );
    }
    Ok(())
}

/// The value of an attribute: the base plus the additions, times the base multipliers, times
/// each total multiplier.
fn set_attribute(attributes: &mut MotionAttributes, attribute: &Attribute) {
    let sum = |operation| -> f64 {
        attribute
            .modifiers
            .iter()
            .filter(|modifier| modifier.operation == operation)
            .map(|modifier| modifier.amount)
            .sum()
    };
    let total = attribute
        .modifiers
        .iter()
        .filter(|modifier| modifier.operation == 2)
        .fold(1.0, |product, modifier| product * (1.0 + modifier.amount));
    let value = (attribute.base + sum(0)) * (1.0 + sum(1)) * total;
    let field = match attribute.name.as_str() {
        "minecraft:gravity" => &mut attributes.gravity,
        "minecraft:step_height" => &mut attributes.step_height,
        "minecraft:movement_speed" => &mut attributes.movement_speed,
        "minecraft:jump_strength" => &mut attributes.jump_strength,
        "minecraft:scale" => &mut attributes.scale,
        "minecraft:sneaking_speed" => &mut attributes.sneaking_speed,
        "minecraft:movement_efficiency" => &mut attributes.movement_efficiency,
        "minecraft:bounciness" => &mut attributes.bounciness,
        "minecraft:friction_modifier" => &mut attributes.friction_modifier,
        "minecraft:air_drag_modifier" => &mut attributes.air_drag_modifier,
        _ => return,
    };
    *field = value;
}

fn set_effect(effects: &mut MotionEffects, name: &str, level: Option<u8>) {
    match name {
        "minecraft:slow_falling" => effects.slow_falling = level.is_some(),
        "minecraft:levitation" => effects.levitation = level,
        "minecraft:jump_boost" => effects.jump_boost = level,
        "minecraft:weaving" => effects.weaving = level.is_some(),
        "minecraft:blindness" => effects.blindness = level.is_some(),
        _ => {}
    }
}

/// The state of the small test table that stands for a recorded block state.
fn test_state(blocks: &TestBlocks, state: &str) -> Result<BlockStateId, String> {
    let (block, properties) = state.split_once('[').unwrap_or((state, ""));
    let has = |property: &str| properties.contains(property);
    Ok(match block {
        "minecraft:air" => BlockStateId::AIR,
        "minecraft:stone" => blocks.stone,
        "minecraft:ice" => blocks.ice,
        "minecraft:slime_block" => blocks.slime,
        "minecraft:soul_sand" => blocks.soul_sand,
        "minecraft:honey_block" => blocks.honey,
        "minecraft:cobweb" => blocks.cobweb,
        "minecraft:sweet_berry_bush" => blocks.sweet_berry_bush,
        "minecraft:powder_snow" => blocks.powder_snow,
        "minecraft:water" => blocks.water,
        _ if block.ends_with("_slab") && has("type=bottom") => blocks.bottom_slab,
        _ if block.ends_with("_slab") && has("type=top") => blocks.top_slab,
        _ if block.ends_with("_carpet") => blocks.carpet,
        _ => return Err(format!("the test table has no state for {state}")),
    })
}
