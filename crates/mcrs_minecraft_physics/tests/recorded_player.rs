//! Parity of the own player with the client ticks of a vanilla client, recorded by
//! `tools/mcrs_minecraft_client_recorder` into `tests/fixtures/player`.

use std::fs;
use std::path::Path;

use glam::DVec3;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_physics::MotionAttributes;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Orientation;
use mcrs_minecraft_physics::PlayerInput;
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
use mcrs_minecraft_physics::set_attribute;
use mcrs_minecraft_physics::set_effect;
use mcrs_minecraft_physics::sprint_modifiers;
use mcrs_minecraft_physics::test_state;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;

#[test]
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

#[test]
fn the_sprint_modifier_of_the_recording_is_left_to_the_kernel() {
    let recording: Recording = "client-ticks 2
version test
experiment E9
step sprint
instruction none
events
0 < attribute minecraft:movement_speed 0.1 test:armor 0.5 1
0 > sprint true
0 > tick
0 < attribute minecraft:movement_speed 0.1 test:armor 0.5 1 test:sprint 0.30000001192092896 2
0 > tick
0 > sprint false
0 > tick
0 < attribute minecraft:movement_speed 0.1 test:armor 0.5 1
0 > tick
"
    .parse()
    .unwrap();
    let sprint = sprint_modifiers(&recording);
    assert_eq!(sprint, ["test:sprint"]);
    let Some(Event::Attribute(sprinting)) = recording.events.get(3) else {
        panic!("the fourth event is the update while the client sprints");
    };
    let mut attributes = MotionAttributes::default();
    set_attribute(&mut attributes, sprinting, &sprint);
    assert_eq!(attributes.movement_speed, 0.1 * (1.0 + 0.5));
}

fn held_keys(keys: &mcrs_minecraft_client_ticks::Keys) -> mcrs_minecraft_physics::Keys {
    mcrs_minecraft_physics::Keys {
        forward: keys.forward,
        back: keys.backward,
        left: keys.left,
        right: keys.right,
        jump: keys.jump,
        shift: keys.shift,
        sprint: keys.sprint,
        ..Default::default()
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
    let sprint = sprint_modifiers(recording);
    let mut state = PlayerState::new(DVec3::ZERO);
    let mut input = PlayerInput::default();
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    for (tick, events) in recording.ticks().enumerate() {
        let mut sent = None;
        for event in events {
            match event {
                Event::Teleport(teleport) => apply_teleport(
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
                Event::Motion(velocity) => state.body.velocity = DVec3::from_array(*velocity),
                Event::Explosion(impulse) => state.body.velocity += DVec3::from_array(*impulse),
                Event::Abilities(abilities) => {
                    state.may_fly = abilities.may_fly;
                    state.flight0 = abilities.flying;
                    state.flight_speed = abilities.flying_speed;
                }
                Event::Attribute(attribute) => {
                    set_attribute(&mut state.attributes, attribute, &sprint);
                }
                Event::Effect(effect) => {
                    set_effect(&mut state.effects, &effect.name, Some(effect.amplifier));
                }
                Event::RemoveEffect(effect) => set_effect(&mut state.effects, effect, None),
                Event::Block(change) => {
                    let [x, y, z] = change.position;
                    world.set(BlockPos::new(x, y, z), test_state(&blocks, &change.state)?);
                }
                Event::ChunkForgotten([x, z]) => world.unload_chunk(*x, *z),
                Event::UseItem => state.item_use.using = true,
                Event::ReleaseUseItem => state.item_use.using = false,
                Event::Input(keys) => input.keys = held_keys(keys),
                Event::Move(step) => {
                    if let Some(rotation) = step.rotation {
                        input.rotation = Orientation::facing(rotation.yaw, rotation.pitch);
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
            assert_eq!(
                state.body.position.to_array(),
                position,
                "{name}, tick {tick}"
            );
        }
        assert_eq!(
            state.body.flags.on_ground, step.on_ground,
            "{name}, tick {tick}"
        );
        assert_eq!(
            state.body.flags.horizontal_collision, step.horizontal_collision,
            "{name}, tick {tick}"
        );
    }
    Ok(())
}
