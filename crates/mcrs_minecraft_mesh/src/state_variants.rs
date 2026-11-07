use std::slice;

use crate::block::{BlockInfo, CubeFace, ModelQuad};
use crate::variant_pick::VariantPick;

/// The geometry of a state whose parts have more than one variant, and how a cell chooses.
#[derive(Clone, Debug)]
pub struct StateVariants {
    pub pick: VariantPick,
    pub shapes: VariantShapes,
}

#[derive(Clone, Debug)]
pub enum VariantShapes {
    /// The geometry of each variant of the one part, after the first: the first is the
    /// geometry of the [`BlockInfo`] itself.
    Whole(Vec<Shape>),
    /// For each part, in the order of the pick, the quads of each of its variants. The
    /// [`BlockInfo`] itself holds the geometry of the parts with one variant.
    Parts(Vec<Vec<Vec<ModelQuad>>>),
}

#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub cube: Option<[CubeFace; 6]>,
    pub drop: u8,
    pub quads: Vec<ModelQuad>,
}

impl BlockInfo {
    /// The choice of the cell at the world block position `position`.
    #[must_use]
    pub fn choice_at(&self, position: [i32; 3]) -> u32 {
        self.variants
            .as_ref()
            .map_or(0, |variants| variants.pick.choice(position))
    }

    /// The cube faces and the drop that a cell with this choice draws.
    #[must_use]
    pub fn cube_of(&self, choice: u32) -> (Option<&[CubeFace; 6]>, u8) {
        match self.chosen_shape(choice) {
            Some(shape) => (shape.cube.as_ref(), shape.drop),
            None => (self.cube.as_ref(), self.drop),
        }
    }

    /// The model quads that a cell with this choice draws.
    #[must_use]
    pub fn quads_of(&self, choice: u32) -> CellQuads<'_> {
        if let Some(shape) = self.chosen_shape(choice) {
            return CellQuads {
                current: shape.quads.iter(),
                parts: &[],
                weights: &[],
                rest: 0,
            };
        }
        let (parts, weights): (&[Vec<Vec<ModelQuad>>], &[Vec<u32>]) = match &self.variants {
            Some(variants) => match &variants.shapes {
                VariantShapes::Parts(parts) => (parts, &variants.pick.weights),
                VariantShapes::Whole(_) => (&[], &[]),
            },
            None => (&[], &[]),
        };
        CellQuads {
            current: self.quads.iter(),
            parts,
            weights,
            rest: choice,
        }
    }

    fn chosen_shape(&self, choice: u32) -> Option<&Shape> {
        let VariantShapes::Whole(shapes) = &self.variants.as_ref()?.shapes else {
            return None;
        };
        shapes.get(usize::try_from(choice.checked_sub(1)?).ok()?)
    }
}

/// The quads of a cell: those of the block, then those of the chosen variant of each part.
pub struct CellQuads<'a> {
    current: slice::Iter<'a, ModelQuad>,
    parts: &'a [Vec<Vec<ModelQuad>>],
    weights: &'a [Vec<u32>],
    rest: u32,
}

impl<'a> Iterator for CellQuads<'a> {
    type Item = &'a ModelQuad;

    fn next(&mut self) -> Option<&'a ModelQuad> {
        loop {
            if let Some(quad) = self.current.next() {
                return Some(quad);
            }
            let (part, parts) = self.parts.split_first()?;
            let size = self
                .weights
                .first()
                .map_or(1, |weights| weights.len().max(1) as u32);
            self.weights = self.weights.get(1..).unwrap_or(&[]);
            self.parts = parts;
            let index = self.rest % size;
            self.rest /= size;
            self.current = part
                .get(index as usize)
                .map_or([].iter(), |quads| quads.iter());
        }
    }
}
