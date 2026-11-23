use crate::dimension_view::DimensionView;
use crate::entity_view::EntityView;
use crate::execute_view::saved_data;
use crate::mob_view::MobView;
use crate::scale_text::scale_text;
use crate::select::{SelectionBuffers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, DoubleArgument, NodeId, StringArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{BlockPosArgument, block_floor};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_argument_vanilla::VanillaArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback, Invocation,
};
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::save_player;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_snbt::text_form;
use mcrs_minecraft_text::{IntoText, Text};

const INVENTORY: &str = "Inventory";
const SELECTED_ITEM_SLOT: &str = "SelectedItemSlot";
const SELECTED_ITEM: &str = "SelectedItem";
const SLOT: &str = "Slot";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Entity,
    Block,
}

#[derive(Clone, Copy)]
pub(crate) struct Arguments<T> {
    target: ArgumentHandle<T>,
    path: ArgumentHandle<StringArgument>,
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

pub(crate) struct Query {
    invocation: Invocation,
    holder: Holder,
    path: Option<String>,
    scale: Option<f64>,
}

pub(crate) enum Subject {
    Player(Entity),
    Saved(NbtCompound),
}

pub(crate) struct Found {
    query: Query,
    subject: Subject,
}

pub(crate) struct Read {
    query: Query,
    data: NbtTag,
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
        .arg(target, "path", StringArgument::Word)
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

pub(crate) fn find(
    In(call): In<Call<Form, DataCommand>>,
    context: VanillaContext,
    dimension: DimensionView,
    mut found: Local<SelectionBuffers>,
) -> Result<Found, CommandFailure> {
    match call.branch {
        Form::Entity => find_entity(&call, &context, &mut found),
        Form::Block => find_block(&call, &context, &dimension),
    }
}

fn query<T>(
    call: &Call<Form, DataCommand>,
    context: &VanillaContext,
    arguments: Arguments<T>,
    holder: Holder,
) -> Result<Query, CommandFailure> {
    let path: Option<String> = context
        .opt(&call.invocation, arguments.path)?
        .map(|path| path.chars().collect());
    Ok(Query {
        invocation: call.invocation.clone(),
        holder,
        path,
        scale: context.opt(&call.invocation, arguments.scale)?,
    })
}

fn find_entity(
    call: &Call<Form, DataCommand>,
    context: &VanillaContext,
    found: &mut SelectionBuffers,
) -> Result<Found, CommandFailure> {
    let arguments = call.command.entity;
    found.with(|targets| {
        select(context, &call.invocation, arguments.target, targets)?;
        let target = targets
            .first()
            .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))?;
        let subject = match target {
            EntityView::Player(player) => {
                Subject::Player(player.entity.ok_or_else(CommandFailure::failed)?)
            }
            EntityView::Mob { view, .. } => Subject::Saved(mob_data(view)),
        };
        let query = query(
            call,
            context,
            arguments,
            Holder::Entity(target.display_name()),
        )?;
        Ok(Found { query, subject })
    })
}

fn find_block(
    call: &Call<Form, DataCommand>,
    context: &VanillaContext,
    dimension: &DimensionView,
) -> Result<Found, CommandFailure> {
    let arguments = call.command.block;
    let source = context.source(&call.invocation)?;
    let pos = block_floor(
        context
            .arg(&call.invocation, arguments.target)?
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
    let query = query(call, context, arguments, Holder::Block(pos))?;
    Ok(Found {
        query,
        subject: Subject::Saved(data),
    })
}

pub(crate) fn read(
    In(found): In<Result<Found, CommandFailure>>,
    world: &mut World,
) -> Result<Read, CommandFailure> {
    let Found { query, subject } = found?;
    let data = match subject {
        Subject::Player(player) => player_data(world, player)?,
        Subject::Saved(data) => data,
    };
    Ok(Read {
        query,
        data: NbtTag::Compound(data),
    })
}

pub(crate) fn reply(
    In(read): In<Result<Read, CommandFailure>>,
    mut output: CommandOutput,
) -> CommandResult {
    let Read { query, data } = read?;
    let Query {
        invocation,
        holder,
        path,
        scale,
    } = query;
    let Some(path) = path else {
        output.success(&invocation, holder.query(&data), Feedback::Source);
        return Ok(1);
    };
    let value = single_value(&data, &path)?;
    let Some(scale) = scale else {
        output.success(&invocation, holder.query(&value), Feedback::Source);
        return Ok(result_of(&value));
    };
    let number = number_of(&value).ok_or_else(|| {
        CommandFailure::translate("commands.data.get.invalid", vec![path.clone().into()])
    })?;
    let result = floor_to_int(number * scale);
    output.success(
        &invocation,
        holder.get(path, scale_text(scale), result),
        Feedback::Source,
    );
    Ok(result)
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

fn player_data(world: &mut World, player: Entity) -> Result<NbtCompound, CommandFailure> {
    let mut data = save_player(world, player, CaptureCause::Autosave)
        .map_err(|error| {
            tracing::error!("the data of a player was not captured: {error}");
            CommandFailure::failed()
        })?
        .root;
    if let Some(item) = selected_item(&data) {
        data.put_component(SELECTED_ITEM, item);
    }
    Ok(data)
}

fn selected_item(data: &NbtCompound) -> Option<NbtCompound> {
    let slot = data.get_int(SELECTED_ITEM_SLOT)?;
    let mut item = data
        .get_list(INVENTORY)?
        .iter()
        .find_map(|entry| match entry {
            NbtTag::Compound(entry) if entry.get_byte(SLOT).map(i32::from) == Some(slot) => {
                Some(entry.clone())
            }
            _ => None,
        })?;
    item.child_tags.retain(|(key, _)| key != SLOT);
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

fn single_value(data: &NbtTag, path_text: &str) -> Result<NbtTag, CommandFailure> {
    let path: NbtPath = path_text.parse().map_err(|_| CommandFailure::failed())?;
    let mut selected = Vec::new();
    path.select(data, &mut selected);
    let mut selected = selected.into_iter();
    match (selected.next(), selected.next()) {
        (Some(value), None) => Ok(value.into_owned()),
        (None, _) => Err(CommandFailure::translate(
            "arguments.nbtpath.nothing_found",
            vec![path_text.to_owned().into()],
        )),
        (Some(_), Some(_)) => Err(CommandFailure::translate(
            "commands.data.get.multiple",
            vec![],
        )),
    }
}

fn number_of(tag: &NbtTag) -> Option<f64> {
    match *tag {
        NbtTag::Byte(value) => Some(f64::from(value)),
        NbtTag::Short(value) => Some(f64::from(value)),
        NbtTag::Int(value) => Some(f64::from(value)),
        NbtTag::Long(value) => Some(value as f64),
        NbtTag::Float(value) => Some(f64::from(value)),
        NbtTag::Double(value) => Some(value),
        _ => None,
    }
}

fn result_of(value: &NbtTag) -> i32 {
    if let Some(number) = number_of(value) {
        return floor_to_int(number);
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
