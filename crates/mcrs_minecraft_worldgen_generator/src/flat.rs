use mcrs_minecraft_chunk::VoxelId;
use mcrs_voxel::{BlockStateId, SectionPos};

use crate::ColumnBlocks;

/// One layer of a flat dimension: `height` blocks of `state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatLayer {
    pub state: BlockStateId,
    pub height: u32,
}

/// The layers of a flat dimension, from the bottom of the dimension up. Above the last layer
/// every block is air.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlatLayers {
    pub layers: Vec<FlatLayer>,
}

impl FlatLayers {
    /// Fills `column` with the layers from `min_y` up. A layer above the column is cut off.
    pub fn fill(&self, column: &mut ColumnBlocks, y_sections: &[i32], min_y: i32) {
        column.reset(y_sections);
        let mut y = min_y;
        for layer in &self.layers {
            let height = i32::try_from(layer.height).unwrap_or(i32::MAX);
            let top = y.saturating_add(height);
            for block_y in y..top {
                let Some(index) = column.section_index(block_y) else {
                    continue;
                };
                let local = block_y & SectionPos::MASK as i32;
                let state = VoxelId(layer.state.number());
                column.fill_box_in_section(index, 0, 16, local, local + 1, 0, 16, state);
            }
            y = top;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layers_stack_from_the_bottom_of_the_dimension() {
        let y_sections: Vec<i32> = (-4..20).collect();
        let state = |number| BlockStateId::checked(number, 4).expect("below the bound");
        let (bedrock, dirt, grass) = (state(1), state(2), state(3));
        let layers = FlatLayers {
            layers: vec![
                FlatLayer {
                    state: bedrock,
                    height: 1,
                },
                FlatLayer {
                    state: dirt,
                    height: 2,
                },
                FlatLayer {
                    state: grass,
                    height: 1,
                },
            ],
        };
        let mut column = ColumnBlocks::new(&y_sections);
        layers.fill(&mut column, &y_sections, -64);
        let cells = column.section_cells(0);
        let at = |y: usize, x: usize, z: usize| cells[y * 256 + z * 16 + x].get();
        assert_eq!(
            [
                at(0, 0, 0),
                at(1, 5, 9),
                at(2, 15, 15),
                at(3, 7, 3),
                at(4, 0, 0)
            ],
            [
                VoxelId(1),
                VoxelId(2),
                VoxelId(2),
                VoxelId(3),
                VoxelId::default()
            ],
            "the grass block is the top block, at y = -61"
        );
    }
}
