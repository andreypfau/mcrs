//! Conformance C1 to C7 of `docs/spec/movement-checks.md`: the client ticks of a vanilla client,
//! recorded by `tools/mcrs_minecraft_client_recorder` into `tests/fixtures/conformance`, pass
//! layer 2 with no violation and no setback.

use std::fs;
use std::path::Path;

use glam::DVec3;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_minecraft_move_predict::Change;
use mcrs_minecraft_move_predict::ChangeStates;
use mcrs_minecraft_move_predict::ClientTick;
use mcrs_minecraft_move_predict::Limits;
use mcrs_minecraft_move_predict::Mode;
use mcrs_minecraft_move_predict::Outcome;
use mcrs_minecraft_move_predict::Prediction;
use mcrs_minecraft_move_predict::Pusher;
use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::MotionAttributes;
use mcrs_minecraft_physics::MotionEffects;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Relative;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::Teleport;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::apply_teleport;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

#[test]
#[ignore = "needs recorded vanilla client fixtures"]
fn a_vanilla_client_passes_layer_2() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("no recordings in {}: {error}", directory.display()))
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
        replay(&name, &recording).unwrap();
    }
}

/// The server side of a recording: its world, layer 2 and the state the changes carry.
struct Server {
    blocks: TestBlocks,
    tables: Tables,
    world: TestWorld,
    prediction: Prediction,
    sprint: Vec<String>,
    attributes: MotionAttributes,
    effects: MotionEffects,
    rotation: PlayerRotation,
    tick: ClientTick,
    sent: u64,
}

/// Feeds the recorded ticks to layer 2 with each change applied in the tick that applied it.
fn replay(name: &str, recording: &Recording) -> Result<(), String> {
    let blocks = TestBlocks::new();
    let mut world = TestWorld::new();
    for fill in &recording.header.fills {
        let [x0, y0, z0] = fill.min;
        let [x1, y1, z1] = fill.max;
        let state = test_state(&blocks, &fill.state)?;
        world.fill(BlockPos::new(x0, y0, z0), BlockPos::new(x1, y1, z1), state);
    }
    let mut server = Server {
        tables: Tables::new(blocks.table.clone()),
        blocks,
        world,
        prediction: Prediction::new(PlayerState::new(DVec3::ZERO), Limits::default()),
        sprint: sprint_modifiers(recording),
        attributes: MotionAttributes::default(),
        effects: MotionEffects::default(),
        rotation: PlayerRotation::default(),
        tick: ClientTick::default(),
        sent: 0,
    };
    for (number, events) in recording.ticks().enumerate() {
        server.tick.sprint = None;
        server.tick.item_use = None;
        server.tick.reported.position = None;
        for event in events {
            let outcome = server.event(event)?;
            assert_eq!(outcome, Outcome::default(), "{name}, tick {number}");
        }
        server.tick.input.rotation = server.rotation;
        server.tick.changes = ChangeStates {
            applied: server.sent,
            seen: server.sent,
        };
        let view = View::new(&server.world, &NoBodies, &server.tables);
        let outcome = server.prediction.tick(view, &[] as &[Pusher], &server.tick);
        let passed = outcome.mode == Mode::Exempt || outcome == Outcome::default();
        assert!(passed, "{name}, tick {number}: {outcome:?}");
    }
    Ok(())
}

