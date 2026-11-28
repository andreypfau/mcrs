use crate::dimension_view::DimensionView;
use crate::entity_view::EntityView;
use crate::execute_view::saved_data;
use crate::mob_view::MobView;
use crate::scale_text::scale_text;
use crate::select::{SelectionBuffers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, DoubleArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{BlockPosArgument, block_floor};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_argument_vanilla::{NbtPathArgument, NbtPathValue, VanillaArgument};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback,
};
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::save_player;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt_path::Number;
use mcrs_minecraft_snbt::text_form;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Entity,
    Block,
}

#[derive(Clone, Copy)]
pub(crate) struct Arguments<T> {
    target: ArgumentHandle<T>,
    path: ArgumentHandle<NbtPathArgument>,
    scale: ArgumentHandle<DoubleArgument>,
}

#[derive(Clone, Copy)]
pub(crate) struct DataCommand {
    entity: Arguments<EntityArgument>,
    block: Arguments<BlockPosArgument>,
}

pub(crate) enum Holder {
    Entity(Text),
    Block([i32; 3]),
}

struct Found {
    holder: Holder,
    data: NbtCompound,
    path: Option<NbtPathValue>,
    scale: Option<f64>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Form>,
) -> Result<DataCommand, BuildError> {
    let data = registrar
        .literal(NodeId::ROOT, "data")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let get = registrar.literal(data, "get").id()?;
    let entity = registrar.literal(get, "entity").id()?;
    let entity = arguments(registrar, entity, EntityArgument::ONE_ENTITY, Form::Entity)?;
    let block = registrar.literal(get, "block").id()?;
    let block = arguments(registrar, block, BlockPosArgument, Form::Block)?;
    Ok(DataCommand { entity, block })
}

fn arguments<T: Copy + Into<VanillaArgument>>(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Form>,
    parent: NodeId,
    argument: T,
    form: Form,
) -> Result<Arguments<T>, BuildError> {
    let target = registrar.arg(parent, "target", argument).runs(form).id()?;
    let path = registrar
        .arg(target, "path", NbtPathArgument)
        .runs(form)
        .id()?;
    let scale = registrar
        .arg(path, "scale", DoubleArgument::ANY)
        .runs(form)
        .id()?;
    Ok(Arguments {
        target,
        path,
        scale,
    })
}

pub(crate) fn run(
    In(call): In<Call<Form, DataCommand>>,
    context: VanillaContext,
    dimension: DimensionView,
    mut found: Local<SelectionBuffers>,
    mut access: ParamSet<(&World, CommandOutput)>,
) -> CommandResult {
    let Found {
        holder,
        data,
        path,
        scale,
    } = match call.branch {
        Form::Entity => find_entity(&call, &context, &mut found, access.p0())?,
        Form::Block => find_block(&call, &context, &dimension)?,
    };
    let invocation = &call.invocation;
    let data = NbtTag::Compound(data);
    let mut output = access.p1();
    let Some(path) = path else {
        output.success(invocation, holder.query(&data), Feedback::Source);
        return Ok(1);
    };
    let value = single_value(&data, &path)?;
    let Some(scale) = scale else {
        output.success(invocation, holder.query(&value), Feedback::Source);
        return Ok(result_of(&value));
    };
    let number = Number::of(&value).ok_or_else(|| {
        CommandFailure::translate("commands.data.get.invalid", vec![path.text.clone().into()])
    })?;
    let result = floor_to_int(number.to_f64() * scale);
    output.success(
        invocation,
        holder.get(path.text, scale_text(scale), result),
        Feedback::Source,
    );
    Ok(result)
}

fn find_entity(
    call: &Call<Form, DataCommand>,
    context: &VanillaContext,
    found: &mut SelectionBuffers,
    world: &World,
) -> Result<Found, CommandFailure> {
    let arguments = call.command.entity;
    found.with(|targets| {
        select(context, &call.invocation, arguments.target, targets)?;
        let target = targets
            .first()
            .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))?;
        let path = context.opt(&call.invocation, arguments.path)?;
        let scale = context.opt(&call.invocation, arguments.scale)?;
        let data = match target {
            EntityView::Player(player) => {
                player_data(world, player.entity.ok_or_else(CommandFailure::failed)?)?
            }
            EntityView::Mob { view, .. } => mob_data(view),
        };
        Ok(Found {
            holder: Holder::Entity(target.display_name()),
            data,
            path,
            scale,
        })
    })
}

