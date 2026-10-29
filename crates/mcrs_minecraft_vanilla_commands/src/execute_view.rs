use crate::dimension_view::{DimensionView, unloaded};
use crate::execute::{Branch, ExecuteCommand, Polarity, Test};
use crate::select::{FoundPlayers, select};
use crate::set::VanillaContext;
use bevy_ecs::system::SystemParam;
use bevy_math::DVec3;
use mcrs_command::ArgumentHandle;
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_command::{DimensionArgument, Lookup, ResourceOrTag};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, DimensionEntry, Modified, ModifyResult, PlayerEntry,
    SourceState,
};
use mcrs_minecraft_command_entity::EntityArgument;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::text::IntoText;

/// Everything a stage of `execute` reads: the players of every dimension through the context,
/// and the blocks of the source dimension, the world the stage runs in.
#[derive(SystemParam)]
pub(crate) struct ExecuteView<'w, 's> {
    pub(crate) context: VanillaContext<'w, 's>,
    pub(crate) dimension: DimensionView<'w, 's>,
}

/// The result of a condition: a boolean, or a count (CMD-EX section 0.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Boolean(bool),
    Count(i32),
}

impl ExecuteView<'_, '_> {
    /// One source for each entity of `targets` in any dimension, from `each` (CMD-EX-024).
    pub(crate) fn fork(
        &self,
        call: &Call<Branch, ExecuteCommand>,
        targets: ArgumentHandle<EntityArgument>,
        found: &mut FoundPlayers,
        output: &mut CommandOutput,
        each: impl Fn(&SourceState, &PlayerEntry) -> SourceState,
    ) -> ModifyResult {
        found.with(|out| {
            select(&self.context, call, targets, out)?;
            for target in out.iter() {
                output.fork(each(&call.source, target));
            }
            Ok(Modified::Forked)
        })
    }

    pub(crate) fn condition(
        &self,
        call: &Call<Branch, ExecuteCommand>,
        test: Test,
        polarity: Polarity,
        found: &mut FoundPlayers,
    ) -> Result<Outcome, CommandFailure> {
        let args = call.command.condition(polarity);
        let context = &self.context;
        let source = context.source(call)?;
        match test {
            Test::Entity => found.with(|out| {
                select(context, call, args.entities, out)?;
                Ok(Outcome::Count(i32::try_from(out.len()).unwrap_or(i32::MAX)))
            }),
            Test::Dimension => {
                let named = dimension(context, call, args.dimension)?;
                Ok(Outcome::Boolean(named.label == call.source.dimension))
            }
            Test::Loaded => {
                let position = context.arg(call, args.loaded)?.position(&source);
                let [x, _, z] = mcrs_minecraft_command_coordinates::block_floor(position);
                Ok(Outcome::Boolean(self.dimension.loaded(x, z)))
            }
            Test::Block => {
                let position = context.arg(call, args.block_pos)?.position(&source);
                let pos = mcrs_minecraft_command_coordinates::block_floor(position);
                self.dimension.readable(pos)?;
                let predicate = context.arg(call, args.block)?;
                let state = self.dimension.block_state(pos).ok_or_else(unloaded)?;
                let entry = context
                    .lookup()
                    .blocks()
                    .and_then(|blocks| blocks.0.owner(state));
                let Some(entry) = entry else {
                    return Ok(Outcome::Boolean(false));
                };
                let name = entry.identifier.as_str();
                let in_subject = match predicate.subject {
                    ResourceOrTag::Entry(id) => id.eq_str(name),
                    ResourceOrTag::Tag(tag) => context
                        .lookup()
                        .tag_entries(BLOCK.location(), tag)
                        .any(|member| member == name),
                };
                let passes = in_subject
                    && predicate.test(entry, state)
                    && predicate.test_data(|| {
                        let entry = self.dimension.block_entity(pos)?;
                        saved_data(entry).ok()
                    });
                Ok(Outcome::Boolean(passes))
            }
        }
    }

    /// CMD-EX-021: the source gets the dimension, and its x and z are scaled.
    pub(crate) fn move_to(
        &self,
        call: &Call<Branch, ExecuteCommand>,
        state: SourceState,
    ) -> ModifyResult {
        let to = dimension(&self.context, call, call.command.dimension)?;
        let targets = self.context.targets();
        let from = targets
            .dimension_of(state.dimension)
            .and_then(|index| targets.dimensions().get(index))
            .ok_or_else(CommandFailure::failed)?;
        if to.label == from.label {
            return Ok(Modified::One(state));
        }
        let scale = from.coordinate_scale / to.coordinate_scale;
        Ok(Modified::One(SourceState {
            dimension: to.label,
            position: DVec3::new(
                state.position.x * scale,
                state.position.y,
                state.position.z * scale,
            ),
            ..state
        }))
    }
}

/// The running dimension an argument names (ARG-RES-004).
fn dimension<'a>(
    context: &'a VanillaContext,
    call: &Call<Branch, ExecuteCommand>,
    arg: ArgumentHandle<DimensionArgument>,
) -> Result<&'a DimensionEntry, CommandFailure> {
    let id = context.arg(call, arg)?;
    context.targets().dimension_named(id).ok_or_else(|| {
        CommandFailure::translate(
            "argument.dimension.invalid",
            vec![id.to_string().into_text()],
        )
    })
}

/// The saved data of a block entity (ARG-BLK-012): the compound the save writes, which holds
/// `id`, `x`, `y` and `z`, with an empty `components` when it has none.
fn saved_data(entry: &GeneratedBlockEntity) -> Result<NbtCompound, mcrs_minecraft_nbt::Error> {
    let mut compound = mcrs_minecraft_nbt::to_nbt_compound(entry)?;
    if compound.get("components").is_none() {
        compound.put_component("components", NbtCompound::new());
    }
    Ok(compound)
}
