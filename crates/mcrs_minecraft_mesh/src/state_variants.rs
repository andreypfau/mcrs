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

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use mcrs_minecraft_core::Direction;

    use super::*;
    use crate::block::Pass;
    use crate::face_shade::FaceShade;
    use crate::tint::Tint;
    use crate::variant_pick::PartSource;

    fn quad(sprite: u16) -> ModelQuad {
        ModelQuad {
            positions: [Vec3::ZERO; 4],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: Direction::Up,
            face: None,
            sprite,
            pass: Pass::Solid,
            shade: FaceShade::Factor(1.0),
            tint: Tint::None,
            emission: 0,
        }
    }

    fn two_parts() -> BlockInfo {
        let sizes = [2usize, 3];
        let parts = sizes
            .iter()
            .enumerate()
            .map(|(part, size)| {
                (0..*size)
                    .map(|variant| vec![quad((part * 10 + variant) as u16)])
                    .collect()
            })
            .collect();
        BlockInfo {
            variants: Some(Box::new(StateVariants {
                pick: VariantPick {
                    seed_offset: [0, 0, 0],
                    source: PartSource::Multipart,
                    weights: sizes.iter().map(|size| vec![1; *size]).collect(),
                },
                shapes: VariantShapes::Parts(parts),
            })),
            ..BlockInfo::default()
        }
    }

    #[test]
    fn a_choice_reads_each_part_as_a_digit_of_its_variant_count() {
        let info = two_parts();
        let pick = &info.variants.as_ref().unwrap().pick;
        assert_eq!(pick.choices(), Some(6));
        for choice in 0..6u32 {
            let sprites: Vec<u16> = info.quads_of(choice).map(|quad| quad.sprite).collect();
            let (first, second) = (choice % 2, choice / 2 % 3);
            assert_eq!(
                sprites,
                [first as u16, 10 + second as u16],
                "choice {choice}"
            );
        }
    }

    #[test]
    fn the_choice_of_a_position_packs_the_chosen_variants_in_the_same_radix() {
        let info = two_parts();
        let pick = &info.variants.as_ref().unwrap().pick;
        for x in -3..4 {
            for z in -3..4 {
                let position = [x, 64, z];
                let chosen = pick.chosen(position);
                assert_eq!(
                    pick.choice(position),
                    (chosen[0] + 2 * chosen[1]) as u32,
                    "{position:?}"
                );
            }
        }
    }
}