fn find_block(
    call: &Call<Form, DataCommand>,
    context: &VanillaContext,
    dimension: &DimensionView,
) -> Result<Found, CommandFailure> {
    let source = context.source(&call.invocation)?;
    let pos = block_floor(
        context
            .arg(&call.invocation, call.command.block.target)?
            .position(&source),
    );
    dimension.readable(pos)?;
    let entry = dimension
        .block_entity(pos)
        .ok_or_else(|| CommandFailure::translate("commands.data.block.invalid", vec![]))?;
    let data = saved_data(entry).map_err(|error| {
        tracing::error!("the data of a block entity was not captured: {error}");
        CommandFailure::failed()
    })?;
    Ok(Found {
        holder: Holder::Block(pos),
        data,
        path: context.opt(&call.invocation, call.command.block.path)?,
        scale: context.opt(&call.invocation, call.command.block.scale)?,
    })
}

impl Holder {
    fn query(self, data: &NbtTag) -> Text {
        let data = text_form(data).into_text();
        match self {
            Self::Entity(name) => Text::translate("commands.data.entity.query", vec![name, data]),
            Self::Block([x, y, z]) => Text::translate_args(
                "commands.data.block.query",
                vec![x.into(), y.into(), z.into(), data.into()],
            ),
        }
    }

    fn get(self, path: String, scale: String, result: i32) -> Text {
        match self {
            Self::Entity(name) => Text::translate_args(
                "commands.data.entity.get",
                vec![path.into(), name.into(), scale.into(), result.into()],
            ),
            Self::Block([x, y, z]) => Text::translate_args(
                "commands.data.block.get",
                vec![
                    path.into(),
                    x.into(),
                    y.into(),
                    z.into(),
                    scale.into(),
                    result.into(),
                ],
            ),
        }
    }
}

fn player_data(world: &World, player: Entity) -> Result<NbtCompound, CommandFailure> {
    let mut data = save_player(world, player, CaptureCause::Autosave)
        .map_err(|error| {
            tracing::error!("the data of a player was not captured: {error}");
            CommandFailure::failed()
        })?
        .root;
    if let Some(item) = selected_item(&data) {
        data.put_component("SelectedItem", item);
    }
    Ok(data)
}

fn selected_item(data: &NbtCompound) -> Option<NbtCompound> {
    let slot = data.get_int("SelectedItemSlot")?;
    let mut item = data
        .get_list("Inventory")?
        .iter()
        .find_map(|entry| match entry {
            NbtTag::Compound(entry) if entry.get_byte("Slot").map(i32::from) == Some(slot) => {
                Some(entry.clone())
            }
            _ => None,
        })?;
    item.child_tags.retain(|(key, _)| key != "Slot");
    Some(item)
}

fn mob_data(view: MobView<'_>) -> NbtCompound {
    let position = view.position();
    let look = view.look();
    let mut data = NbtCompound::new();
    data.put_list(
        "Pos",
        vec![
            NbtTag::Double(position.x),
            NbtTag::Double(position.y),
            NbtTag::Double(position.z),
        ],
    );
    data.put_list(
        "Rotation",
        vec![NbtTag::Float(look.yaw), NbtTag::Float(look.pitch)],
    );
    if let Some(vitals) = view.vitals {
        data.put_float("Health", vitals.health);
    }
    data
}

fn single_value(data: &NbtTag, path: &NbtPathValue) -> Result<NbtTag, CommandFailure> {
    let mut selected = Vec::new();
    path.path
        .try_select(data, &mut selected)
        .map_err(|nothing| {
            let text = path.text.get(..nothing.end).unwrap_or(&path.text);
            CommandFailure::translate(
                "arguments.nbtpath.nothing_found",
                vec![text.to_owned().into()],
            )
        })?;
    let mut selected = selected.into_iter();
    match (selected.next(), selected.next()) {
        (Some(value), None) => Ok(value.into_owned()),
        _ => Err(CommandFailure::translate(
            "commands.data.get.multiple",
            vec![],
        )),
    }
}

fn result_of(value: &NbtTag) -> i32 {
    if let Some(number) = Number::of(value) {
        return floor_to_int(number.to_f64());
    }
    let count = match value {
        NbtTag::ByteArray(elements) => elements.len(),
        NbtTag::IntArray(elements) => elements.len(),
        NbtTag::LongArray(elements) => elements.len(),
        NbtTag::List(elements) => elements.len(),
        NbtTag::Compound(compound) => compound.child_tags.len(),
        NbtTag::String(text) => text.encode_utf16().count(),
        _ => 0,
    };
    i32::try_from(count).unwrap_or(i32::MAX)
}

fn floor_to_int(value: f64) -> i32 {
    value.floor() as i32
}
