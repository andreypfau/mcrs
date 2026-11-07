use std::collections::HashMap;

use glam::{IVec3, Vec3};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_mesh::smooth_light::{
    self, CellLight, LightState, Sample, VertexLight, VertexValues,
};
use serde::Deserialize;

const GOLDEN: &str = include_str!("../fixtures/vanilla/block_smooth_light.json");

#[derive(Deserialize)]
struct Golden {
    palette: Vec<PaletteEntry>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct PaletteEntry {
    state: String,
    full_collision: bool,
    solid_render: bool,
    light_dampening: u8,
    light_emission: u8,
    emissive_rendering: bool,
    occlusion_shade: f32,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    direction: Direction,
    own: String,
    positions: [[f32; 3]; 4],
    default_light: [u8; 2],
    cells: Vec<[i32; 6]>,
    own_light: [u8; 2],
    smooth_shade: [u8; 4],
    smooth_light: [[u8; 2]; 4],
    flat_shade: u8,
    flat_light_without_cull_face: [u8; 2],
    flat_light_with_cull_face: [u8; 2],
}

impl PaletteEntry {
    fn light_state(&self) -> LightState {
        LightState {
            occlusion_shade: self.occlusion_shade,
            solid_render: self.solid_render,
            light_dampening: self.light_dampening,
            full_collision: self.full_collision,
            emission: self.light_emission,
            emissive: self.emissive_rendering,
        }
    }

    /// A case names its own state by the block and the properties it sets, and the palette
    /// names every property.
    fn is_named_by(&self, name: &str) -> bool {
        let (block, properties) = split_state(name);
        let (own_block, own_properties) = split_state(&self.state);
        block == own_block
            && properties
                .iter()
                .all(|property| own_properties.contains(property))
    }
}

fn split_state(name: &str) -> (&str, Vec<&str>) {
    match name.split_once('[') {
        Some((block, rest)) => (block, rest.trim_end_matches(']').split(',').collect()),
        None => (name, Vec::new()),
    }
}

fn light([block, sky]: [u8; 2]) -> CellLight {
    CellLight { block, sky }
}

fn values(light: VertexLight) -> [u8; 2] {
    [light.block, light.sky]
}

fn overworld_face_shade(direction: Direction) -> f32 {
    match direction {
        Direction::Down => 0.5,
        Direction::Up => 1.0,
        Direction::North | Direction::South => 0.8,
        Direction::West | Direction::East => 0.6,
    }
}

fn shade_bytes(values: &VertexValues, direction: Direction) -> [u8; 4] {
    values
        .occlusion
        .map(|occlusion| smooth_light::shade_byte(occlusion, overworld_face_shade(direction)))
}

fn check(case: &Case, palette: &[PaletteEntry]) -> Vec<String> {
    let air = palette
        .iter()
        .find(|entry| entry.state == "minecraft:air")
        .expect("the palette names air")
        .light_state();
    let own = palette
        .iter()
        .find(|entry| entry.is_named_by(&case.own))
        .expect("the palette names the own state")
        .light_state();
    let cells: HashMap<IVec3, Sample> = case
        .cells
        .iter()
        .map(|&[x, y, z, index, block, sky]| {
            let entry = usize::try_from(index)
                .ok()
                .and_then(|index| palette.get(index))
                .expect("a cell names a palette entry");
            let sample = Sample {
                state: entry.light_state(),
                light: light([block, sky].map(|level| level as u8)),
            };
            (IVec3::new(x, y, z), sample)
        })
        .collect();
    let sample = |offset: IVec3| {
        assert!(
            offset.abs().max_element() <= 2,
            "no rule reads a cell further than two away, and {offset} is"
        );
        if offset == IVec3::ZERO {
            Sample {
                state: own,
                light: light(case.own_light),
            }
        } else {
            cells.get(&offset).copied().unwrap_or(Sample {
                state: air,
                light: light(case.default_light),
            })
        }
    };
    let positions = case.positions.map(Vec3::from_array);
    let direction = case.direction;

    let mut mismatches = Vec::new();
    let mut compare = |what: &str, ours: String, game: String| {
        if ours != game {
            mismatches.push(format!(
                "{}: {what} {ours} where the game has {game}",
                case.name
            ));
        }
    };
    let smooth = smooth_light::smooth(&positions, direction, sample);
    compare(
        "smooth shade",
        format!("{:?}", shade_bytes(&smooth, direction)),
        format!("{:?}", case.smooth_shade),
    );
    compare(
        "smooth light",
        format!("{:?}", smooth.light.map(values)),
        format!("{:?}", case.smooth_light),
    );
    let culled = smooth_light::flat(&positions, direction, Some(direction), sample);
    compare(
        "flat light with a cull face",
        format!("{:?}", culled.light.map(values)),
        format!("{:?}", [case.flat_light_with_cull_face; 4]),
    );
    let unculled = smooth_light::flat(&positions, direction, None, sample);
    compare(
        "flat light without a cull face",
        format!("{:?}", unculled.light.map(values)),
        format!("{:?}", [case.flat_light_without_cull_face; 4]),
    );
    compare(
        "flat shade",
        format!("{:?}", shade_bytes(&unculled, direction)),
        format!("{:?}", [case.flat_shade; 4]),
    );
    mismatches
}

#[test]
fn vertex_values_match_the_game() {
    let golden: Golden = serde_json::from_str(GOLDEN).expect("the golden artefact parses");
    let mismatches: Vec<String> = golden
        .cases
        .iter()
        .flat_map(|case| check(case, &golden.palette))
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} mismatches over {} cases:\n{}",
        mismatches.len(),
        golden.cases.len(),
        mismatches.join("\n")
    );
}
