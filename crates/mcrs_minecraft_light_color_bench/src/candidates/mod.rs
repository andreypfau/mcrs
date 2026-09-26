use std::time::Duration;

use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::Region;

use crate::fixture::{Scene, output_positions};

pub mod bfs;
pub mod block;
pub mod gpu;
pub mod gradient;
pub mod hybrid;
pub mod planar;
pub mod reference;
pub mod single;

#[derive(Clone, Copy, Debug, Default)]
pub struct Stages {
    pub snapshot: Duration,
    pub costs: Duration,
    pub propagation: Duration,
    pub resolve: Duration,
    /// Device buffers the run holds, for a candidate that runs on the GPU.
    pub device_memory: usize,
    /// CPU wall clock a GPU run spends creating its buffers, submitting, and
    /// waiting for its readback.
    pub round_trip: Duration,
}

impl Stages {
    pub fn total(&self) -> Duration {
        self.snapshot + self.costs + self.propagation + self.resolve
    }
}

/// A section's 18³ output, x fastest then z then y, and the level of each
/// light type there when the candidate computes one.
pub struct Outcome {
    pub texels: Box<[[u8; 4]]>,
    pub lanes: Option<Vec<(LightType, Vec<u8>)>>,
    /// Lit output cells whose colour the candidate could not tell and guessed.
    pub undetermined: usize,
}

pub struct Candidate {
    pub name: &'static str,
    /// Whether every lane is claimed to equal the server's relax per type.
    pub exact: bool,
    pub note: &'static str,
    /// Cells propagated per output cell.
    pub redundancy: f64,
    /// `None` for a section no light source reaches.
    pub run: fn(&Scene, SectionPos, &mut Stages) -> Option<Outcome>,
}

pub const CANDIDATES: &[Candidate] = &[
    Candidate {
        name: "reference",
        exact: true,
        note: "Exact by construction: one byte lane per light type, relaxed over the whole \
               region until nothing changes. Reads only the blocks.",
        redundancy: cube(46.0) / cube(18.0),
        run: reference::run,
    },
    Candidate {
        name: "bfs",
        exact: true,
        note: "Exact: the server's relax, once per light type. Reads only the blocks.",
        redundancy: cube(48.0) / cube(18.0),
        run: bfs::run,
    },
    Candidate {
        name: "planar",
        exact: true,
        note: "Exact: the reference's waves over one byte plane per light type. Reads only the \
               blocks.",
        redundancy: cube(46.0) / cube(18.0),
        run: planar::run,
    },
    Candidate {
        name: "block",
        exact: true,
        note: "Exact: the reference kernel over one region per 3×3×3 block of sections, charged \
               a 27th to each. Reads only the blocks, but needs the whole block and its ring \
               present, so colour work is scheduled in blocks. Its redundancy is 78³ over 50³ = \
               3.80×, not the about 2× estimated before measuring.",
        redundancy: cube(78.0) / cube(50.0),
        run: block::run,
    },
    Candidate {
        name: "gradient_strict",
        exact: false,
        note: "Approximate: colours each cell from the server's light levels, as the mean of \
               the brighter neighbours whose light arrives at exactly its level, and of its own \
               colour when it emits at that level. It reads the server's light levels, so \
               colour must be recomputed whenever the light changes. That reverses the rule of \
               recolouring only when a chunk arrives or a block changes, and changes when \
               colour work is scheduled. It never disagrees with the server's levels, which \
               per-type propagation can at unloaded borders. Borders between colours are \
               sharp, because only ties blend. Undetermined cells are lit cells with no such neighbour, left at the \
               default tint.",
        redundancy: cube(46.0) / cube(18.0),
        run: gradient::strict,
    },
    Candidate {
        name: "gradient_weighted",
        exact: false,
        note: "Approximate: the same pass over the server's light levels, but every brighter \
               neighbour counts, weighted by the light curve at the level its light arrives \
               with, and a cell that emits counts its own colour weighted by its emission. \
               Softer than strict parents, still approximate. It reads the server's light \
               levels, so colour must be recomputed whenever the light changes, reversing the \
               rule of recolouring only when a chunk arrives or a block changes. Undetermined \
               cells are lit cells with no brighter neighbour and no emission, left at the \
               default tint.",
        redundancy: cube(46.0) / cube(18.0),
        run: gradient::weighted,
    },
    Candidate {
        name: "hybrid",
        exact: false,
        note: "Approximate: propagates every light type except the one with the most emitters \
               in the region, and gives that type the server's level wherever the level \
               strictly exceeds every other lane. The level is the maximum over types but does \
               not say which type attains it, so the dominant lane is exact only where it \
               strictly wins and is set to 0 elsewhere. It reads the server's light levels, so \
               it shares the gradients' recolour consequence. Undetermined cells are lit cells \
               whose level does not exceed another lane.",
        redundancy: cube(46.0) / cube(18.0),
        run: hybrid::run,
    },
    Candidate {
        name: "single",
        exact: true,
        note: "Exact where it applies: a section whose region holds one light type skips \
               propagation and takes that type's colour wherever the server's level is above \
               0, with the level as its lane. It reads the server's light levels as that mask. \
               Every other section falls through to the reference, so the row times the \
               combined cost. It composes with every other candidate.",
        redundancy: cube(46.0) / cube(18.0),
        run: single::run,
    },
    Candidate {
        name: "gpu",
        exact: true,
        note: "Exact: the reference's waves on the GPU, four byte lanes to a 32-bit word. Each \
               section's 16³ brick holds, per cell, the entry cost and the three face vetoes \
               from the same edge costs as the reference, and the emission and light type. A \
               gather dispatch assembles a section's 46³ region from the 27 bricks around it, \
               15 wave dispatches propagate it, and a cut dispatch leaves the 18³ output, which \
               is read back and resolved on the CPU. Snapshot is the CPU brick build for the \
               scene charged a 125th per section, plus the section's palette; costs are part of \
               the bricks; propagation is GPU time from timestamp queries around the one compute \
               pass. Peak memory is the device buffers one section holds, its brick included. \
               Reads only the blocks. The production form would live in the deferred renderer; \
               the browser's cost is not measured here.",
        redundancy: cube(46.0) / cube(18.0),
        run: gpu::run,
    },
];

const fn cube(side: f64) -> f64 {
    side * side * side
}

/// The 18³ output of one region lane, read through `level(cell)`.
pub fn cut(
    region: &Region,
    output_min: BlockPos,
    output_size: i32,
    level: impl Fn(usize) -> u8,
) -> Vec<u8> {
    let offset = output_min - region.min.as_ivec3();
    let mut out = Vec::with_capacity((output_size * output_size * output_size) as usize);
    for y in 0..output_size {
        for z in 0..output_size {
            for x in 0..output_size {
                out.push(level(region.index(
                    offset.x + x,
                    offset.y + y,
                    offset.z + z,
                )));
            }
        }
    }
    out
}

/// The first output cell where a lane differs from the server's rule, as
/// `<section> <cell> <type> <got> <want>`.
pub fn mismatch(
    section: SectionPos,
    got: &[(LightType, Vec<u8>)],
    want: impl Fn(LightType) -> Vec<u8>,
) -> Option<String> {
    got.iter().find_map(|(t, levels)| {
        let want = want(*t);
        output_positions(section)
            .zip(levels.iter().zip(&want))
            .find(|(_, (got, want))| got != want)
            .map(|(pos, (got, want))| {
                format!(
                    "{},{},{} {},{},{} {} {got} {want}",
                    section.x, section.y, section.z, pos.x, pos.y, pos.z, t.0
                )
            })
    })
}
