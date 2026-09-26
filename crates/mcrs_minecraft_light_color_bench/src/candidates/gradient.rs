use std::time::Instant;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::region::{
    EAST_FACE, EdgeCosts, REACH, Region, SOUTH_FACE, UP_FACE, section_output,
};
use mcrs_minecraft_light_color::resolve::light_weight;

use super::{Outcome, Stages};
use crate::fixture::Scene;

/// Known RGB and the default type's share, both on a 0 to 255 scale.
type Colour = [f32; 4];

const UNLIT: Colour = [0.0, 0.0, 0.0, 255.0];

pub fn strict(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    run(scene, section, stages, |arriving, here| {
        if arriving == here { 1.0 } else { 0.0 }
    })
}

pub fn weighted(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    run(scene, section, stages, |arriving, _| light_weight(arriving))
}

/// `weight(arriving, here)` is how much a colour arriving at level `arriving`
/// counts in a cell whose server level is `here`.
fn run(
    scene: &Scene,
    section: SectionPos,
    stages: &mut Stages,
    weight: fn(u8, u8) -> f32,
) -> Option<Outcome> {
    *stages = Stages::default();
    let (registry, colours) = (&scene.registry, &scene.colours);
    let (min, size) = section_output(section);

    let clock = Instant::now();
    let region = Region::new(min, size, scene.bounds, registry, scene.cells());
    let side = region.size;
    let mut level = vec![0u8; region.cell_count()];
    for y in 0..side {
        for z in 0..side {
            for x in 0..side {
                let pos = BlockPos::new(region.min.x + x, region.min.y + y, region.min.z + z);
                level[region.index(x, y, z)] = scene.server_level(pos);
            }
        }
    }
    stages.snapshot = clock.elapsed();

    let clock = Instant::now();
    let costs = EdgeCosts::new(&region, registry);
    stages.costs = clock.elapsed();

    let clock = Instant::now();
    let mut buckets: [Vec<u32>; 16] = Default::default();
    for (cell, &l) in level.iter().enumerate() {
        if l > 0 {
            buckets[l as usize].push(cell as u32);
        }
    }
    let mut colour = vec![UNLIT; region.cell_count()];
    let mut orphan = vec![false; region.cell_count()];
    for bucket in buckets.iter().rev() {
        for &cell in bucket {
            let v = cell as usize;
            let here = level[v];
            let (mut sum, mut total) = ([0f32; 4], 0f32);
            let mut add = |c: Colour, w: f32| {
                for (s, c) in sum.iter_mut().zip(c) {
                    *s += c * w;
                }
                total += w;
            };
            let id = region.blocks[v];
            let emission = registry.emission(id).get();
            if emission > 0 {
                add(own(colours, id), weight(emission, here));
            }
            for n in open_neighbours(v, side as usize, &costs.veto)
                .into_iter()
                .flatten()
            {
                if level[n] > here {
                    add(
                        colour[n],
                        weight(level[n].saturating_sub(costs.entry[v]), here),
                    );
                }
            }
            if total > 0.0 {
                colour[v] = sum.map(|s| s / total);
            } else {
                orphan[v] = true;
            }
        }
    }
    stages.propagation = clock.elapsed();

    let clock = Instant::now();
    let mut texels = Vec::with_capacity((size * size * size) as usize);
    let mut undetermined = 0;
    for y in 0..size {
        for z in 0..size {
            for x in 0..size {
                let cell = region.index(REACH + x, REACH + y, REACH + z);
                undetermined += orphan[cell] as usize;
                texels.push(colour[cell].map(|c| c.round() as u8));
            }
        }
    }
    stages.resolve = clock.elapsed();

    Some(Outcome {
        texels: texels.into_boxed_slice(),
        lanes: None,
        undetermined,
    })
}

fn own(colours: &LightColors, id: VoxelId) -> Colour {
    colours
        .rgb(colours.light_type(id))
        .map_or(UNLIT, |[r, g, b]| [r.into(), g.into(), b.into(), 0.0])
}

/// The six neighbours of `cell` inside the region whose shared face light may
/// cross.
fn open_neighbours(cell: usize, side: usize, veto: &[u8]) -> [Option<usize>; 6] {
    let (row, layer) = (side, side * side);
    let (x, z, y) = (cell % row, cell / row % side, cell / layer);
    let open = |low: usize, face: u8| veto[low] & face == 0;
    [
        (x > 0 && open(cell - 1, EAST_FACE)).then(|| cell - 1),
        (x + 1 < side && open(cell, EAST_FACE)).then_some(cell + 1),
        (z > 0 && open(cell - row, SOUTH_FACE)).then(|| cell - row),
        (z + 1 < side && open(cell, SOUTH_FACE)).then_some(cell + row),
        (y > 0 && open(cell - layer, UP_FACE)).then(|| cell - layer),
        (y + 1 < side && open(cell, UP_FACE)).then_some(cell + layer),
    ]
}
