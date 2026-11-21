use std::path::Path;

use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_voxel::{BlockPos, BlockStateId};

use super::corpus::{Placement, Probe, Scenario, corpus};
use super::server::{VanillaServer, Verdict, server_jar};
use crate::world::{FLOOR, World, definitions};

const VANILLA_FLOOR: i32 = -61;
const VANILLA_ORIGIN: (i32, i32, i32) = (0, VANILLA_FLOOR + 1, 0);
const SCENARIO_SPACING: i32 = 48;
const SCENARIO_REACH: i32 = 40;

fn parse(text: &str) -> (&str, Vec<(&str, &str)>) {
    let (name, rest) = text.split_once('[').unwrap_or((text, ""));
    let properties = rest
        .trim_end_matches(']')
        .split(',')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| pair.split_once('='))
        .collect();
    (name, properties)
}

fn state_of(text: &str) -> BlockStateId {
    let (name, properties) = parse(text);
    let entry = definitions()
        .block(name)
        .unwrap_or_else(|| panic!("the block {name}"));
    properties
        .into_iter()
        .fold(entry.default_state_id, |state, (property, value)| {
            entry
                .with_text(state, property, value)
                .unwrap_or_else(|| panic!("{property}={value} of {name}"))
        })
}

fn matches(state: BlockStateId, text: &str) -> bool {
    let (name, properties) = parse(text);
    let Some(entry) = definitions().owner(state) else {
        return false;
    };
    if entry.identifier.as_str() != name {
        return false;
    }
    properties.into_iter().all(|(property, value)| {
        entry
            .value_of(state, property)
            .map(PropertyValue::to_text)
            .is_some_and(|actual| actual == value)
    })
}

fn kernel_pos(origin: BlockPos, at: (i32, i32, i32)) -> BlockPos {
    BlockPos::new(origin.x + at.0, origin.y + at.1, origin.z + at.2)
}

fn vanilla_pos(offset: i32, at: (i32, i32, i32)) -> (i32, i32, i32) {
    (
        VANILLA_ORIGIN.0 + at.0,
        VANILLA_ORIGIN.1 + at.1,
        VANILLA_ORIGIN.2 + offset + at.2,
    )
}

type Trace = Vec<Vec<Option<usize>>>;

fn run_kernel(scenario: &Scenario) -> Trace {
    let mut world = World::new();
    let origin = BlockPos::new(0, FLOOR, 0);
    for Placement { at, state } in &scenario.build {
        world.place(kernel_pos(origin, *at), state_of(state));
    }
    let mut trace = Vec::with_capacity(scenario.ticks);
    for tick in 0..scenario.ticks {
        for (when, Placement { at, state }) in &scenario.actions {
            if *when == tick {
                world.place(kernel_pos(origin, *at), state_of(state));
            }
        }
        world.tick();
        trace.push(
            scenario
                .probes
                .iter()
                .map(|Probe { at, candidates }| {
                    let state = world.state(kernel_pos(origin, *at));
                    candidates.iter().position(|text| matches(state, text))
                })
                .collect(),
        );
    }
    trace
}

fn run_vanilla(server: &mut VanillaServer, scenario: &Scenario, offset: i32) -> Trace {
    for Placement { at, state } in &scenario.build {
        let (x, y, z) = vanilla_pos(offset, *at);
        server.run(&format!("setblock {x} {y} {z} {state}"));
    }
    server.run("say built");
    server.expect_line("built", std::time::Duration::from_secs(20));
    let mut trace = Vec::with_capacity(scenario.ticks);
    for tick in 0..scenario.ticks {
        for (when, Placement { at, state }) in &scenario.actions {
            if *when == tick {
                let (x, y, z) = vanilla_pos(offset, *at);
                server.run(&format!("setblock {x} {y} {z} {state}"));
            }
        }
        server.step();
        trace.push(
            scenario
                .probes
                .iter()
                .map(|Probe { at, candidates }| {
                    candidates.iter().position(|text| {
                        server.test_block(vanilla_pos(offset, *at), text) == Verdict::Passed
                    })
                })
                .collect(),
        );
    }
    trace
}

fn clear_vanilla(server: &mut VanillaServer, offset: i32) {
    let (x, y, z) = VANILLA_ORIGIN;
    let centre = offset + z;
    for (low, high) in [
        (centre - SCENARIO_REACH, centre),
        (centre + 1, centre + SCENARIO_REACH),
    ] {
        server.run(&format!(
            "fill {} {} {} {} {} {} minecraft:air",
            x - SCENARIO_REACH,
            y,
            low,
            x + SCENARIO_REACH,
            y + 4,
            high
        ));
        server.expect_any(
            &["Successfully filled", "No blocks were filled"],
            std::time::Duration::from_secs(20),
        );
    }
}

fn describe(scenario: &Scenario, trace: &Trace) -> String {
    let mut text = String::new();
    for (tick, row) in trace.iter().enumerate() {
        text.push_str(&format!("tick {tick}:"));
        for (probe, found) in scenario.probes.iter().zip(row) {
            let shown = found
                .and_then(|index| probe.candidates.get(index))
                .copied()
                .unwrap_or("none");
            text.push_str(&format!(" {:?}={shown}", probe.at));
        }
        text.push('\n');
    }
    text
}

#[test]
fn the_corpus_ticks_as_the_vanilla_server_does() {
    let Some(jar) = server_jar() else {
        return;
    };
    let work = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/vanilla-server/redstone");
    let mut server = VanillaServer::start(&jar, &work);
    server.run("tick freeze");
    server.expect_line("frozen", std::time::Duration::from_secs(20));
    server.run("forceload add -64 -64 63 383");
    server.run("gamerule doDaylightCycle false");
    server.run("gamerule randomTickSpeed 0");
    server.run("gamerule doWeatherCycle false");
    let scenarios = corpus();
    let mut failures = Vec::new();
    for (index, scenario) in scenarios.iter().enumerate() {
        let offset = i32::try_from(index).expect("a small corpus") * SCENARIO_SPACING;
        let kernel = run_kernel(scenario);
        let vanilla = run_vanilla(&mut server, scenario, offset);
        clear_vanilla(&mut server, offset);
        if kernel != vanilla {
            failures.push(format!(
                "{}\nkernel:\n{}vanilla:\n{}",
                scenario.name,
                describe(scenario, &kernel),
                describe(scenario, &vanilla)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} scenarios differ from the vanilla server:\n{}",
        failures.len(),
        scenarios.len(),
        failures.join("\n")
    );
}
