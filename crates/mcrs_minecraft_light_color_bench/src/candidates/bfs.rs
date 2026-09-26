use std::collections::BTreeMap;
use std::time::Instant;

use mcrs_minecraft_core::{BoundingBox, SectionPos};
use mcrs_minecraft_light::field::{FieldLayout, LightField};
use mcrs_minecraft_light::relax;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::propagate::Lanes;
use mcrs_minecraft_light_color::region::{Palette, Region, section_output};
use mcrs_minecraft_light_color::resolve::resolve;

use super::{Outcome, Stages};
use crate::fixture::{Scene, cell_index, emitters, output_positions, snapshot};

/// The server's own relax, once per light type, over the section and its
/// neighbours.
pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    *stages = Stages::default();
    let (min, size) = section_output(section);

    let clock = Instant::now();
    let layout =
        FieldLayout::covering(BoundingBox::of_section(section).inflated(SectionPos::SIZE as i32));
    let blocks = snapshot(scene, &layout);
    let mut seeds: BTreeMap<LightType, Vec<_>> = BTreeMap::new();
    for (index, t, emission) in emitters(scene, &layout, &blocks) {
        seeds.entry(t).or_default().push((index, emission));
    }
    stages.snapshot = clock.elapsed();
    if seeds.is_empty() {
        return None;
    }

    let clock = Instant::now();
    let cell = cell_index(&layout);
    let output: Vec<u32> = output_positions(section).map(cell).collect();
    let bytes = seeds.len();
    let mut levels = vec![0u8; output.len() * bytes];
    for (lane, sources) in seeds.values().enumerate() {
        let field = LightField::new(layout.clone());
        for &(index, emission) in sources {
            field.set(index, emission);
        }
        relax(
            &field,
            &blocks,
            &scene.registry,
            sources.iter().map(|s| s.0).collect(),
        );
        for (i, &index) in output.iter().enumerate() {
            levels[i * bytes + lane] = field.get(index).get();
        }
    }
    stages.propagation = clock.elapsed();

    let clock = Instant::now();
    let region = Region {
        min,
        size,
        blocks: Box::default(),
    };
    let palette = Palette {
        types: seeds.keys().copied().collect(),
    };
    let lanes = Lanes {
        bytes,
        levels: levels.into_boxed_slice(),
    };
    let texels = resolve(&region, &lanes, &palette, &scene.colours, min, size);
    stages.resolve = clock.elapsed();

    let lanes = palette
        .types
        .iter()
        .enumerate()
        .map(|(lane, &t)| {
            let levels = lanes
                .levels
                .iter()
                .skip(lane)
                .step_by(bytes)
                .copied()
                .collect();
            (t, levels)
        })
        .collect();
    Some(Outcome {
        texels,
        lanes: Some(lanes),
        undetermined: 0,
    })
}
