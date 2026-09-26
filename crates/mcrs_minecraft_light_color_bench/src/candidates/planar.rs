use std::time::Instant;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::propagate::Lanes;
use mcrs_minecraft_light_color::region::{
    EAST_FACE, EdgeCosts, Palette, Region, SOUTH_FACE, UP_FACE, section_output,
};
use mcrs_minecraft_light_color::resolve::resolve;

use super::{Outcome, Stages, cut};
use crate::fixture::Scene;

/// Per cell, whether light may arrive from each neighbour, as `0xff` or `0`,
/// so a wave masks instead of branching. Cells on the region's edge are closed
/// towards the outside.
struct Open {
    from_west: Vec<u8>,
    from_east: Vec<u8>,
    from_north: Vec<u8>,
    from_south: Vec<u8>,
    from_below: Vec<u8>,
    from_above: Vec<u8>,
}

impl Open {
    fn new(region: &Region, costs: &EdgeCosts) -> Open {
        let size = region.size as usize;
        let (row, layer) = (size, size * size);
        let n = region.cell_count();
        let open = |closed: bool| if closed { 0 } else { 0xff };
        let mut planes = Open {
            from_west: vec![0; n],
            from_east: vec![0; n],
            from_north: vec![0; n],
            from_south: vec![0; n],
            from_below: vec![0; n],
            from_above: vec![0; n],
        };
        let veto = &costs.veto;
        for y in 0..size {
            for z in 0..size {
                for x in 0..size {
                    let c = x + row * z + layer * y;
                    if x > 0 {
                        planes.from_west[c] = open(veto[c - 1] & EAST_FACE != 0);
                    }
                    if x + 1 < size {
                        planes.from_east[c] = open(veto[c] & EAST_FACE != 0);
                    }
                    if z > 0 {
                        planes.from_north[c] = open(veto[c - row] & SOUTH_FACE != 0);
                    }
                    if z + 1 < size {
                        planes.from_south[c] = open(veto[c] & SOUTH_FACE != 0);
                    }
                    if y > 0 {
                        planes.from_below[c] = open(veto[c - layer] & UP_FACE != 0);
                    }
                    if y + 1 < size {
                        planes.from_above[c] = open(veto[c] & UP_FACE != 0);
                    }
                }
            }
        }
        planes
    }
}

/// One byte plane per light type, padded by a layer on each side so every
/// neighbour read is in bounds; the masks keep the padding out.
fn relax_plane(plane: &mut Vec<u8>, spare: &mut Vec<u8>, entry: &[u8], open: &Open, size: usize) {
    let n = entry.len();
    let (row, layer) = (size, size * size);
    let pad = layer;
    loop {
        let cur = &plane[..];
        let here = &cur[pad..pad + n];
        let west = &cur[pad - 1..pad - 1 + n];
        let east = &cur[pad + 1..pad + 1 + n];
        let north = &cur[pad - row..pad - row + n];
        let south = &cur[pad + row..pad + row + n];
        let below = &cur[pad - layer..pad - layer + n];
        let above = &cur[pad + layer..pad + layer + n];
        let next = &mut spare[pad..pad + n];
        let (from_west, from_east) = (&open.from_west[..n], &open.from_east[..n]);
        let (from_north, from_south) = (&open.from_north[..n], &open.from_south[..n]);
        let (from_below, from_above) = (&open.from_below[..n], &open.from_above[..n]);
        let entry = &entry[..n];
        let mut changed = 0u8;
        for i in 0..n {
            let incoming = (west[i] & from_west[i])
                .max(east[i] & from_east[i])
                .max(north[i] & from_north[i])
                .max(south[i] & from_south[i])
                .max(below[i] & from_below[i])
                .max(above[i] & from_above[i]);
            let level = here[i].max(incoming.saturating_sub(entry[i]));
            changed |= level ^ here[i];
            next[i] = level;
        }
        std::mem::swap(plane, spare);
        if changed == 0 {
            return;
        }
    }
}

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
    let open = Open::new(&region, &costs);
    stages.costs = clock.elapsed();

    let clock = Instant::now();
    let side = region.size as usize;
    let n = region.cell_count();
    let pad = side * side;
    let mut planes = vec![vec![0u8; n + 2 * pad]; palette.types.len()];
    for (cell, &id) in region.blocks.iter().enumerate() {
        let emission = registry.emission(id);
        if !emission.is_zero()
            && let Some(lane) = palette.lane(colours.light_type(id))
        {
            planes[lane][pad + cell] = emission.get();
        }
    }
    let mut spare = vec![0u8; n + 2 * pad];
    for plane in &mut planes {
        relax_plane(plane, &mut spare, &costs.entry, &open, side);
    }
    stages.propagation = clock.elapsed();

    let clock = Instant::now();
    let lanes: Vec<Vec<u8>> = planes
        .iter()
        .map(|plane| cut(&region, min, size, |cell| plane[pad + cell]))
        .collect();
    let bytes = lanes.len();
    let mut levels = vec![0u8; lanes[0].len() * bytes];
    for (lane, plane) in lanes.iter().enumerate() {
        for (i, &level) in plane.iter().enumerate() {
            levels[i * bytes + lane] = level;
        }
    }
    let output = Region {
        min,
        size,
        blocks: Box::default(),
    };
    let texels = resolve(
        &output,
        &Lanes {
            bytes,
            levels: levels.into_boxed_slice(),
        },
        &palette,
        colours,
        min,
        size,
    );
    stages.resolve = clock.elapsed();

    Some(Outcome {
        texels,
        lanes: Some(palette.types.iter().copied().zip(lanes).collect()),
        undetermined: 0,
    })
}
