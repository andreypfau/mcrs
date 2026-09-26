use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::level::LightBounds;

use crate::colors::LightColors;
use crate::region::{
    EAST_FACE, EdgeCosts, Palette, Region, SOUTH_FACE, UP_FACE, lane_bytes, section_output,
};
use crate::resolve::resolve;

/// Light per cell and palette lane: cell `c`, lane `l` is `levels[c * bytes + l]`.
pub struct Lanes {
    pub bytes: usize,
    pub levels: Box<[u8]>,
}

impl Lanes {
    #[inline]
    pub fn level(&self, cell: usize, lane: usize) -> u8 {
        self.levels[cell * self.bytes + lane]
    }
}

/// Light of every palette type in the region. A cell whose type the palette
/// lacks is not a source.
pub fn propagate(
    region: &Region,
    costs: &EdgeCosts,
    palette: &Palette,
    registry: &LightRegistry,
    colours: &LightColors,
) -> Lanes {
    let bytes = lane_bytes(palette.types.len());
    let levels = match bytes {
        0 => Box::default(),
        1 => words::<1>(region, costs, palette, registry, colours),
        2 => words::<2>(region, costs, palette, registry, colours),
        4 => words::<4>(region, costs, palette, registry, colours),
        _ => words::<8>(region, costs, palette, registry, colours),
    };
    Lanes { bytes, levels }
}

fn words<const N: usize>(
    region: &Region,
    costs: &EdgeCosts,
    palette: &Palette,
    registry: &LightRegistry,
    colours: &LightColors,
) -> Box<[u8]> {
    let words = lane_bytes(palette.types.len()) / N;
    let mut levels = vec![[0u8; N]; region.cell_count() * words];
    seed(&mut levels, words, region, palette, registry, colours);
    for word in 0..words {
        relax_word(&mut levels, words, word, region, costs);
    }
    levels.into_flattened().into_boxed_slice()
}

fn seed<const N: usize>(
    levels: &mut [[u8; N]],
    words: usize,
    region: &Region,
    palette: &Palette,
    registry: &LightRegistry,
    colours: &LightColors,
) {
    for (cell, &id) in region.blocks.iter().enumerate() {
        let emission = registry.emission(id);
        if emission.is_zero() {
            continue;
        }
        if let Some(lane) = palette.lane(colours.light_type(id)) {
            levels[cell * words + lane / N][lane % N] = emission.get();
        }
    }
}

#[inline]
fn gather<const N: usize>(incoming: &mut [u8; N], neighbour: &[u8; N]) {
    for (a, &b) in incoming.iter_mut().zip(neighbour) {
        *a = (*a).max(b);
    }
}

/// Raises one word of every cell until a whole sweep changes nothing. Values
/// only rise, so updating in place reaches the same least fixed point as
/// separate waves, in fewer sweeps.
fn relax_word<const N: usize>(
    levels: &mut [[u8; N]],
    words: usize,
    word: usize,
    region: &Region,
    costs: &EdgeCosts,
) {
    let size = region.size as usize;
    let (row, layer) = (size, size * size);
    let at = |cell: usize| cell * words + word;
    loop {
        let mut changed = false;
        for y in 0..size {
            for z in 0..size {
                for x in 0..size {
                    let cell = x + row * z + layer * y;
                    let veto = &costs.veto;
                    let mut incoming = [0u8; N];
                    if x > 0 && veto[cell - 1] & EAST_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell - 1)]);
                    }
                    if x + 1 < size && veto[cell] & EAST_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell + 1)]);
                    }
                    if z > 0 && veto[cell - row] & SOUTH_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell - row)]);
                    }
                    if z + 1 < size && veto[cell] & SOUTH_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell + row)]);
                    }
                    if y > 0 && veto[cell - layer] & UP_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell - layer)]);
                    }
                    if y + 1 < size && veto[cell] & UP_FACE == 0 {
                        gather(&mut incoming, &levels[at(cell + layer)]);
                    }

                    let entry = costs.entry[cell];
                    let current = &mut levels[at(cell)];
                    let mut raised = false;
                    for (level, &light) in current.iter_mut().zip(&incoming) {
                        let arriving = light.saturating_sub(entry);
                        raised |= arriving > *level;
                        *level = (*level).max(arriving);
                    }
                    changed |= raised;
                }
            }
        }
        if !changed {
            return;
        }
    }
}

/// Resolved colour of a section and its one-block apron, or `None` when no
/// light source is within reach.
pub fn colour_section<'a>(
    section: SectionPos,
    bounds: LightBounds,
    registry: &LightRegistry,
    colours: &LightColors,
    cells: impl Fn(SectionPos) -> Option<&'a [u16; SectionPos::VOLUME]>,
) -> Option<Box<[[u8; 4]]>> {
    let (output_min, output_size) = section_output(section);
    let region = Region::new(output_min, output_size, bounds, registry, cells);
    let palette = Palette::of(&region, registry, colours);
    if palette.types.is_empty() {
        return None;
    }
    let costs = EdgeCosts::new(&region, registry);
    let lanes = propagate(&region, &costs, &palette, registry, colours);
    Some(resolve(
        &region,
        &lanes,
        &palette,
        colours,
        output_min,
        output_size,
    ))
}
