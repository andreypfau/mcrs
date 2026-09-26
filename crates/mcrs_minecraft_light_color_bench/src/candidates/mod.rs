use std::time::Duration;

use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::Region;

use crate::fixture::{Scene, output_positions};

pub mod bfs;
pub mod block;
pub mod gradient;
pub mod planar;
pub mod reference;

#[derive(Clone, Copy, Debug, Default)]
pub struct Stages {
    pub snapshot: Duration,
    pub costs: Duration,
    pub propagation: Duration,
    pub resolve: Duration,
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
               present, so colour work is scheduled in blocks.",
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
               colour work is scheduled. Borders between colours are sharp, because only ties \
               blend. Undetermined cells are lit cells with no such neighbour, left at the \
               default tint.",
        redundancy: cube(46.0) / cube(18.0),
        run: gradient::strict,
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
