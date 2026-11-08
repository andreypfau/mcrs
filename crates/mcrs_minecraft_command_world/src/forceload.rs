use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::ColumnPosArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback,
};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaContext};
use mcrs_minecraft_engine::teleport::MAX_XZ_BLOCKS;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_text::{IntoText, Text, TranslateArg};
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::forced_columns::ForcedColumns;

/// The most columns that one `add` or `remove` changes.
const MAX_COLUMNS: i32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Add,
    Remove,
    RemoveAll,
    Query,
    QueryAt,
}

#[derive(Clone, Copy)]
pub(crate) struct ForceloadCommand {
    add_from: ArgumentHandle<ColumnPosArgument>,
    add_to: ArgumentHandle<ColumnPosArgument>,
    remove_from: ArgumentHandle<ColumnPosArgument>,
    remove_to: ArgumentHandle<ColumnPosArgument>,
    query_at: ArgumentHandle<ColumnPosArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<ForceloadCommand, BuildError> {
    let forceload = registrar
        .literal(NodeId::ROOT, "forceload")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let add = registrar.literal(forceload, "add").id()?;
    let add_from = registrar
        .arg(add, "from", ColumnPosArgument)
        .runs(Branch::Add)
        .id()?;
    let add_to = registrar
        .arg(add_from, "to", ColumnPosArgument)
        .runs(Branch::Add)
        .id()?;
    let remove = registrar.literal(forceload, "remove").id()?;
    let remove_from = registrar
        .arg(remove, "from", ColumnPosArgument)
        .runs(Branch::Remove)
        .id()?;
    let remove_to = registrar
        .arg(remove_from, "to", ColumnPosArgument)
        .runs(Branch::Remove)
        .id()?;
    let _ = registrar
        .literal(remove, "all")
        .runs(Branch::RemoveAll)
        .id()?;
    let query = registrar
        .literal(forceload, "query")
        .runs(Branch::Query)
        .id()?;
    let query_at = registrar
        .arg(query, "pos", ColumnPosArgument)
        .runs(Branch::QueryAt)
        .id()?;
    Ok(ForceloadCommand {
        add_from,
        add_to,
        remove_from,
        remove_to,
        query_at,
    })
}

/// The columns of the block range between two corners, both included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColumnRange {
    min: ColumnPos,
    max: ColumnPos,
}

impl ColumnRange {
    /// The range of the block corners `from` and `to`, or `argument.pos.outofworld` when a corner
    /// is past the world border.
    pub(crate) fn new(from: [i32; 2], to: [i32; 2]) -> Result<Self, CommandFailure> {
        let [x1, z1] = from;
        let [x2, z2] = to;
        let (min_x, max_x, min_z, max_z) = (x1.min(x2), x1.max(x2), z1.min(z2), z1.max(z2));
        if min_x < -MAX_XZ_BLOCKS
            || min_z < -MAX_XZ_BLOCKS
            || max_x >= MAX_XZ_BLOCKS
            || max_z >= MAX_XZ_BLOCKS
        {
            return Err(CommandFailure::translate("argument.pos.outofworld", vec![]));
        }
        let column = |x: i32, z: i32| ColumnPos::new(x >> SectionPos::BITS, z >> SectionPos::BITS);
        Ok(Self {
            min: column(min_x, min_z),
            max: column(max_x, max_z),
        })
    }

    pub(crate) fn count(self) -> i64 {
        (i64::from(self.max.x) - i64::from(self.min.x) + 1)
            * (i64::from(self.max.z) - i64::from(self.min.z) + 1)
    }

    /// Fails with `commands.forceload.toobig` when the range holds more than 256 columns.
    pub(crate) fn check(self) -> Result<(), CommandFailure> {
        let count = self.count();
        if count > i64::from(MAX_COLUMNS) {
            let count = i32::try_from(count).unwrap_or(i32::MAX);
            return Err(CommandFailure::translate(
                "commands.forceload.toobig",
                vec![MAX_COLUMNS.into(), count.into()],
            ));
        }
        Ok(())
    }

    /// The columns in order of x, then z.
    fn columns(self) -> impl Iterator<Item = ColumnPos> {
        (self.min.x..=self.max.x)
            .flat_map(move |x| (self.min.z..=self.max.z).map(move |z| ColumnPos::new(x, z)))
    }
}

fn chunk_text(column: ColumnPos) -> TranslateArg<mcrs_minecraft_text::ItemHover> {
    format!("[{}, {}]", column.x, column.z).into_text().into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    Add,
    Remove,
}