impl Server {
    /// Takes one recorded packet; a teleport answers the outcome of its accept.
    fn event(&mut self, event: &Event) -> Result<Outcome, String> {
        let change = match event {
            Event::Teleport(teleport) => {
                let teleport = Teleport {
                    position: DVec3::from_array(teleport.position),
                    velocity: DVec3::from_array(teleport.velocity),
                    yaw: teleport.rotation.yaw,
                    pitch: teleport.rotation.pitch,
                    relative: Relative::from_bits_truncate(
                        u16::try_from(teleport.relative).map_err(|error| error.to_string())?,
                    ),
                };
                let mut destination = *self.prediction.accepted();
                let sine = &self.tables.sine;
                apply_teleport(sine, &mut destination, &mut self.rotation, &teleport);
                self.prediction.teleport(teleport);
                return Ok(self.prediction.accept(&self.tables, destination.position));
            }
            Event::Motion(velocity) => Change::Motion(DVec3::from_array(*velocity)),
            Event::Explosion(impulse) => Change::Impulse(DVec3::from_array(*impulse)),
            Event::Abilities(abilities) => Change::Abilities {
                may_fly: abilities.may_fly,
                flying: abilities.flying,
                flight_speed: abilities.flying_speed,
            },
            Event::Attribute(attribute) => {
                set_attribute(&mut self.attributes, attribute, &self.sprint);
                Change::Attributes(self.attributes)
            }
            Event::Effect(effect) => {
                set_effect(&mut self.effects, &effect.name, Some(effect.amplifier));
                Change::Effects(self.effects)
            }
            Event::RemoveEffect(effect) => {
                set_effect(&mut self.effects, effect, None);
                Change::Effects(self.effects)
            }
            Event::Block(block) => {
                let [x, y, z] = block.position;
                let at = BlockPos::new(x, y, z);
                let new = test_state(&self.blocks, &block.state)?;
                let old = self.world.get(at);
                self.world.set(at, new);
                Change::Block { at, old, new }
            }
            _ => {
                self.packet(event);
                return Ok(Outcome::default());
            }
        };
        self.prediction.change(change);
        self.sent += 1;
        Ok(Outcome::default())
    }

    /// A packet that is not a predicted-state change.
    fn packet(&mut self, event: &Event) {
        let tick = &mut self.tick;
        match event {
            Event::ChunkForgotten([x, z]) => self.world.unload_chunk(*x, *z),
            Event::Input(keys) => {
                let held = &mut tick.input.keys;
                held.forward = keys.forward;
                held.back = keys.backward;
                held.left = keys.left;
                held.right = keys.right;
                held.jump = keys.jump;
                held.shift = keys.shift;
                held.sprint = keys.sprint;
            }
            Event::Sprinting(on) => tick.sprint = Some(*on),
            Event::UseItem => tick.item_use = Some(true),
            Event::ReleaseUseItem => tick.item_use = Some(false),
            Event::Move(step) => {
                if let Some(look) = step.rotation {
                    self.rotation = PlayerRotation {
                        yaw: look.yaw,
                        pitch: look.pitch,
                    };
                }
                tick.reported = Move {
                    position: step.position.map(DVec3::from_array),
                    look: None,
                    on_ground: step.on_ground,
                    horizontal_collision: step.horizontal_collision,
                };
            }
            _ => {}
        }
    }
}

/// The ids of the `movement_speed` modifiers that follow the sprint state of the client.
fn sprint_modifiers(recording: &Recording) -> Vec<String> {
    let mut sprinting = false;
    let mut while_sprinting: Option<Vec<String>> = None;
    let mut otherwise = Vec::new();
    for event in &recording.events {
        match event {
            Event::Sprinting(on) => sprinting = *on,
            Event::Attribute(attribute) if attribute.name == "minecraft:movement_speed" => {
                let ids: Vec<String> = attribute
                    .modifiers
                    .iter()
                    .map(|modifier| modifier.id.clone())
                    .collect();
                match &mut while_sprinting {
                    _ if !sprinting => otherwise.extend(ids),
                    Some(kept) => kept.retain(|id| ids.contains(id)),
                    None => while_sprinting = Some(ids),
                }
            }
            _ => {}
        }
    }
    let mut ids = while_sprinting.unwrap_or_default();
    ids.retain(|id| !otherwise.contains(id));
    ids
}

/// The value of an attribute without the sprint modifiers, which the physics model applies from
/// the sprint state.
fn set_attribute(attributes: &mut MotionAttributes, attribute: &Attribute, sprint: &[String]) {
    let modifiers = || {
        attribute
            .modifiers
            .iter()
            .filter(|modifier| !sprint.contains(&modifier.id))
    };
    let sum = |operation| -> f64 {
        modifiers()
            .filter(|modifier| modifier.operation == operation)
            .map(|modifier| modifier.amount)
            .sum()
    };
    let total = modifiers()
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
