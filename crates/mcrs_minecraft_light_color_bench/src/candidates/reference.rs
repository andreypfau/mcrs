use std::time::Instant;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::propagate::propagate;
use mcrs_minecraft_light_color::region::{EdgeCosts, Palette, Region, section_output};
use mcrs_minecraft_light_color::resolve::resolve;

use super::{Outcome, Stages, cut};
use crate::fixture::Scene;

pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    *stages = Stages::default();
    let (registry, colours) = (&scene.registry, &scene.colours);
    let (min, size) = section_output(section);

    let clock = Instant::now();
    let region = Region::new(min, size, scene.bounds, registry, scene.cells());
    let palette = Palette::of(&region, registry, colours);
    stages.snapshot = clock.elapsed();
    if palette.types.is_empty() {
        return None;
    }

    let clock = Instant::now();
    let costs = EdgeCosts::new(&region, registry);
    stages.costs = clock.elapsed();

    let clock = Instant::now();
    let lanes = propagate(&region, &costs, &palette, registry, colours);
    stages.propagation = clock.elapsed();

    let clock = Instant::now();
    let texels = resolve(&region, &lanes, &palette, colours, min, size);
    stages.resolve = clock.elapsed();

    let lanes = palette
        .types
        .iter()
        .enumerate()
        .map(|(lane, &t)| (t, cut(&region, min, size, |cell| lanes.level(cell, lane))))
        .collect();
    Some(Outcome {
        texels,
        lanes: Some(lanes),
    })
}
