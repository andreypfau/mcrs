use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_engine::column_ticks::{ColumnTicks, StoredTick};
use mcrs_minecraft_simulation::PendingTick;
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;

/// The ticks that generation scheduled in a column, as the column keeps them until a queue takes
/// them. Generation keeps no delay: each tick is due when its section first runs ticks. A later
/// tick of the same block or fluid at the same position is dropped.
pub(crate) fn column_ticks(
    ticks: &[PendingTick<GenerationKinds>],
    definitions: &BlockDefinitions,
) -> ColumnTicks {
    let mut column = ColumnTicks::default();
    for tick in ticks {
        match *tick {
            PendingTick::Block {
                pos,
                block,
                priority,
                ..
            } => {
                let Some(kind) = definitions.block_index(block) else {
                    continue;
                };
                if column
                    .block
                    .iter()
                    .any(|held| held.pos == pos && held.kind == kind)
                {
                    continue;
                }
                column.block.push(StoredTick {
                    pos,
                    kind,
                    delay: 0,
                    priority,
                });
            }
            PendingTick::Fluid {
                pos,
                fluid,
                priority,
                ..
            } => {
                let kind = fluid.id();
                if column
                    .fluid
                    .iter()
                    .any(|held| held.pos == pos && held.kind == kind)
                {
                    continue;
                }
                column.fluid.push(StoredTick {
                    pos,
                    kind,
                    delay: 0,
                    priority,
                });
            }
        }
    }
    column
}
