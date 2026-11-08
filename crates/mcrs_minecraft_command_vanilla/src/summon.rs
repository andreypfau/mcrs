use crate::mob_view::display_name;
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{ResourceArgument, Vec3Argument};
use mcrs_minecraft_command_argument_vanilla::NbtCompoundArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Worlds,
};
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y};
use mcrs_minecraft_engine_physics::{MobExtras, MobSpawnRequest, SpawnSetup};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::registry_keys::ENTITY_TYPE;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_physics::{BirthKey, BirthSource, birth_uuid};
use mcrs_minecraft_text::Text;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct SummonCommand {
    entity: ArgumentHandle<ResourceArgument>,
    pos: ArgumentHandle<Vec3Argument>,
    nbt: ArgumentHandle<NbtCompoundArgument>,
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
    let nbt = registrar
        .arg(pos, "nbt", NbtCompoundArgument)
        .runs(())
        .id()?;
    Ok(SummonCommand { entity, pos, nbt })
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
    let failed = || CommandFailure::translate("commands.summon.failed", vec![]);
    let Some(kind) = MobType::of(entity_type) else {
        return Err(failed());
    };
    let (setup, extras) = match context.opt(&call.invocation, command.nbt)? {
        None => (SpawnSetup::First, MobExtras::default()),
        Some(text) => {
            let compound = text.read().map_err(|_| failed())?;
            (SpawnSetup::None, extras(compound).ok_or_else(failed)?)
        }
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
            setup,
            extras,
        },
    );
    output.success(
        &call.invocation,
        Text::translate(
            "commands.summon.success",
            vec![display_name(entity_type, Uuid::from_u128(uuid), None)],
        ),
        Feedback::SourceAndOperators,
    );
    Ok(1)
}

/// The keys of `<nbt>` that this server loads (TRK-SUM-003, TRK-DEV-002): `NoAI`, `CustomName`
/// as a string and `Tags`. `None` when the compound holds another key. A value with the wrong tag
/// keeps the default.
fn extras(compound: NbtCompound) -> Option<MobExtras> {
    let mut extras = MobExtras::default();
    for (key, tag) in compound {
        match (key.as_str(), tag) {
            ("NoAI", NbtTag::Byte(value)) => extras.no_ai = value != 0,
            ("CustomName", NbtTag::String(name)) => extras.custom_name = Some(name),
            ("Tags", NbtTag::List(tags)) => {
                extras.tags = tags
                    .into_iter()
                    .filter_map(|tag| match tag {
                        NbtTag::String(tag) => Some(tag),
                        _ => None,
                    })
                    .collect();
            }
            ("NoAI" | "CustomName" | "Tags", _) => {}
            _ => return None,
        }
    }
    Some(extras)
}

pub(crate) fn apply(worlds: &mut Worlds, request: MobSpawnRequest) {
    worlds.world_mut().write_message(request);
}
