use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{ResourceArgument, Vec3Argument};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Worlds,
};
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y};
use mcrs_minecraft_engine_physics::{MobSpawnRequest, SpawnSetup};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::registry_keys::ENTITY_TYPE;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::{BirthKey, BirthSource, birth_uuid};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{HoverEvent, IntoText, Text};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct SummonCommand {
    entity: ArgumentHandle<ResourceArgument>,
    pos: ArgumentHandle<Vec3Argument>,
}

/// The command sequence of the summons of the current tick of this world: the order of the next
/// spawn request in the command effect buffer of the tick (TRK-LIFE-002).
#[derive(Default)]
pub(crate) struct SummonOrder {
    tick: u64,
    next: u32,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<SummonCommand, BuildError> {
    let summon = registrar
        .literal(NodeId::ROOT, "summon")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let entity = registrar
        .arg(
            summon,
            "entity",
            ResourceArgument {
                registry: ENTITY_TYPE.location(),
            },
        )
        .runs(())
        .id()?;
    let pos = registrar.arg(entity, "pos", Vec3Argument).runs(()).id()?;
    Ok(SummonCommand { entity, pos })
}

fn spawnable(position: DVec3) -> bool {
    let block = position.floor();
    (-MAX_XZ..MAX_XZ).contains(&block.x)
        && (-MAX_XZ..MAX_XZ).contains(&block.z)
        && (-MAX_Y..MAX_Y).contains(&block.y)
}

/// TRK-SUM-002: one mob of the type at the position, or at the source, with the first spawn setup
/// of its type. The birth key and the UUID come from the birth clock and the command sequence.
pub(crate) fn run(
    In(call): In<Call<(), SummonCommand>>,
    context: VanillaContext,
    clock: Res<BirthClock>,
    mut output: CommandOutput,
    mut spawns: ResMut<EffectQueue<MobSpawnRequest>>,
    mut order: Local<SummonOrder>,
) -> CommandResult {
    let command = call.command;
    let name = context.arg(&call.invocation, command.entity)?;
    let entity_type = EntityType::get(name).ok_or_else(CommandFailure::failed)?;
    if entity_type == EntityType::Player {
        return Err(CommandFailure::translate(
            "entity.not_summonable",
            vec![name.to_string().into()],
        ));
    }
    let source = context.source(&call.invocation)?;
    let position = match context.opt(&call.invocation, command.pos)? {
        Some(coordinates) => coordinates.position(&source),
        None => source.position,
    };
    if !spawnable(position) {
        return Err(CommandFailure::translate(
            "commands.summon.invalidPosition",
            vec![],
        ));
    }
    let Some(kind) = MobType::of(entity_type) else {
        return Err(CommandFailure::translate("commands.summon.failed", vec![]));
    };
    if order.tick != clock.0 {
        *order = SummonOrder {
            tick: clock.0,
            next: 0,
        };
    }
    let key = BirthKey::born(clock.0, &BirthSource::Command { order: order.next }, 0);
    order.next = order.next.wrapping_add(1);
    let uuid = birth_uuid(&key, None);
    output.effect(
        &mut spawns,
        MobSpawnRequest {
            kind,
            position,
            key,
            uuid,
            setup: SpawnSetup::First,
        },
    );
    output.success(
        &call.invocation,
        Text::translate(
            "commands.summon.success",
            vec![display_name(entity_type, Uuid::from_u128(uuid))],
        ),
        Feedback::SourceAndOperators,
    );
    Ok(1)
}

/// The display name of a mob with no custom name (TRK-SUM-004): the name of its type, with the
/// type and the UUID on hover and the UUID as the insertion.
fn display_name(entity_type: EntityType, uuid: Uuid) -> Text {
    let key = format!(
        "entity.{}.{}",
        entity_type.location().namespace(),
        entity_type.location().path()
    );
    let name = Text::translate(key.clone(), vec![]);
    let mut text = Text::translate(key, vec![]).insertion(uuid.hyphenated().to_string());
    text.style.hover_event = Some(HoverEvent::ShowEntity {
        id: ResourceLocationBuf::from(entity_type.location()),
        uuid,
        name: Some(name),
    });
    text
}

pub(crate) fn apply(worlds: &mut Worlds, request: MobSpawnRequest) {
    worlds.world_mut().write_message(request);
}
