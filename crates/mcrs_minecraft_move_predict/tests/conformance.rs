//! Layer 2 against recordings of `tools/mcrs_minecraft_client_recorder`. A vanilla client in the
//! conformance tests C1 to C7 of `docs/spec/movement-checks.md` and in the player experiments of
//! `docs/spec/entity-physics.md` raises no flag. A client with a cheat module, in
//! `tests/fixtures/cheats/<module>`, raises a flag when the module changes the movement.
//!
//! The replay takes the time of each event at the proxy as the time of the server: a change goes
//! out when the proxy passed it to the client, and the client ticks before its application predict
//! without it.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use glam::DVec3;
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
use mcrs_minecraft_physics::set_attribute;
use mcrs_minecraft_physics::set_effect;
use mcrs_minecraft_physics::sprint_modifiers;
use mcrs_minecraft_physics::test_state;
use mcrs_voxel::BlockPos;

const HONEST: [&str; 2] = [
    "tests/fixtures/conformance",
    "../mcrs_minecraft_physics/tests/fixtures/player",
];

/// The cheat modules that change the movement that layer 2 predicts.
const PREDICTED_CHEATS: [&str; 5] = ["fly", "speed", "no_fall", "phase", "step"];

/// A faster tick rate passes layer 2: the tick rate check of layer 1 sees it.
const UNPREDICTED_CHEATS: [&str; 1] = ["timer"];

#[test]
fn a_vanilla_client_raises_no_flag() {
    let paths: Vec<PathBuf> = HONEST.iter().flat_map(|path| recordings(path)).collect();
    if paths.is_empty() {
        println!("skipped: no recordings in {HONEST:?}");
        return;
    }
    let mut total = Tally::default();
    for path in &paths {
        let tally = replay_file(path).unwrap();
        println!(
            "{}: {} predicted ticks, {} coarse, {} flags",
            path.display(),
            tally.predicted,
            tally.coarse,
            tally.flags.len()
        );
        total.predicted += tally.predicted;
        total.coarse += tally.coarse;
        let name = path.display();
        total
            .flags
            .extend(tally.flags.iter().map(|flag| format!("{name}: {flag}")));
    }
    println!(
        "{} recordings, {} predicted ticks, {} coarse, {} flags; by the rule of three the false \
         positive rate is below 3/{} = {:.2e} per tick at 95 % confidence",
        paths.len(),
        total.predicted,
        total.coarse,
        total.flags.len(),
        total.predicted,
        3.0 / total.predicted.max(1) as f64
    );
    assert!(
        total.flags.is_empty(),
        "a vanilla client has no violation:\n{}",
        total.flags.join("\n")
    );
}

#[test]
fn each_recording_of_a_predicted_cheat_module_is_flagged() {
    let mut missed = Vec::new();
    for module in PREDICTED_CHEATS.iter().chain(&UNPREDICTED_CHEATS) {
        let paths = recordings(&format!("tests/fixtures/cheats/{module}"));
        if paths.is_empty() {
            println!("{module}: skipped, no recordings");
            continue;
        }
        let mut detected = 0;
        for path in &paths {
            let tally = replay_file(path).unwrap();
            if !tally.flags.is_empty() {
                detected += 1;
            } else if *module == "no_fall" {
                assert_eq!(
                    tally.accepted,
                    without_on_ground_bits(path).unwrap().accepted,
                    "{}: MVC-CHK-002 exempts the first client tick after an accept, and the server \
                     never uses the on-ground bit",
                    path.display()
                );
            } else if PREDICTED_CHEATS.contains(module) {
                missed.push(path.display().to_string());
            }
        }
        println!(
            "{module}: {detected} of {} recordings flagged ({:.0} %)",
            paths.len(),
            100.0 * f64::from(detected) / paths.len() as f64
        );
    }
    assert!(
        missed.is_empty(),
        "a module that changes the movement raises a flag in each recording:\n{}",
        missed.join("\n")
    );
}

/// The `.txt` files under the directory `relative` to the crate, in subdirectories too, sorted.
fn recordings(relative: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut directories = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "txt") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

fn replay_file(path: &Path) -> Result<Tally, String> {
    replay_edited(path, |_| {})
}

fn without_on_ground_bits(path: &Path) -> Result<Tally, String> {
    replay_edited(path, |event| {
        if let Event::Move(step) = event {
            step.on_ground = false;
        }
    })
}

