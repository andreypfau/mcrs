use std::time::Instant;

use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::propagate::propagate;
use mcrs_minecraft_light_color::region::{EdgeCosts, Palette, Region, section_output};
use mcrs_minecraft_light_color::resolve::resolve;

use super::{Outcome, Stages, cut};
use crate::fixture::Scene;

pub const BLOCK_OUTPUT: i32 = 3 * SectionPos::SIZE as i32 + 2;

/// The reference kernel over one region for the scene's whole inner 3×3×3
/// block, charged to each of its 27 sections at a 27th of the cost.
pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    *stages = Stages::default();
    let (registry, colours) = (&scene.registry, &scene.colours);
    let inner = scene.inner_min();
    let min = BlockPos::new(inner.x - 1, inner.y - 1, inner.z - 1);
    let size = BLOCK_OUTPUT;

    let clock = Instant::now();
    let region = Region::new(min, size, scene.bounds, registry, scene.cells());
    let palette = Palette::of(&region, registry, colours);
    stages.snapshot = clock.elapsed() / 27;
    if palette.types.is_empty() {
        return None;
    }

    let clock = Instant::now();
    let costs = EdgeCosts::new(&region, registry);
    stages.costs = clock.elapsed() / 27;

    let clock = Instant::now();
    let lanes = propagate(&region, &costs, &palette, registry, colours);
    stages.propagation = clock.elapsed() / 27;

    let clock = Instant::now();
    let texels = resolve(&region, &lanes, &palette, colours, min, size);
    stages.resolve = clock.elapsed() / 27;

    let (section_min, section_size) = section_output(section);
    let offset = section_min - min.as_ivec3();
    assert!(
        offset.min_element() >= 0 && offset.max_element() + section_size <= size,
        "section {section:?} is not in the scene's inner block"
    );
    let mut section_texels = Vec::with_capacity((section_size.pow(3)) as usize);
    for y in 0..section_size {
        for z in 0..section_size {
            for x in 0..section_size {
                let i = (offset.x + x) + size * ((offset.z + z) + size * (offset.y + y));
                section_texels.push(texels[i as usize]);
            }
        }
    }
    let lanes = palette
        .types
        .iter()
        .enumerate()
        .map(|(lane, &t)| {
            let levels = cut(&region, section_min, section_size, |cell| {
                lanes.level(cell, lane)
            });
            (t, levels)
        })
        .collect();
    Some(Outcome {
        texels: section_texels.into_boxed_slice(),
        lanes: Some(lanes),
    })
}
