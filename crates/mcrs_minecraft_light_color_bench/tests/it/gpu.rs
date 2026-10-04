use std::time::Duration;

use crate::common::{self, Neighbourhood, TORCH_TYPE};
use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::{LightColors, LightType};
use mcrs_minecraft_light_color::region::{Palette, Region, section_output};
use mcrs_minecraft_light_color::resolve::{Lanes, resolve};
use mcrs_minecraft_light_color_bench::candidates::gpu::run;
use mcrs_minecraft_light_color_bench::candidates::{Outcome, Stages, mismatch};
use mcrs_minecraft_light_color_bench::fixture::{SECTIONS, Scene, oracle, scene};
use proptest::prelude::*;

const CENTRE: SectionPos = SectionPos(IVec3::new(0, 4, 0));

/// Where the job's palette breaks the reach contract: a type in it that no
/// emitter in the region has, or a type left out that lights the output.
fn reach_breach(
    lane_types: &[LightType],
    region_types: &[LightType],
    want: impl Fn(LightType) -> Vec<u8>,
) -> Option<String> {
    if let Some(t) = lane_types.iter().find(|t| !region_types.contains(t)) {
        return Some(format!(
            "type {} has a lane but no emitter in the region",
            t.0
        ));
    }
    region_types
        .iter()
        .filter(|t| !lane_types.contains(t))
        .find(|&&t| want(t).iter().any(|&level| level > 0))
        .map(|t| format!("type {} lights the output but has no lane", t.0))
}

struct TexelMismatch {
    first: Option<String>,
    differ: usize,
    largest: u8,
}

/// The GPU's texels against the CPU resolve of the server's levels over the
/// job's palette: every channel within 1, and every unlit cell exactly unlit.
fn texel_mismatch(
    section: SectionPos,
    outcome: &Outcome,
    colours: &LightColors,
    want: impl Fn(LightType) -> Vec<u8>,
) -> TexelMismatch {
    let types: Vec<LightType> = outcome.lanes.iter().flatten().map(|(t, _)| *t).collect();
    let (min, size) = section_output(section);
    let levels: Vec<Vec<u8>> = types.iter().map(|&t| want(t)).collect();
    let lanes = Lanes {
        bytes: types.len(),
        levels: (0..size.pow(3) as usize)
            .flat_map(|cell| levels.iter().map(move |lane| lane[cell]))
            .collect(),
    };
    let output = Region {
        min,
        size,
        blocks: Box::default(),
    };
    let cpu = resolve(&output, &lanes, &Palette { types }, colours, min, size);
    let mut found = TexelMismatch {
        first: None,
        differ: 0,
        largest: 0,
    };
    for (cell, ((pos, got), want)) in common::output_positions(section)
        .zip(outcome.texels.iter())
        .zip(cpu.iter())
        .enumerate()
    {
        let unlit = levels.iter().all(|lane| lane[cell] == 0);
        let delta = got
            .iter()
            .zip(want)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        found.differ += (delta > 0) as usize;
        found.largest = found.largest.max(delta);
        if found.first.is_none() && (delta > 1 || unlit && *got != [0, 0, 0, 255]) {
            found.first = Some(format!(
                "at {pos} the GPU texel is {got:?}, the CPU's {want:?}"
            ));
        }
    }
    found
}

fn scene_of(world: &Neighbourhood) -> Scene {
    let mut scene = Scene {
        name: String::new(),
        bounds: world.bounds,
        origin: SectionPos(world.centre.0 - IVec3::splat(2)),
        registry: common::registry().as_ref().clone(),
        colours: common::colours(),
        blocks: Vec::new(),
        light: vec![None; SECTIONS],
    };
    scene.blocks = (0..SECTIONS)
        .map(|slot| world.section(scene.section_at(slot)).map(|c| Box::new(*c)))
        .collect();
    scene
}

/// The GPU's outcome for the world's centre section, once its palette, every
/// lane and every texel are checked against the server's relax.
fn gpu_matches_relax(world: &Neighbourhood) -> Option<Outcome> {
    let scene = scene_of(world);
    let (min, size) = section_output(world.centre);
    let region = Region::new(min, size, scene.bounds, &scene.registry, scene.cells());
    let types = Palette::of(&region, &scene.registry, &scene.colours).types;
    let outcome = run(&scene, world.centre, &mut Stages::default());
    let lanes = outcome.as_ref().map_or(&[][..], |o| {
        o.lanes.as_deref().expect("the GPU keeps its lanes")
    });
    let lane_types: Vec<LightType> = lanes.iter().map(|(t, _)| *t).collect();
    if let Some(problem) = reach_breach(&lane_types, &types, |t| common::oracle(world, t)) {
        panic!("{problem}");
    }
    if let Some(first) = mismatch(world.centre, lanes, |t| common::oracle(world, t)) {
        panic!("section, cell, type, lane, relax: {first}");
    }
    if let Some(outcome) = &outcome {
        let found = texel_mismatch(world.centre, outcome, &scene.colours, |t| {
            common::oracle(world, t)
        });
        if let Some(first) = found.first {
            panic!("{first}");
        }
    }
    outcome
}