fn replay_edited(path: &Path, edit: impl FnMut(&mut Event)) -> Result<Tally, String> {
    let context = |error: String| format!("{}: {error}", path.display());
    let text = fs::read_to_string(path).map_err(|error| context(error.to_string()))?;
    let mut recording: Recording = text
        .parse()
        .map_err(|error| context(format!("{error:?}")))?;
    recording.events.iter_mut().for_each(edit);
    replay(&recording).map_err(context)
}

#[derive(Debug, Default)]
struct Tally {
    predicted: usize,
    coarse: usize,
    flags: Vec<String>,
    /// The accepted state after each client tick.
    accepted: Vec<PlayerState>,
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
    /// The changes that the client applied: the answer to the probe after each is in.
    applied: u64,
}

/// Sends each clientbound event at its time, and feeds the serverbound events in the order of
/// the client. The probe after a change gives the tick that applied it, so a change is applied
/// from the tick after its place in the recording on.
fn replay(recording: &Recording) -> Result<Tally, String> {
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
        applied: 0,
    };
    let timed: Vec<(u64, &Event)> = recording
        .times
        .iter()
        .copied()
        .zip(&recording.events)
        .collect();
    let mut sends = timed
        .iter()
        .enumerate()
        .filter(|(_, (_, event))| event.is_clientbound())
        .peekable();
    let mut tally = Tally::default();
    let mut ticks = 0;
    for (index, &(time, event)) in timed.iter().enumerate() {
        while let Some((_, &(_, sent))) =
            sends.next_if(|&(send, &(sent_at, _))| send <= index || sent_at < time)
        {
            server.send(sent)?;
        }
        let outcome = if event.is_clientbound() {
            server.apply(event)?
        } else if *event == Event::TickEnd {
            ticks += 1;
            let outcome = server.tick();
            tally.accepted.push(*server.prediction.accepted());
            match outcome.mode {
                Mode::Predicted => tally.predicted += 1,
                Mode::Coarse => tally.coarse += 1,
                Mode::Resync | Mode::Exempt => {}
            }
            outcome
        } else {
            server.packet(event);
            continue;
        };
        let unflagged = Outcome {
            mode: Mode::Predicted,
            ..outcome
        };
        if unflagged != Outcome::default() {
            tally
                .flags
                .push(format!("client tick {ticks}: {outcome:?}"));
        }
    }
    Ok(tally)
}

fn teleport_of(teleport: &mcrs_minecraft_client_ticks::Teleport) -> Result<Teleport, String> {
    Ok(Teleport {
        position: DVec3::from_array(teleport.position),
        velocity: DVec3::from_array(teleport.velocity),
        yaw: teleport.rotation.yaw,
        pitch: teleport.rotation.pitch,
        relative: Relative::from_bits_truncate(
            u16::try_from(teleport.relative).map_err(|error| error.to_string())?,
        ),
    })
}

impl Server {
    /// The server writes a clientbound event: a teleport is pending, a change is in its window.
    fn send(&mut self, event: &Event) -> Result<(), String> {
        let change = match event {
            Event::Teleport(teleport) => {
                self.prediction.teleport(teleport_of(teleport)?);
                return Ok(());
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
            _ => return Ok(()),
        };
        self.prediction.change(change);
        Ok(())
    }

    /// The client applied a clientbound event; a teleport answers the outcome of its accept.
    fn apply(&mut self, event: &Event) -> Result<Outcome, String> {
        match event {
            Event::Teleport(teleport) => {
                let mut destination = *self.prediction.accepted();
                apply_teleport(
                    &mut destination,
                    &mut self.rotation,
                    &teleport_of(teleport)?,
                );
                return Ok(self.prediction.accept(destination.position));
            }
            Event::ChunkForgotten([x, z]) => self.world.unload_chunk(*x, *z),
            _ => self.applied += 1,
        }
        Ok(Outcome::from(Mode::Exempt))
    }

    fn tick(&mut self) -> Outcome {
        self.tick.input.rotation = self.rotation;
        self.tick.changes = ChangeStates {
            applied: self.applied,
            seen: self.applied,
        };
        let view = View::new(&self.world, &NoBodies, &self.tables);
        let outcome = self.prediction.tick(view, &[] as &[Pusher], &self.tick);
        self.tick.sprint = None;
        self.tick.item_use = None;
        self.tick.reported.position = None;
        outcome
    }

    /// A serverbound packet of the tick.
    fn packet(&mut self, event: &Event) {
        let tick = &mut self.tick;
        match event {
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
