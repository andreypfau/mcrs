use std::time::Instant;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::propagate::{Lanes, propagate};
use mcrs_minecraft_light_color::region::{EdgeCosts, Palette, REACH, Region, section_output};
use mcrs_minecraft_light_color::resolve::resolve;

use super::{Outcome, Stages};
use crate::fixture::{Scene, output_positions};

pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    *stages = Stages::default();
    let (registry, colours) = (&scene.registry, &scene.colours);
    let (min, size) = section_output(section);

    let clock = Instant::now();
    let region = Region::new(min, size, scene.bounds, registry, scene.cells());
    let palette = Palette::of(&region, registry, colours);
    let mut emitters = [0usize; 256];
    for &id in &region.blocks {
        if !registry.emission(id).is_zero() {
            emitters[colours.light_type(id).0 as usize] += 1;
        }
    }
    let server: Vec<u8> = output_positions(section)
        .map(|pos| scene.server_level(pos))
        .collect();
    stages.snapshot = clock.elapsed();
    let &dominant = palette
        .types
        .iter()
        .max_by_key(|t| emitters[t.0 as usize])?;

    let clock = Instant::now();
    let costs = EdgeCosts::new(&region, registry);
    stages.costs = clock.elapsed();

    let clock = Instant::now();
    let others = Palette {
        types: palette
            .types
            .iter()
            .copied()
            .filter(|&t| t != dominant)
            .collect(),
    };
    let propagated = propagate(&region, &costs, &others, registry, colours);
    stages.propagation = clock.elapsed();

    let clock = Instant::now();
    let bytes = palette.types.len();
    let into: Vec<usize> = others
        .types
        .iter()
        .map(|&t| palette.lane(t).unwrap())
        .collect();
    let d = palette.lane(dominant).unwrap();
    let mut levels = vec![0u8; server.len() * bytes];
    let mut undetermined = 0;
    let mut i = 0;
    for y in 0..size {
        for z in 0..size {
            for x in 0..size {
                let cell = region.index(REACH + x, REACH + y, REACH + z);
                let mut strongest = 0;
                for (lane, &to) in into.iter().enumerate() {
                    let level = propagated.level(cell, lane);
                    strongest = strongest.max(level);
                    levels[i * bytes + to] = level;
                }
                if server[i] > strongest {
                    levels[i * bytes + d] = server[i];
                } else if server[i] > 0 {
                    undetermined += 1;
                }
                i += 1;
            }
        }
    }
    let output = Region {
        min,
        size,
        blocks: Box::default(),
    };
    let lanes = Lanes {
        bytes,
        levels: levels.into_boxed_slice(),
    };
    let texels = resolve(&output, &lanes, &palette, colours, min, size);
    stages.resolve = clock.elapsed();

    let lanes = palette
        .types
        .iter()
        .enumerate()
        .map(|(lane, &t)| {
            let levels = lanes.levels.iter().skip(lane).step_by(bytes).copied();
            (t, levels.collect())
        })
        .collect();
    Some(Outcome {
        texels,
        lanes: Some(lanes),
        undetermined,
    })
}
