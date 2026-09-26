use std::time::Duration;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::colors::LightType;

use crate::fixture::{Scene, output_positions};

pub mod gpu;

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
}

pub struct Candidate {
    pub name: &'static str,
    pub note: &'static str,
    /// Cells propagated per output cell.
    pub redundancy: f64,
    /// `None` for a section no light source reaches.
    pub run: fn(&Scene, SectionPos, &mut Stages) -> Option<Outcome>,
}

pub const CANDIDATES: &[Candidate] = &[Candidate {
    name: "gpu",
    note: "Exact: waves over byte lanes on the GPU, four lanes to a 32-bit word. Each section's \
           16³ brick holds, per cell, the entry cost and the three face vetoes from the region's \
           edge costs, and the emission and light type. A gather dispatch assembles a section's \
           46³ region from the 27 bricks around it, 15 wave dispatches propagate it, and a cut \
           dispatch leaves the 18³ output, which is read back and resolved on the CPU. Snapshot \
           is the CPU brick build for the scene charged a 125th per section, plus the section's \
           palette; costs are part of the bricks; propagation is GPU time from timestamp queries \
           around the one compute pass. Peak memory is the device buffers one section holds, its \
           brick included. Reads only the blocks. The production form would live in the deferred \
           renderer; the browser's cost is not measured here.",
    redundancy: cube(46.0) / cube(18.0),
    run: gpu::run,
}];

const fn cube(side: f64) -> f64 {
    side * side * side
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
