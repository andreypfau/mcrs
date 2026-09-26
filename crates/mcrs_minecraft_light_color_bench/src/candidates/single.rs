use std::time::Instant;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::region::{Palette, Region, section_output};

use super::{Outcome, Stages, reference};
use crate::fixture::{Scene, output_positions};

const UNLIT: [u8; 4] = [0, 0, 0, 255];

pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    *stages = Stages::default();
    let (registry, colours) = (&scene.registry, &scene.colours);
    let (min, size) = section_output(section);

    let clock = Instant::now();
    let region = Region::new(min, size, scene.bounds, registry, scene.cells());
    let palette = Palette::of(&region, registry, colours);
    let &[only] = &palette.types[..] else {
        let check = clock.elapsed();
        let outcome = reference::run(scene, section, stages);
        stages.snapshot += check;
        return outcome;
    };
    stages.snapshot = clock.elapsed();

    let clock = Instant::now();
    let lit = colours.rgb(only).map_or(UNLIT, |[r, g, b]| [r, g, b, 0]);
    let levels: Vec<u8> = output_positions(section)
        .map(|pos| scene.server_level(pos))
        .collect();
    let texels = levels
        .iter()
        .map(|&level| if level > 0 { lit } else { UNLIT })
        .collect();
    stages.resolve = clock.elapsed();

    Some(Outcome {
        texels,
        lanes: Some(vec![(only, levels)]),
        undetermined: 0,
    })
}
