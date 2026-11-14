use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockStateId;

use crate::Click;

/// The state of `block` that a click places over `replaced` (PLC-STA-001, PLC-STA-002,
/// PLC-STA-004, PLC-STA-005). `None` for a block of two parts, which this placement does not
/// place yet.
// chisle: no wall forms, no connections and no merges; each needs its own rules.
pub fn placed_state(
    definitions: &BlockDefinitions,
    block: Id<Block>,
    click: &Click,
    replaced: BlockStateId,
) -> Option<BlockStateId> {
    let entry = definitions.entry(block)?;
    if has_value(entry, "half", "upper") || has_value(entry, "part", "head") {
        return None;
    }
    let mut state = entry.default_state_id;
    let mut set = |property: &str, value: &str| {
        if let Some(next) = entry.with_text(state, property, value) {
            state = next;
        }
    };
    let half = if click.upper_half() { "top" } else { "bottom" };
    if has_value(entry, "type", "double") {
        set("type", half);
    }
    let stairs = has_value(entry, "shape", "inner_left");
    if stairs {
        set("half", half);
    }
    set("axis", click.face.axis().name());
    if let Some(facing) = facing(block, entry, click, stairs) {
        set("facing", facing.name());
    }
    let water_source = definitions
        .state(replaced)
        .fluid
        .is_some_and(|fluid| fluid.source && fluid.fluid == Fluid::Water.id());
    set("waterlogged", if water_source { "true" } else { "false" });
    Some(state)
}

fn has_value(entry: &BlockEntry, property: &str, value: &str) -> bool {
    entry
        .with_text(entry.default_state_id, property, value)
        .is_some()
}

fn facing(block: Id<Block>, entry: &BlockEntry, click: &Click, stairs: bool) -> Option<Direction> {
    let attached = ["face", "attachment", "hanging"]
        .iter()
        .any(|property| entry.properties.index_of(property).is_some());
    if attached || entry.properties.index_of("facing").is_none() {
        return None;
    }
    let anvil = [Block::Anvil, Block::ChippedAnvil, Block::DamagedAnvil]
        .iter()
        .any(|anvil| anvil.id() == block);
    let horizontal = click.horizontal_look();
    Some(
        if block == Block::EndRod.id() || block == Block::AmethystCluster.id() {
            click.face
        } else if block == Block::Hopper.id() {
            if click.face.is_vertical() {
                Direction::Down
            } else {
                click.face.opposite()
            }
        } else if block == Block::Observer.id() {
            click.look()
        } else if anvil {
            horizontal.clockwise()
        } else if stairs || entry.properties.index_of("in_wall").is_some() {
            horizontal
        } else if has_value(entry, "facing", "up") {
            click.look().opposite()
        } else {
            horizontal.opposite()
        },
    )
}
