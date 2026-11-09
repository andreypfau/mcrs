use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{BlockPosArgument, Coordinates, block_floor};
use mcrs_minecraft_command_argument_block::BlockStateArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback,
};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaContext};
use mcrs_minecraft_engine::game_rules::GameRules;
use mcrs_minecraft_engine_world::voxel_update::BlockSetRequest;
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_text::Text;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos};

use crate::block_view::BlockView;
use crate::forced_columns::ForcedColumns;
use crate::landing::run_next_tick;

/// The flags of a set of a command that is not strict: the clients hear of it, a block entity
/// keeps its side effects quiet, and the neighbours get no update.
const COMMAND_SET: BlockUpdateFlags =
    BlockUpdateFlags::CLIENTS.union(BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
const RECURSION: i16 = 512;

/// Which positions of the region a fill changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Every position gets the block; `replace` without a filter is the same.
    Plain,
    /// The border positions get the block; the inner positions do not change.
    Outline,
    /// The border positions get the block; the inner positions get air.
    Hollow,
}

#[derive(Clone, Copy)]
pub(crate) struct FillCommand {
    from: ArgumentHandle<BlockPosArgument>,
    to: ArgumentHandle<BlockPosArgument>,
    block: ArgumentHandle<BlockStateArgument>,
}

// The modes `destroy`, `keep` and `strict` and the filter of `replace` are not
// registered, and a fill sends no neighbour update after its sets.
pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Mode>,
) -> Result<FillCommand, BuildError> {
    let fill = registrar
        .literal(NodeId::ROOT, "fill")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let from = registrar.arg(fill, "from", BlockPosArgument).id()?;
    let to = registrar.arg(from, "to", BlockPosArgument).id()?;
    let block = registrar
        .arg(to, "block", BlockStateArgument)
        .runs(Mode::Plain)
        .id()?;
    for (name, mode) in [
        ("outline", Mode::Outline),
        ("hollow", Mode::Hollow),
        ("replace", Mode::Plain),
    ] {
        let _ = registrar.literal(block, name).runs(mode).id()?;
    }
    Ok(FillCommand { from, to, block })
}

/// The box of block positions between two corners, both included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    min: BlockPos,
    max: BlockPos,
}

