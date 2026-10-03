//! A structure template painted by a procedure: boxes, walls and roofs laid
//! on a canvas in order, then a rule layer that completes what the neighbours
//! of a block already decide.

mod canvas;
mod data;
mod machines;
mod parts;
pub mod rules;
mod turn;

pub use canvas::{Canvas, Gable};
pub use data::{Fields, Tag, compound};
pub use parts::{
    Coords, Hinge, Patch, Slopes, corner_stairs, fence_joined, top_stairs, wall_joined,
};
pub use turn::turned;

use mcrs_minecraft_core::{Axis, Direction, Mirror, Rotation};
use mcrs_minecraft_worldgen_feature::template::PaletteState;

pub type Pos = (i32, i32, i32);

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    /// The whole block state, as written.
    Exact(PaletteState),
    /// The properties the neighbours cannot decide; the rule layer adds the rest.
    Settled(PaletteState),
    /// Grass, or dirt under a full cube or a fluid.
    Ground,
    /// No block: the cell is absent from the template.
    Void,
}

pub const GROUND: Cell = Cell::Ground;
pub const VOID: Cell = Cell::Void;

fn state(text: &str) -> PaletteState {
    text.parse()
        .unwrap_or_else(|e| panic!("block state `{text}`: {e}"))
}

pub fn block(state_text: &str) -> Cell {
    Cell::Exact(state(state_text))
}

pub fn settled(state_text: &str) -> Cell {
    Cell::Settled(state(state_text))
}

/// Bottom stairs facing `facing`, their shape left to the neighbours.
pub fn stairs(block: &str, facing: Direction) -> Cell {
    Cell::Settled(rules::stairs(block, facing))
}

pub fn log(block: &str, axis: Axis) -> Cell {
    Cell::Exact(rules::with(&state(block), "axis", axis.name()))
}

impl Cell {
    pub fn turned(&self, turn: Turn) -> Cell {
        match self {
            Cell::Exact(state) => Cell::Exact(turned(state, turn)),
            Cell::Settled(state) => Cell::Settled(turned(state, turn)),
            other => other.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Turn {
    pub mirror: Mirror,
    pub rotation: Rotation,
}

impl Turn {
    pub const NONE: Turn = Turn::new(Mirror::None, Rotation::None);
    pub const CLOCKWISE_90: Turn = Turn::new(Mirror::None, Rotation::Clockwise90);
    pub const HALF: Turn = Turn::new(Mirror::None, Rotation::Clockwise180);
    pub const COUNTERCLOCKWISE_90: Turn = Turn::new(Mirror::None, Rotation::Counterclockwise90);
    /// Mirrored across x, then turned.
    pub const MIRRORED: Turn = Turn::new(Mirror::FrontBack, Rotation::None);
    pub const MIRRORED_CLOCKWISE_90: Turn = Turn::new(Mirror::FrontBack, Rotation::Clockwise90);
    pub const MIRRORED_HALF: Turn = Turn::new(Mirror::FrontBack, Rotation::Clockwise180);
    pub const MIRRORED_COUNTERCLOCKWISE_90: Turn =
        Turn::new(Mirror::FrontBack, Rotation::Counterclockwise90);

    pub const fn new(mirror: Mirror, rotation: Rotation) -> Self {
        Turn { mirror, rotation }
    }
}
