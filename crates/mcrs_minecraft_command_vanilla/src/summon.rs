use crate::mob_view::display_name;
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::DVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{ResourceArgument, Vec3Argument};
use mcrs_minecraft_command_argument_vanilla::NbtCompoundArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Worlds,
};
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y};
use mcrs_minecraft_engine_physics::{MobExtras, MobSpawnRequest, SpawnSetup};
use mcrs_minecraft_engine_world::entity_births::EntityBirths;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::registry_keys::ENTITY_TYPE;
use mcrs_minecraft_entity_key::{BirthSequences, BirthSource, EntityUuid};
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::{ItemStack, read_saved};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_vanilla_item_entity::ItemSpawnRequest;
use mcrs_voxel::BlockPos;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct SummonCommand {
    entity: ArgumentHandle<ResourceArgument>,
    pos: ArgumentHandle<Vec3Argument>,
    nbt: ArgumentHandle<NbtCompoundArgument>,
}

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

#[derive(SystemParam)]
pub(crate) struct Spawns<'w> {
    mobs: ResMut<'w, EffectQueue<MobSpawnRequest>>,
    items: ResMut<'w, EffectQueue<ItemSpawnRequest>>,
    table: Option<Res<'w, ItemTable>>,
}

enum Summon {
    Mob {
        kind: &'static EntityTypeData,
        setup: SpawnSetup,
        extras: MobExtras,
    },
    Item(SummonedItem),
}

fn server_mob(entity_type: EntityType) -> Option<&'static EntityTypeData> {
    match entity_type {
        EntityType::Pig
        | EntityType::Cow
        | EntityType::Sheep
        | EntityType::Chicken
        | EntityType::ZombifiedPiglin => EntityTypeData::of(entity_type),
        _ => None,
    }
}

fn read_summon(
    entity_type: EntityType,
    nbt: Option<NbtCompound>,
    table: Option<&ItemTable>,
) -> Option<(Summon, Option<u128>)> {
    if entity_type == EntityType::Item {
        let item = read_item_keys(nbt?, table?)?;
        let uuid = item.uuid;
        return Some((Summon::Item(item), uuid));
    }
    let kind = server_mob(entity_type)?;
    Some(match nbt {
        None => (
            Summon::Mob {
                kind,
                setup: SpawnSetup::First,
                extras: MobExtras::default(),
            },
            None,
        ),
        Some(compound) => {
            let Summoned { extras, uuid } = read_keys(compound)?;
            (
                Summon::Mob {
                    kind,
                    setup: SpawnSetup::None,
                    extras,
                },
                uuid,
            )
        }
    })
}

pub(crate) fn run(
    In(call): In<Call<(), SummonCommand>>,
    context: VanillaContext,
    births: EntityBirths,
    mut output: CommandOutput,
    mut spawns: Spawns,
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
    let nbt = context
        .opt(&call.invocation, command.nbt)?
        .map(|text| text.read())
        .transpose()
        .map_err(|_| failed())?;
    let (summon, uuid) =
        read_summon(entity_type, nbt, spawns.table.as_deref()).ok_or_else(failed)?;
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
    let uuid = uuid.unwrap_or_else(|| births.born(&birth, sequence).as_u128());
    order.uuids.insert(uuid);
    let custom_name = match &summon {
        Summon::Mob { extras, .. } => extras.custom_name.as_deref(),
        Summon::Item(_) => None,
    };
    let name = display_name(entity_type, Uuid::from_u128(uuid), custom_name);
    match summon {
        Summon::Item(item) => output.effect(
            &mut spawns.items,
            ItemSpawnRequest {
                stack: item.stack,
                position,
                key: EntityUuid::from_u128(uuid),
                values: item.values,
                owner: item.owner,
                thrower: item.thrower,
            },
        ),
        Summon::Mob {
            kind,
            setup,
            extras,
        } => output.effect(
            &mut spawns.mobs,
            MobSpawnRequest {
                kind,
                position,
                uuid,
                setup,
                extras,
            },
        ),
    }
    output.success(
        &call.invocation,
        Text::translate("commands.summon.success", vec![name]),
        Feedback::SourceAndOperators,
    );
    Ok(1)
}

fn taken(order: &mut SummonOrder, context: &VanillaContext, uuid: u128) -> bool {
    if context.lookup().mob_by_uuid(uuid).is_some() {
        order.uuids.remove(&uuid);
        return true;
    }
    order.uuids.contains(&uuid)
}

#[derive(Default)]
struct Summoned {
    extras: MobExtras,
    uuid: Option<u128>,
}

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
            ("OnGround", NbtTag::Byte(value)) => extras.on_ground = value != 0,
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
            ("NoAI" | "OnGround" | "CustomName" | "Tags" | "UUID" | "PersistenceRequired", _) => {}
            _ => return None,
        }
    }
    Some(summoned)
}

pub(crate) fn apply(worlds: &mut Worlds, request: MobSpawnRequest) {
    worlds.world_mut().write_message(request);
}

pub(crate) fn apply_item(worlds: &mut Worlds, request: ItemSpawnRequest) {
    let world = worlds.world_mut();
    if world.contains_resource::<Messages<ItemSpawnRequest>>() {
        world.write_message(request);
    }
}

struct SummonedItem {
    stack: ItemStack,
    values: ItemEntity,
    owner: Option<u128>,
    thrower: Option<u128>,
    uuid: Option<u128>,
}

fn read_item_keys(compound: NbtCompound, table: &ItemTable) -> Option<SummonedItem> {
    let mut stack = None;
    let mut values = ItemEntity::default();
    let (mut owner, mut thrower, mut uuid) = (None, None, None);
    for (key, tag) in compound {
        match (key.as_str(), tag) {
            ("Item", NbtTag::Compound(saved)) => stack = Some(read_saved(&saved, table).ok()?),
            ("Age", NbtTag::Short(age)) => values.age = age,
            ("PickupDelay", NbtTag::Short(delay)) => values.pickup_delay = delay,
            ("Health", NbtTag::Short(health)) => values.health = health,
            ("Owner", NbtTag::IntArray(words)) => owner = uuid_of(&words),
            ("Thrower", NbtTag::IntArray(words)) => thrower = uuid_of(&words),
            ("UUID", NbtTag::IntArray(words)) => uuid = uuid_of(&words),
            ("Item" | "Age" | "PickupDelay" | "Health" | "Owner" | "Thrower" | "UUID", _) => {}
            _ => return None,
        }
    }
    Some(SummonedItem {
        stack: stack?,
        values,
        owner,
        thrower,
        uuid,
    })
}

fn uuid_of(words: &[i32]) -> Option<u128> {
    let [a, b, c, d] = words[..] else {
        return None;
    };
    let word = |value: i32| u128::from(value.cast_unsigned());
    Some(word(a) << 96 | word(b) << 64 | word(c) << 32 | word(d))
}