fn lane_at(outcome: &Outcome, t: LightType, pos: BlockPos) -> u8 {
    let (_, levels) = outcome
        .lanes
        .iter()
        .flatten()
        .find(|(lane, _)| *lane == t)
        .expect("the type has a lane");
    let at = common::output_positions(CENTRE)
        .position(|p| p == pos)
        .expect("the position is in the output");
    levels[at]
}

#[test]
fn a_lone_torch_resolves_to_its_colour_and_dark_cells_to_the_default_weight() {
    let world = common::torch_on_a_floor(CENTRE);
    let texels = gpu_matches_relax(&world).expect("a torch lights").texels;
    let levels = common::oracle(&world, TORCH_TYPE);
    let [r, g, b] = common::colour(TORCH_TYPE);
    assert!(levels.contains(&14) && levels.contains(&0));
    for ((pos, texel), level) in common::output_positions(CENTRE)
        .zip(texels.iter())
        .zip(levels)
    {
        let expected = if level > 0 {
            [r, g, b, 0]
        } else {
            [0, 0, 0, 255]
        };
        assert_eq!(*texel, expected, "at {pos} with level {level}");
    }
}

#[test]
fn sixteen_seventeen_and_forty_types_each_match_the_server_rule_on_the_gpu() {
    for count in [16, 17, 40] {
        let outcome = gpu_matches_relax(&common::many_types(CENTRE, count)).expect("lit");
        assert_eq!(outcome.lanes.map(|l| l.len()), Some(count as usize));
    }
}

#[test]
fn an_empty_region_has_no_colour_on_the_gpu() {
    assert!(gpu_matches_relax(&Neighbourhood::air(CENTRE)).is_none());
}

#[test]
fn opaque_emitters_keep_their_own_light_on_the_gpu() {
    let (world, glowstone) = common::glowstone_among_torches(CENTRE);
    let outcome = gpu_matches_relax(&world).expect("lit");
    assert_eq!(lane_at(&outcome, LightType::DEFAULT, glowstone), 15);
    assert_eq!(lane_at(&outcome, TORCH_TYPE, glowstone), 0);
}

#[test]
fn nothing_is_retained_between_gpu_recomputes() {
    let a = common::torch_on_a_floor(CENTRE);
    let b = common::soul_and_lava_by_a_wall(CENTRE);
    let run = |world| gpu_matches_relax(world).expect("lit").texels;
    let (a_first, b_second) = (run(&a), run(&b));
    let (b_first, a_second) = (run(&b), run(&a));
    assert!(a_first == a_second);
    assert!(b_first == b_second);
    assert!(a_first != b_first);
}

#[test]
fn a_block_id_past_the_registry_is_opaque_and_uncoloured_on_the_gpu() {
    let (world, beside) = common::unknown_blocks_by_a_torch(CENTRE);
    let outcome = gpu_matches_relax(&world).expect("the torch still lights");
    assert_eq!(lane_at(&outcome, TORCH_TYPE, beside), 0);
}

mod exhaustive {
    use super::*;

    #[test]
    fn every_gpu_lane_matches_the_server_rule_on_every_scene() {
        for name in ["nether_lava", "caves", "overlap"] {
            let scene = scene(name);
            let mut lit = 0;
            for section in scene.inner() {
                let (min, size) = section_output(section);
                let region = Region::new(min, size, scene.bounds, &scene.registry, scene.cells());
                let types = Palette::of(&region, &scene.registry, &scene.colours).types;
                let mut stages = Stages::default();
                let Some(outcome) = run(&scene, section, &mut stages) else {
                    if let Some(problem) = reach_breach(&[], &types, |t| oracle(&scene, section, t))
                    {
                        panic!("{name}: {section:?} has no outcome, but {problem}");
                    }
                    continue;
                };
                lit += 1;
                let lanes = outcome.lanes.expect("the GPU keeps its lanes");
                let lane_types: Vec<LightType> = lanes.iter().map(|(t, _)| *t).collect();
                if let Some(problem) =
                    reach_breach(&lane_types, &types, |t| oracle(&scene, section, t))
                {
                    panic!("{name}: {section:?}: {problem}");
                }
                if let Some(first) = mismatch(section, &lanes, |t| oracle(&scene, section, t)) {
                    panic!("{name}: section, cell, type, lane, relax: {first}");
                }
                assert!(
                    stages.propagation > Duration::ZERO,
                    "{name}: an untimed pass"
                );
            }
            assert!(lit > 0, "{name} has no section the GPU lights");
        }
    }

    proptest! {
        #[test]
        fn every_gpu_lane_matches_the_server_rule_on_random_regions(parts in common::world_parts()) {
            gpu_matches_relax(&common::random_world(CENTRE, parts));
        }
    }
}
