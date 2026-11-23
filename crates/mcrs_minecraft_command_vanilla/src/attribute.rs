use crate::entity_view::EntityView;
use crate::select::{SelectionBuffers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, DoubleArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::ResourceArgument;
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Worlds,
};
use mcrs_minecraft_engine_mob_attributes::{AttributeEdit, MobAttributes};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeBases, AttributeInstances, AttributeSet, InstanceEdit,
};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_text::{Text, TranslateArg};

static NO_INSTANCES: AttributeSet = AttributeSet::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Get,
    BaseGet,
    BaseSet,
    BaseReset,
}

#[derive(Clone, Copy)]
pub(crate) struct AttributeCommand {
    target: ArgumentHandle<EntityArgument>,
    attribute: ArgumentHandle<ResourceArgument>,
    scale: ArgumentHandle<DoubleArgument>,
    base_scale: ArgumentHandle<DoubleArgument>,
    value: ArgumentHandle<DoubleArgument>,
}

struct Holder<'a> {
    entity: Entity,
    bases: AttributeBases,
    set: &'a AttributeSet,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<AttributeCommand, BuildError> {
    let root = registrar
        .literal(NodeId::ROOT, "attribute")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let target = registrar
        .arg(root, "target", EntityArgument::ONE_ENTITY)
        .id()?;
    let attribute = registrar
        .arg(
            target,
            "attribute",
            ResourceArgument {
                registry: rl!("minecraft:attribute"),
            },
        )
        .id()?;
    let get = registrar.literal(attribute, "get").runs(Branch::Get).id()?;
    let scale = registrar
        .arg(get, "scale", DoubleArgument::ANY)
        .runs(Branch::Get)
        .id()?;
    let base = registrar.literal(attribute, "base").id()?;
    let set = registrar.literal(base, "set").id()?;
    let value = registrar
        .arg(set, "value", DoubleArgument::ANY)
        .runs(Branch::BaseSet)
        .id()?;
    let base_get = registrar.literal(base, "get").runs(Branch::BaseGet).id()?;
    let base_scale = registrar
        .arg(base_get, "scale", DoubleArgument::ANY)
        .runs(Branch::BaseGet)
        .id()?;
    registrar
        .literal(base, "reset")
        .runs(Branch::BaseReset)
        .id()?;
    Ok(AttributeCommand {
        target,
        attribute,
        scale,
        base_scale,
        value,
    })
}

pub(crate) fn apply(worlds: &mut Worlds, edit: AttributeEdit) {
    let Some(mut messages) = worlds
        .world_mut()
        .get_resource_mut::<Messages<AttributeEdit>>()
    else {
        tracing::debug!(
            ?edit,
            "a command attribute edit finds no reader; it does nothing"
        );
        return;
    };
    messages.write(edit);
}

pub(crate) fn run(
    In(call): In<Call<Branch, AttributeCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut edits: ResMut<EffectQueue<AttributeEdit>>,
    mobs: Query<&MobAttributes>,
    players: Query<&AttributeInstances>,
    mut found: Local<SelectionBuffers>,
) -> CommandResult {
    let command = call.command;
    let location = context.arg(&call.invocation, command.attribute)?;
    let attribute = Attribute::get(location).ok_or_else(CommandFailure::failed)?;
    found.with(|targets| {
        select(&context, &call.invocation, command.target, targets)?;
        let view = targets
            .first()
            .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))?;
        let name = view.display_name();
        let Some(holder) = holder(view, &mobs, &players) else {
            return Err(CommandFailure::translate(
                "commands.attribute.failed.entity",
                vec![name.into()],
            ));
        };
        let attribute_name = Text::translate(
            format!("attribute.name.{}", attribute.location().path()),
            vec![],
        );
        let (Some(base), Some(value), Some(type_base)) = (
            holder.set.base(holder.bases, attribute),
            holder.set.value(holder.bases, attribute),
            holder.bases.base(attribute),
        ) else {
            return Err(CommandFailure::translate(
                "commands.attribute.failed.no_attribute",
                vec![name.into(), attribute_name.into()],
            ));
        };
        let (key, shown, result) = match call.branch {
            Branch::Get => {
                let scale = context.opt(&call.invocation, command.scale)?;
                let result = scaled(value, scale.unwrap_or(1.0));
                ("commands.attribute.value.get.success", value, result)
            }
            Branch::BaseGet => {
                let scale = context.opt(&call.invocation, command.base_scale)?;
                let result = scaled(base, scale.unwrap_or(1.0));
                ("commands.attribute.base_value.get.success", base, result)
            }
            Branch::BaseSet => {
                let new_base = context.arg(&call.invocation, command.value)?;
                let edit = edit(holder.entity, attribute, new_base);
                output.effect(&mut edits, edit);
                ("commands.attribute.base_value.set.success", new_base, 1)
            }
            Branch::BaseReset => {
                let edit = edit(holder.entity, attribute, type_base);
                output.effect(&mut edits, edit);
                ("commands.attribute.base_value.reset.success", type_base, 1)
            }
        };
        output.success(
            &call.invocation,
            Text::translate_args(
                key,
                vec![
                    attribute_name.into(),
                    name.into(),
                    TranslateArg::Number(NbtTag::Double(shown)),
                ],
            ),
            Feedback::Source,
        );
        Ok(result)
    })
}

fn holder<'a>(
    view: EntityView<'_>,
    mobs: &'a Query<&MobAttributes>,
    players: &'a Query<&AttributeInstances>,
) -> Option<Holder<'a>> {
    match view {
        EntityView::Player(player) => {
            let entity = player.entity?;
            let holder = match players.get(entity) {
                Ok(instances) => Holder {
                    entity,
                    bases: instances.bases(),
                    set: instances.set(),
                },
                Err(_) => Holder {
                    entity,
                    bases: mcrs_minecraft_entity::attribute_bases(EntityType::Player)?,
                    set: &NO_INSTANCES,
                },
            };
            Some(holder)
        }
        EntityView::Mob { view, .. } => Some(Holder {
            entity: view.entity,
            bases: mcrs_minecraft_entity::attribute_bases(view.entity_type())?,
            set: mobs
                .get(view.entity)
                .map_or(&NO_INSTANCES, MobAttributes::set),
        }),
    }
}

fn edit(target: Entity, attribute: Attribute, base: f64) -> AttributeEdit {
    AttributeEdit {
        target,
        attribute,
        edit: InstanceEdit::SetBase(base),
    }
}

fn scaled(number: f64, scale: f64) -> i32 {
    (number * scale) as i32
}