pub(crate) fn run(
    In(call): In<Call<Branch, ForceloadCommand>>,
    context: VanillaContext,
    mut forced: ResMut<ForcedColumns>,
    mut loading: Query<&mut ColumnLoading>,
    mut output: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let source = context.source(&call.invocation)?;
    let dimension: TranslateArg<_> = context
        .targets()
        .dimensions()
        .iter()
        .find(|entry| entry.label == call.invocation.source.dimension)
        .map(|entry| entry.key.location().to_string())
        .unwrap_or_default()
        .into_text()
        .into();
    let block_of = |handle| -> Result<Option<[i32; 2]>, CommandFailure> {
        Ok(context.opt(&call.invocation, handle)?.map(
            |coordinates: mcrs_minecraft_command_argument::ColumnCoordinates| {
                coordinates.block(&source)
            },
        ))
    };
    let (change, from, to) = match call.branch {
        Branch::Add => (Change::Add, command.add_from, command.add_to),
        Branch::Remove => (Change::Remove, command.remove_from, command.remove_to),
        Branch::RemoveAll => {
            let mut loading = loading.single_mut().map_err(|_| CommandFailure::failed())?;
            for column in forced.take() {
                loading.remove_ticket(column);
            }
            output.success(
                &call.invocation,
                Text::translate_args("commands.forceload.removed.all", vec![dimension]),
                Feedback::SourceAndOperators,
            );
            return Ok(0);
        }
        Branch::QueryAt => {
            let [x, z] = block_of(command.query_at)?.ok_or_else(CommandFailure::failed)?;
            let column = ColumnRange::new([x, z], [x, z])?.min;
            let args = vec![chunk_text(column), dimension];
            if !forced.contains(column) {
                return Err(CommandFailure::translate(
                    "commands.forceload.query.failure",
                    args,
                ));
            }
            output.success(
                &call.invocation,
                Text::translate_args("commands.forceload.query.success", args),
                Feedback::Source,
            );
            return Ok(1);
        }
        Branch::Query => {
            let count = i32::try_from(forced.len()).unwrap_or(i32::MAX);
            let list = forced
                .iter()
                .map(|column| format!("[{}, {}]", column.x, column.z))
                .collect::<Vec<String>>()
                .join(", ");
            let message = match count {
                0 => Text::translate_args("commands.forceload.added.none", vec![dimension]),
                1 => Text::translate_args(
                    "commands.forceload.list.single",
                    vec![dimension, list.into_text().into()],
                ),
                _ => Text::translate_args(
                    "commands.forceload.list.multiple",
                    vec![count.into(), dimension, list.into_text().into()],
                ),
            };
            output.success(&call.invocation, message, Feedback::Source);
            return Ok(count);
        }
    };
    let from = block_of(from)?.ok_or_else(CommandFailure::failed)?;
    let to = block_of(to)?.unwrap_or(from);
    let range = ColumnRange::new(from, to)?;
    range.check()?;
    let mut loading = loading.single_mut().map_err(|_| CommandFailure::failed())?;
    let mut changed: i32 = 0;
    let mut last = range.min;
    for column in range.columns() {
        let marked = match change {
            Change::Add => forced.insert(column),
            Change::Remove => forced.remove(column),
        };
        if !marked {
            continue;
        }
        match change {
            Change::Add => loading.add_ticket(column),
            Change::Remove => loading.remove_ticket(column),
        }
        changed = changed.saturating_add(1);
        last = column;
    }
    let (single, multiple, failure) = match change {
        Change::Add => (
            "commands.forceload.added.single",
            "commands.forceload.added.multiple",
            "commands.forceload.added.failure",
        ),
        Change::Remove => (
            "commands.forceload.removed.single",
            "commands.forceload.removed.multiple",
            "commands.forceload.removed.failure",
        ),
    };
    let message = match changed {
        0 => return Err(CommandFailure::translate(failure, vec![])),
        1 => Text::translate_args(single, vec![chunk_text(last), dimension]),
        _ => Text::translate_args(
            multiple,
            vec![
                changed.into(),
                dimension,
                chunk_text(range.min),
                chunk_text(range.max),
            ],
        ),
    };
    output.success(&call.invocation, message, Feedback::SourceAndOperators);
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_of_more_than_256_columns_is_too_big() {
        let fits = ColumnRange::new([0, 0], [255, 255]).expect("inside the world");
        assert_eq!((fits.count(), fits.check()), (256, Ok(())));
        let over = ColumnRange::new([256, 255], [0, 0]).expect("inside the world");
        assert_eq!(
            over.check(),
            Err(CommandFailure::translate(
                "commands.forceload.toobig",
                vec![256.into(), 272.into()]
            ))
        );
        let negative = ColumnRange::new([-16, 0], [15, 0]).expect("inside the world");
        assert_eq!(negative.columns().count(), 2);
    }
}
