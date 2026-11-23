use crate::entity_view::EntityView;
use crate::mob_view::MobView;
use crate::scale_text::scale_text;
use crate::select::{SelectionBuffers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, DoubleArgument, NodeId, StringArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument_entity::EntityArgument;
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

#[derive(Clone, Copy)]
pub(crate) struct DataCommand {
    target: ArgumentHandle<EntityArgument>,
    path: ArgumentHandle<StringArgument>,
    scale: ArgumentHandle<DoubleArgument>,
}

pub(crate) struct Query {
    invocation: Invocation,
    name: Text,
    path: Option<String>,
    scale: Option<f64>,
}

pub(crate) enum Subject {
    Player(Entity),
    Mob(NbtCompound),
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
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<DataCommand, BuildError> {
    let data = registrar
        .literal(NodeId::ROOT, "data")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let get = registrar.literal(data, "get").id()?;
    let entity = registrar.literal(get, "entity").id()?;
    let target = registrar
        .arg(entity, "target", EntityArgument::ONE_ENTITY)
        .runs(())
        .id()?;
    let path = registrar
        .arg(target, "path", StringArgument::Word)
        .runs(())
        .id()?;
    let scale = registrar
        .arg(path, "scale", DoubleArgument::ANY)
        .runs(())
        .id()?;
    Ok(DataCommand {
        target,
        path,
        scale,
    })
}

pub(crate) fn find(
    In(call): In<Call<(), DataCommand>>,
    context: VanillaContext,
    mut found: Local<SelectionBuffers>,
) -> Result<Found, CommandFailure> {
    let command = call.command;
    let path: Option<String> = context
        .opt(&call.invocation, command.path)?
        .map(|path| path.chars().collect());
    let scale = context.opt(&call.invocation, command.scale)?;
    found.with(|targets| {
        select(&context, &call.invocation, command.target, targets)?;
        let target = targets
            .first()
            .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))?;
        let subject = match target {
            EntityView::Player(player) => {
                Subject::Player(player.entity.ok_or_else(CommandFailure::failed)?)
            }
            EntityView::Mob { view, .. } => Subject::Mob(mob_data(view)),
        };
        let query = Query {
            invocation: call.invocation.clone(),
            name: target.display_name(),
            path,
            scale,
        };
        Ok(Found { query, subject })
    })
}

pub(crate) fn read(
    In(found): In<Result<Found, CommandFailure>>,
    world: &mut World,
) -> Result<Read, CommandFailure> {
    let Found { query, subject } = found?;
    let data = match subject {
        Subject::Player(player) => player_data(world, player)?,
        Subject::Mob(data) => data,
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
        name,
        path,
        scale,
    } = query;
    let Some(path) = path else {
        output.success(&invocation, query_message(name, &data), Feedback::Source);
        return Ok(1);
    };
    let value = single_value(&data, &path)?;
    let Some(scale) = scale else {
        output.success(&invocation, query_message(name, &value), Feedback::Source);
        return Ok(result_of(&value));
    };
    let number = number_of(&value).ok_or_else(|| {
        CommandFailure::translate("commands.data.get.invalid", vec![path.clone().into()])
    })?;
    let result = floor_to_int(number * scale);
    output.success(
        &invocation,
        Text::translate_args(
            "commands.data.entity.get",
            vec![
                path.into(),
                name.into(),
                scale_text(scale).into(),
                result.into(),
            ],
        ),
        Feedback::Source,
    );
    Ok(result)
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

fn query_message(name: Text, data: &NbtTag) -> Text {
    Text::translate(
        "commands.data.entity.query",
        vec![name, text_form(data).into_text()],
    )
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
