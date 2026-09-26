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
    note: "Exact: the deferred renderer's colour kernel, waves over byte lanes on the GPU, four \
           lanes to a 32-bit word. Each section's 16³ brick holds, per cell, the entry cost and \
           the three face vetoes from the region's edge costs, and the emission and light type. \
           A section that is not loaded or lies above the world has no brick and reads as a \
           sentinel slot; one below the world has a brick of its own. A gather dispatch assembles \
           a section's 46³ region from the 27 slots around it, 15 wave dispatches propagate it, \
           and a resolve dispatch mixes the lanes of the 18³ output into Rgba8Unorm texels in a \
           3D atlas slot, with no CPU resolve. The palette holds only the light types whose \
           emitters can reach the output. Snapshot is the CPU brick build for the scene charged \
           a 125th per section, plus the section's palette and job record; costs are part of the \
           bricks; propagation is GPU time from timestamp queries around the one compute pass, \
           resolve included. Peak memory is the device buffers and atlas one section holds, its \
           brick included. Reads only the blocks. The browser's cost is not measured here.",
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
