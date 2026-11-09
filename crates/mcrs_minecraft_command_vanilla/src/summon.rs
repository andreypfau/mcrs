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
use mcrs_minecraft_engine::entity_births::EntityBirths;
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y};
use mcrs_minecraft_engine_physics::{MobExtras, MobSpawnRequest, SpawnSetup};
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::registry_keys::ENTITY_TYPE;
use mcrs_minecraft_entity_key::{BirthSequences, BirthSource};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_text::Text;
use mcrs_voxel::BlockPos;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct SummonCommand {
    entity: ArgumentHandle<ResourceArgument>,
    pos: ArgumentHandle<Vec3Argument>,
    nbt: ArgumentHandle<NbtCompoundArgument>,
}

/// The summons of the current tick of this world: the sequence of each pair of a spawn point and
/// a source block, and the UUIDs that the requests of the tick hold.
#[derive(Default)]
pub(crate) struct SummonOrder {
    tick: u64,
    sequences: BirthSequences,
    uuids: HashSet<u128>,
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
/// of its type. A summon without a UUID gives the mob a new one from the world seed, the
/// dimension, the birth clock, the block of the spawn point, the block of the source and the
/// sequence of that pair in the tick.
pub(crate) fn run(
    In(call): In<Call<(), SummonCommand>>,
    context: VanillaContext,
    births: EntityBirths,
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
    let Some(kind) = EntityTypeData::server_mob(entity_type) else {
        return Err(failed());
    };
    let (setup, Summoned { extras, uuid }) = match context.opt(&call.invocation, command.nbt)? {
        None => (SpawnSetup::First, Summoned::default()),
        Some(text) => {
            let compound = text.read().map_err(|_| failed())?;
            (SpawnSetup::None, read_keys(compound).ok_or_else(failed)?)
        }
    };
    if order.tick != births.tick() {
        *order = SummonOrder {
            tick: births.tick(),
            ..SummonOrder::default()
        };
    }
    if let Some(uuid) = uuid
        && taken(&mut order, &context, uuid)
    {
        return Err(CommandFailure::translate(
            "commands.summon.failed.uuid",
            vec![],
        ));
    }
    let birth = BirthSource::Command {
        spawn: BlockPos::from(position),
        source: BlockPos::from(source.position),
    };
    let sequence = order.sequences.next(births.tick(), &birth);
    let uuid = uuid.unwrap_or_else(|| births.born(&birth, sequence).0);
    order.uuids.insert(uuid);
    let name = display_name(
        entity_type,
        Uuid::from_u128(uuid),
        extras.custom_name.as_deref(),
    );
    output.effect(
        &mut spawns,
        MobSpawnRequest {
            kind,
            position,
            uuid,
            setup,
            extras,
        },
    );
    output.success(
        &call.invocation,
        Text::translate("commands.summon.success", vec![name]),
        Feedback::SourceAndOperators,
    );
    Ok(1)
}

/// The engine refuses a spawn whose UUID a mob or an earlier request holds, and the request of a
/// summon of this tick has no mob in the lookup yet. A UUID that the lookup finds leaves the
/// requests of the tick, so a clock that stands still cannot keep it taken after the mob is gone.
fn taken(order: &mut SummonOrder, context: &VanillaContext, uuid: u128) -> bool {
    if context.lookup().mob_by_uuid(uuid).is_some() {
        order.uuids.remove(&uuid);
        return true;
    }
    order.uuids.contains(&uuid)
}

/// What the keys of `<nbt>` set: the values of the new mob, and its UUID.
#[derive(Default)]
struct Summoned {
    extras: MobExtras,
    uuid: Option<u128>,
}

/// The keys of `<nbt>` that this server loads (TRK-SUM-003, TRK-DEV-002): `NoAI`, `CustomName`
/// as a string, `Tags`, `UUID` as four ints, the most significant first, and
/// `PersistenceRequired`, which changes nothing for a farm animal. `None` when the
/// compound holds another key. A value with the wrong tag keeps the default.
fn read_keys(compound: NbtCompound) -> Option<Summoned> {
    let mut summoned = Summoned::default();
    let extras = &mut summoned.extras;
    for (key, tag) in compound {
        match (key.as_str(), tag) {
            ("UUID", NbtTag::IntArray(words)) => {
                if let [a, b, c, d] = words[..] {
                    let word = |value: i32| u128::from(value.cast_unsigned());
                    summoned.uuid = Some(word(a) << 96 | word(b) << 64 | word(c) << 32 | word(d));
                }
            }
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
            // `PersistenceRequired` only keeps a mob from the removal steps of the despawn check,
            // which no farm animal takes (TRK-SUM-007).
            ("NoAI" | "CustomName" | "Tags" | "UUID" | "PersistenceRequired", _) => {}
            _ => return None,
        }
    }
    Some(summoned)
}

pub(crate) fn apply(worlds: &mut Worlds, request: MobSpawnRequest) {
    worlds.world_mut().write_message(request);
}