impl Region {
    pub(crate) fn new(a: BlockPos, b: BlockPos) -> Self {
        Self {
            min: BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    /// The number of positions, as a 64-bit integer.
    pub(crate) fn volume(self) -> i64 {
        let side = |min: i32, max: i32| i64::from(max) - i64::from(min) + 1;
        side(self.min.x, self.max.x)
            .saturating_mul(side(self.min.y, self.max.y))
            .saturating_mul(side(self.min.z, self.max.z))
    }

    /// Fails with `commands.fill.toobig` when the region has more than `limit` positions.
    pub(crate) fn check(self, limit: i32) -> Result<(), CommandFailure> {
        let volume = self.volume();
        if volume > i64::from(limit) {
            return Err(CommandFailure::translate(
                "commands.fill.toobig",
                vec![limit.into(), volume.into()],
            ));
        }
        Ok(())
    }

    fn is_border(self, pos: BlockPos) -> bool {
        pos.x == self.min.x
            || pos.x == self.max.x
            || pos.y == self.min.y
            || pos.y == self.max.y
            || pos.z == self.min.z
            || pos.z == self.max.z
    }

    /// Each position with x the fastest, then y, then z.
    fn positions(self) -> impl Iterator<Item = BlockPos> {
        (self.min.z..=self.max.z).flat_map(move |z| {
            (self.min.y..=self.max.y)
                .flat_map(move |y| (self.min.x..=self.max.x).map(move |x| BlockPos::new(x, y, z)))
        })
    }

    /// The state that `mode` gives `pos`, or `None` when it leaves the position alone.
    pub(crate) fn target(
        self,
        mode: Mode,
        pos: BlockPos,
        block: BlockStateId,
    ) -> Option<BlockStateId> {
        match mode {
            Mode::Plain => Some(block),
            _ if self.is_border(pos) => Some(block),
            Mode::Outline => None,
            Mode::Hollow => Some(BlockStateId::AIR),
        }
    }
}

/// What a fill reads of this world besides the context of the command.
#[derive(SystemParam)]
pub(crate) struct FillWorld<'w, 's> {
    view: BlockView<'w, 's>,
    rules: Option<Res<'w, GameRules>>,
    blocks: Option<Res<'w, Blocks>>,
    forced: Res<'w, ForcedColumns>,
    label: Res<'w, DimensionLabel>,
}

pub(crate) fn run(
    In(call): In<Call<Mode, FillCommand>>,
    context: VanillaContext,
    world: FillWorld,
    mut commands: Commands,
    mut output: CommandOutput,
    mut sets: MessageWriter<BlockSetRequest>,
) -> CommandResult {
    let FillWorld {
        view,
        rules,
        blocks,
        forced,
        label,
    } = world;
    let command = call.command;
    let source = context.source(&call.invocation)?;
    let corner = |handle: ArgumentHandle<BlockPosArgument>| -> Result<BlockPos, CommandFailure> {
        let coordinates: Coordinates = context.arg(&call.invocation, handle)?;
        let [x, y, z] = block_floor(coordinates.position(&source));
        Ok(BlockPos::new(x, y, z))
    };
    let (from, to) = (corner(command.from)?, corner(command.to)?);
    let landing = |pos: BlockPos| !view.is_loaded(pos) && forced.contains(ColumnPos::from(pos));
    if landing(from) || landing(to) {
        run_next_tick(&mut commands, &context, &label, &call.invocation);
        return Ok(0);
    }
    view.readable(from)?;
    view.readable(to)?;
    let region = Region::new(from, to);
    let limit = rules.as_deref().cloned().unwrap_or_default();
    region.check(limit.get(GameRule::MaxBlockModifications))?;
    let block = context.arg(&call.invocation, command.block)?;
    let state = blocks
        .as_deref()
        .and_then(|blocks| block.state(blocks))
        .ok_or_else(CommandFailure::failed)?;
    let dimension = view.dimension().ok_or_else(CommandFailure::failed)?;
    let mut changed: i32 = 0;
    for pos in region.positions() {
        let Some(target) = region.target(call.branch, pos, state) else {
            continue;
        };
        if view.block_state(pos) == Some(target) {
            continue;
        }
        sets.write(BlockSetRequest {
            dimension,
            pos,
            new_state: target,
            flags: COMMAND_SET,
            recursion_left: RECURSION,
        });
        changed = changed.saturating_add(1);
    }
    if changed == 0 {
        return Err(CommandFailure::translate("commands.fill.failed", vec![]));
    }
    output.success(
        &call.invocation,
        Text::translate_args("commands.fill.success", vec![changed.into()]),
        Feedback::SourceAndOperators,
    );
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_over_the_limit_is_too_big_and_one_at_the_limit_is_not() {
        let region = Region::new(BlockPos::new(181, -61, 181), BlockPos::new(0, -61, 0));
        assert_eq!(region.volume(), 33124);
        assert_eq!(
            region.check(32768),
            Err(CommandFailure::translate(
                "commands.fill.toobig",
                vec![32768.into(), 33124i64.into()]
            ))
        );
        assert_eq!(region.check(33124), Ok(()));
    }

    #[test]
    fn hollow_clears_the_inner_positions_and_a_flat_region_has_none() {
        let block = BlockStateId::checked(5, 10).expect("below the bound");
        let cube = Region::new(BlockPos::new(0, 0, 0), BlockPos::new(2, 2, 2));
        let centre = BlockPos::new(1, 1, 1);
        assert_eq!(
            cube.target(Mode::Hollow, centre, block),
            Some(BlockStateId::AIR)
        );
        assert_eq!(cube.target(Mode::Outline, centre, block), None);
        let ring = Region::new(BlockPos::new(0, -60, 0), BlockPos::new(4, -60, 4));
        let inner = BlockPos::new(2, -60, 2);
        assert_eq!(ring.target(Mode::Hollow, inner, block), Some(block));
        assert_eq!(ring.positions().count(), 25);
    }
}
