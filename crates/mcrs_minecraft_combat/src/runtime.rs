use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryData;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::BlockStateFlags;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_damage::{DamageSource, DamageTypeFlags, DamageTypeRules, Hit};
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode};
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget, to, to_target,
};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::persistence::LoadedPlayerDat;
use mcrs_minecraft_engine::teleport::{TeleportState, position_packet, teleport_to};
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_item::{HolderCommandsExt, Selected, Slots};
use mcrs_minecraft_engine_physics::{Mob, MobChanges, Mobs, hurt_mob};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::block_drops::ExperienceDropped;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform, Velocity};
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_engine_world::entity_ids::GlobalEntityId;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::world::dimension::{
    Dimension, DimensionTypeConfig, DimensionTypeId, InDimension,
};
use mcrs_minecraft_engine_world::world::storage::view::{DimensionView, WorldView};
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::inventory::slots;
use mcrs_minecraft_item::{Items, ProtoStack};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{
    AttributeModifiers, Count, ItemStack as EngineStack, Kind, ModifierOperation, NameLookup,
    NameTable, SlotGroup,
};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAttack, ServerboundChangeDifficulty, ServerboundPlayerAction,
    ServerboundPlayerInput, ServerboundSetCarriedItem, ServerboundUseItemOn,
};
use mcrs_minecraft_registries::{CurrentRegistries, WireId};

use crate::CombatSet;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{
    AttributeSnapshot, ClientboundDamageEvent, ClientboundEntityEvent, ClientboundHurtAnimation,
    ClientboundPlayerCombatKill, ClientboundRemoveMobEffect, ClientboundRespawn,
    ClientboundSetEntityData, ClientboundSetEntityMotion, ClientboundSetEquipment,
    ClientboundSetExperience, ClientboundSetHealth, ClientboundUpdateAttributes,
    ClientboundUpdateMobEffect,
};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol_game::entity_data_index;
use mcrs_minecraft_protocol_game::item::ItemStackValue;
use mcrs_minecraft_protocol_game::player::{PlayerAction, PlayerSpawnInfo};
use mcrs_minecraft_protocol_game::serverbound::{
    ClientCommandAction, ServerboundClientCommand, ServerboundMovePlayerPos,
    ServerboundMovePlayerPosRot, ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_save::read_level_dat;
use mcrs_minecraft_text::{RawText, Text};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::damage_rules::DamageTypeRulesResource;
use crate::damage_types;
use crate::effects::{ActiveEffect, ActiveEffects, MobEffect};
use crate::environment::{
    AirSupply, FireState, FreezeState, WorldBorderState, compute_fall_damage, is_in_void,
};
use crate::equipment::{EntityEquipment, EquipmentChangedEvent};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
use crate::experience::{ExperienceOrb, PlayerExperience};
use crate::explosions::IgniteTntRequest;
use crate::health::{
    Armor, ArmorToughness, AttackDamage, DamageCooldown, Health, KnockbackResistance,
};
use crate::hunger::{CombatDifficulty, FoodData};
use crate::life_cycle::DeathState;
use crate::position::CombatPosition;
use crate::projectiles::{Projectile, ProjectileKind};

#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct CombatVitals {
    last_y: f64,
    fall_distance: f64,
    on_ground: bool,
    lava_ticks: u32,
    hot_floor_ticks: u32,
    cactus_ticks: u32,
    berry_ticks: u32,
    suffocation_ticks: u32,
    border_ticks: u32,
    respawn_at: Transform,
}

#[derive(Component, Clone, Debug, PartialEq)]
struct PlayerRespawnPoint {
    dimension: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    position: DVec3,
    yaw: f32,
    pitch: f32,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlayerInputState {
    sneaking: bool,
    sprinting: bool,
    jumping: bool,
}

impl CombatVitals {
    fn at(transform: Transform) -> Self {
        Self {
            last_y: transform.translation.y,
            fall_distance: 0.0,
            on_ground: false,
            lava_ticks: 0,
            hot_floor_ticks: 0,
            cactus_ticks: 0,
            berry_ticks: 0,
            suffocation_ticks: 0,
            border_ticks: 0,
            respawn_at: transform,
        }
    }
}

#[derive(Component, Debug, Default)]
struct EnvironmentSync {
    air: Option<i32>,
    fire: Option<bool>,
    frozen: Option<i32>,
}

#[derive(Component, Debug, Default)]
struct CombatSync {
    health: Option<f32>,
    food: Option<(u32, f32)>,
    experience: Option<(i32, u32, i32)>,
    effects: Vec<(MobEffect, u8, u32, u8)>,
    attributes: Option<(u32, u32, u32, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CombatModifier {
    attribute: u16,
    amount: f64,
    operation: ModifierOperation,
    slot: SlotGroup,
}

fn loaded_inventory_values(
    loaded: &LoadedPlayerDat,
    table: Option<&ItemTable>,
) -> Vec<Option<EngineStack>> {
    let mut values = vec![None; usize::from(slots::CARRIED) + 1];
    let Some(table) = table else {
        return values;
    };
    for entry in &loaded.inventory {
        let Some(slot) = values.get_mut(usize::from(entry.slot)) else {
            continue;
        };
        let Some(id) = table.id(NameTable::Item, entry.stack.item.location().into()) else {
            continue;
        };
        let Ok(count) = u8::try_from(entry.stack.count.0) else {
            continue;
        };
        let Some(count) = Count::new(count) else {
            continue;
        };
        *slot = Some(EngineStack::new(Kind(id), count));
    }
    values
}

fn slot_group_applies(group: SlotGroup, slot: EquipmentSlot) -> bool {
    match group {
        SlotGroup::Any => true,
        SlotGroup::Mainhand => slot == EquipmentSlot::MainHand,
        SlotGroup::Offhand => slot == EquipmentSlot::OffHand,
        SlotGroup::Hand => matches!(slot, EquipmentSlot::MainHand | EquipmentSlot::OffHand),
        SlotGroup::Feet => slot == EquipmentSlot::Feet,
        SlotGroup::Legs => slot == EquipmentSlot::Legs,
        SlotGroup::Chest => slot == EquipmentSlot::Chest,
        SlotGroup::Head => slot == EquipmentSlot::Head,
        SlotGroup::Armor => matches!(
            slot,
            EquipmentSlot::Feet | EquipmentSlot::Legs | EquipmentSlot::Chest | EquipmentSlot::Head
        ),
        SlotGroup::Body => slot == EquipmentSlot::Body,
        SlotGroup::Saddle => slot == EquipmentSlot::Saddle,
    }
}

fn mob_effect_by_name(name: &str) -> Option<MobEffect> {
    (0..=40)
        .filter_map(MobEffect::from_id)
        .find(|effect| effect.as_str() == name)
}

fn note_ground_pos(
    on: On<Received<ServerboundMovePlayerPos>>,
    mut players: Query<&mut CombatVitals>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_pos_rot(
    on: On<Received<ServerboundMovePlayerPosRot>>,
    mut players: Query<&mut CombatVitals>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_rot(
    on: On<Received<ServerboundMovePlayerRot>>,
    mut players: Query<&mut CombatVitals>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_status(
    on: On<Received<ServerboundMovePlayerStatusOnly>>,
    mut players: Query<&mut CombatVitals>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn on_client_command(
    on: On<Received<ServerboundClientCommand>>,
    mut requests: MessageWriter<RespawnRequest>,
) {
    if on
        .packet()
        .is_some_and(|packet| packet.action == ClientCommandAction::PerformRespawn)
    {
        requests.write(RespawnRequest { player: on.entity });
    }
}

fn on_change_difficulty(
    on: On<Received<ServerboundChangeDifficulty>>,
    mut difficulty: ResMut<CombatDifficulty>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    difficulty.0 = packet.difficulty;
}

fn on_player_input(
    on: On<Received<ServerboundPlayerInput>>,
    mut players: Query<&mut PlayerInputState>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    if let Ok(mut input) = players.get_mut(on.entity) {
        input.sneaking = packet.input.shift();
        input.sprinting = packet.input.sprint();
        input.jumping = packet.input.jump();
    }
}

fn on_player_action(
    on: On<Received<ServerboundPlayerAction>>,
    mut players: Query<(Option<&PlayerGameMode>, Option<&mut FoodData>)>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    if !matches!(packet.action, PlayerAction::StopDestroyBlock) {
        return;
    }
    if let Ok((mode, food)) = players.get_mut(on.entity)
        && mode.is_none_or(|mode| mode.0 != GameMode::Creative)
        && let Some(mut food) = food
    {
        food.add_exhaustion(0.005);
    }
}

fn on_set_carried_item(
    on: On<Received<ServerboundSetCarriedItem>>,
    mut players: Query<(&mut Selected, &mut LoadedPlayerDat)>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    let Ok((mut selected, mut loaded)) = players.get_mut(on.entity) else {
        return;
    };
    let slot = u8::try_from(packet.slot).unwrap_or(0).min(8);
    selected.0 = slot;
    loaded.selected_item_slot = i32::from(slot);
}

fn on_player_use_item_on(
    on: On<Received<ServerboundUseItemOn>>,
    players: Query<(Entity, &InDimension, &Slots, &Selected)>,
    stacks: Query<&Kind>,
    items: Option<Res<Items>>,
    mut ignite: MessageWriter<IgniteTntRequest>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    let Ok((player, dimension, slots, selected)) = players.get(on.entity) else {
        return;
    };
    let Some(items) = items else {
        return;
    };
    let slot = match packet.hand {
        mcrs_minecraft_protocol_game::Hand::Main => usize::from(selected.0),
        mcrs_minecraft_protocol_game::Hand::Off => usize::from(slots::OFFHAND),
    };
    let Some(kind) = slots
        .get(slot)
        .and_then(|stack| stacks.get(stack).ok())
        .map(|kind| kind.0)
    else {
        return;
    };
    let Some(id) = mcrs_minecraft_resource::Id::<Item>::checked(kind, items.len()) else {
        return;
    };
    let Some(entry) = items.get(id) else {
        return;
    };
    if !matches!(
        entry.identifier.as_str(),
        "minecraft:flint_and_steel" | "minecraft:fire_charge"
    ) {
        return;
    }
    ignite.write(IgniteTntRequest {
        dimension: dimension.0,
        pos: packet.block_pos,
        owner: Some(player),
    });
}

fn spawn_block_experience(mut commands: Commands, mut dropped: MessageReader<ExperienceDropped>) {
    for drop in dropped.read() {
        let position = drop.at.as_dvec3() + DVec3::splat(0.5);
        commands.spawn((
            ExperienceOrb::new(drop.amount),
            CombatPosition(position),
            Transform::from_translation(position),
        ));
    }
}

fn tick_player_hunger(
    mut players: Query<(Entity, &mut Health, &mut FoodData), (With<Player>, Without<DeathState>)>,
    rules: Option<Res<GameRules>>,
    difficulty: Option<Res<CombatDifficulty>>,
    mut damage: MessageWriter<DamageRequest>,
) {
    let natural_regeneration = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::NaturalHealthRegeneration));
    for (entity, mut health, mut food) in &mut players {
        let difficulty = difficulty
            .as_deref()
            .map_or(mcrs_minecraft_protocol_game::Difficulty::Normal, |value| {
                value.0
            });
        let (heal, starve) =
            food.tick(health.current, health.max, natural_regeneration, difficulty);
        if heal > 0.0 {
            health.heal(heal);
        }
        if starve > 0.0 && health.is_alive() {
            damage.write(DamageRequest::new(
                entity,
                Hit::from_source(starve, DamageSource::new(damage_types::STARVE)),
            ));
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the environmental damage sources of one player are checked in one place"
)]
fn tick_player_environment(
    mut players: Query<
        (
            Entity,
            &Transform,
            &InDimension,
            Option<&PlayerGameMode>,
            Option<&PlayerFlying>,
            Option<&ActiveEffects>,
            Option<&PlayerInputState>,
            Option<&Velocity>,
            &mut Health,
            &mut FoodData,
            &mut AirSupply,
            &mut FireState,
            &mut FreezeState,
            &mut CombatVitals,
        ),
        (With<Player>, Without<DeathState>),
    >,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    dimensions: Query<&DimensionTypeConfig>,
    rules: Option<Res<GameRules>>,
    border: Option<Res<WorldBorderState>>,
    mut damage: MessageWriter<DamageRequest>,
) {
    let fall_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::FallDamage));
    let fire_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::FireDamage));
    let drowning_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::DrowningDamage));
    let freeze_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::FreezeDamage));
    let dimensions = &dimensions;

    for (
        entity,
        transform,
        in_dimension,
        mode,
        flying,
        effects,
        input,
        velocity,
        health,
        mut food,
        mut air,
        mut fire,
        mut freeze,
        mut vitals,
    ) in &mut players
    {
        if !health.is_alive() {
            continue;
        }
        let mode = mode.map_or(GameMode::Survival, |mode| mode.0);
        if mode == GameMode::Spectator {
            continue;
        }
        let vulnerable = mode != GameMode::Creative;
        let sprinting = input.is_some_and(|input| input.sprinting);

        let position = transform.translation;
        let feet = BlockPos::new(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        );
        let eye = BlockPos::new(
            position.x.floor() as i32,
            (position.y + 1.62).floor() as i32,
            position.z.floor() as i32,
        );
        let below = BlockPos::new(feet.x, feet.y.saturating_sub(1), feet.z);

        let mut in_water = false;
        let mut in_lava = false;
        let mut in_fire = false;
        let mut in_powder_snow = false;
        let mut in_cactus = false;
        let mut in_berry_bush = false;
        let mut in_wall = false;
        let mut on_magma = false;
        let view = world.get(in_dimension.0);
        if let (Some(definitions), Some(view)) = (definitions.as_deref(), view.as_ref()) {
            in_water = is_water(view, definitions, feet) || is_water(view, definitions, eye);
            in_lava = is_lava(view, definitions, feet) || is_lava(view, definitions, eye);
            in_fire = is_block(view, definitions, feet, "minecraft:fire")
                || is_block(view, definitions, feet, "minecraft:soul_fire");
            in_powder_snow = is_block(view, definitions, feet, "minecraft:powder_snow")
                || is_block(view, definitions, eye, "minecraft:powder_snow");
            on_magma = is_block(view, definitions, below, "minecraft:magma_block");
            in_cactus = touches_cactus(view, definitions, position);
            in_berry_bush = is_block(view, definitions, feet, "minecraft:sweet_berry_bush");
            in_wall = view.block(eye).ok().is_some_and(|state| {
                definitions
                    .state(state)
                    .flags
                    .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
                    && definitions.state(state).fluid.is_none()
            });
        }

        if let Some(velocity) = velocity
            && velocity.0.x.hypot(velocity.0.z) > 0.1
        {
            if sprinting {
                food.add_exhaustion(0.1);
            } else if in_water {
                food.add_exhaustion(0.01);
            }
        }
        if input.is_some_and(|input| input.jumping) && vitals.on_ground {
            food.add_exhaustion(if sprinting { 0.2 } else { 0.05 });
        }

        if vulnerable && fall_damage && !flying.is_some_and(|flying| flying.0) {
            let dy = position.y - vitals.last_y;
            if dy.abs() > 8.0 {
                vitals.fall_distance = 0.0;
            } else if dy < 0.0 && !in_water && !in_lava && !in_powder_snow {
                vitals.fall_distance += -dy;
            }
            if vitals.on_ground {
                let multiplier = fall_multiplier(view.as_ref(), definitions.as_deref(), below);
                let fall = compute_fall_damage(vitals.fall_distance, 3.0, multiplier);
                if fall > 0.0 {
                    damage.write(DamageRequest::new(
                        entity,
                        Hit::from_source(fall, DamageSource::new(damage_types::FALL)),
                    ));
                }
                vitals.fall_distance = 0.0;
            }
        } else {
            vitals.fall_distance = 0.0;
        }
        vitals.last_y = position.y;

        let fire_resistant = effects.is_some_and(|effects| effects.has(MobEffect::FireResistance));
        if in_water {
            fire.extinguish();
            vitals.lava_ticks = 0;
        }
        if let Ok(config) = dimensions.get(in_dimension.0)
            && is_in_void(position.y, f64::from(config.min_y))
        {
            damage.write(DamageRequest::new(
                entity,
                Hit::from_source(4.0, DamageSource::new(damage_types::OUT_OF_WORLD)),
            ));
        }
        if !vulnerable {
            vitals.lava_ticks = 0;
            continue;
        }
        if in_water && !effects.is_some_and(|effects| effects.has(MobEffect::WaterBreathing)) {
            if drowning_damage && air.tick_underwater() {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(2.0, DamageSource::new(damage_types::DROWN)),
                ));
            }
        } else {
            air.tick_in_air();
        }
        if in_lava {
            if fire.remaining_fire_ticks <= 0 {
                fire.ignite(300);
            }
            vitals.lava_ticks += 1;
            if !fire_resistant && vitals.lava_ticks.is_multiple_of(10) {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(4.0, DamageSource::new(damage_types::LAVA)),
                ));
            }
        } else {
            vitals.lava_ticks = 0;
        }
        if in_fire && fire.remaining_fire_ticks <= 0 {
            fire.ignite(160);
        }
        if fire.tick() && fire_damage && !fire_resistant {
            damage.write(DamageRequest::new(
                entity,
                Hit::from_source(1.0, DamageSource::new(damage_types::ON_FIRE)),
            ));
        }
        if in_powder_snow {
            if freeze_damage && freeze.tick_in_powder_snow() {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(damage_types::FREEZE)),
                ));
            }
        } else {
            freeze.tick_warm();
        }
        if in_cactus {
            vitals.cactus_ticks += 1;
            if vitals.cactus_ticks.is_multiple_of(10) {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(damage_types::CACTUS)),
                ));
            }
        } else {
            vitals.cactus_ticks = 0;
        }
        if in_wall {
            vitals.suffocation_ticks += 1;
            if vitals.suffocation_ticks.is_multiple_of(10) {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(damage_types::IN_WALL)),
                ));
            }
        } else {
            vitals.suffocation_ticks = 0;
        }
        if in_berry_bush
            && velocity.is_some_and(|velocity| {
                velocity.0.x * velocity.0.x + velocity.0.z * velocity.0.z > 0.003
            })
        {
            vitals.berry_ticks += 1;
            if vitals.berry_ticks.is_multiple_of(10) {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(damage_types::SWEET_BERRY_BUSH)),
                ));
            }
        } else {
            vitals.berry_ticks = 0;
        }
        if on_magma && !input.is_some_and(|input| input.sneaking) {
            vitals.hot_floor_ticks += 1;
            if vitals.hot_floor_ticks.is_multiple_of(10) {
                damage.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(damage_types::HOT_FLOOR)),
                ));
            }
        } else {
            vitals.hot_floor_ticks = 0;
        }
        if let Some(border) = border.as_deref() {
            let border_damage = border.compute_damage(position);
            if border_damage > 0.0 {
                vitals.border_ticks = vitals.border_ticks.saturating_add(1);
                if vitals.border_ticks.is_multiple_of(10) {
                    damage.write(DamageRequest::new(
                        entity,
                        Hit::from_source(
                            border_damage,
                            DamageSource::new(damage_types::OUTSIDE_BORDER),
                        ),
                    ));
                }
            } else {
                vitals.border_ticks = 0;
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: the rows, the world view, the rules and the damage queue are separate parameters"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the environmental damage sources of one mob row are checked in one place"
)]
fn tick_mob_environment(
    rows: Option<ResMut<EntityRows>>,
    changes: Option<ResMut<MobChanges>>,
    clock: Option<Res<BirthClock>>,
    world: WorldView,
    dimensions: Query<Entity, With<Dimension>>,
    dimension_types: Query<&DimensionTypeConfig>,
    definitions: Option<Res<Blocks>>,
    rules: Option<Res<GameRules>>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
) {
    let (Some(mut rows), Some(mut changes)) = (rows, changes) else {
        return;
    };
    let Ok(dimension) = dimensions.single() else {
        return;
    };
    let Some(view) = world.get(dimension) else {
        return;
    };
    let Some(definitions) = definitions.as_deref() else {
        return;
    };
    let fire_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::FireDamage));
    let drowning_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::DrowningDamage));
    let freeze_damage = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::FreezeDamage));
    let min_y = dimension_types
        .get(dimension)
        .map(|config| f64::from(config.min_y))
        .unwrap_or(-64.0);
    let flags = |damage_type: u32| {
        type_rules
            .as_deref()
            .map_or(DamageTypeFlags::builtin_for(damage_type), |rules| {
                rules.0.flags(damage_type)
            })
    };
    let tick = clock.as_deref().map_or(0, |clock| clock.0);
    let places: Vec<Placed> = rows
        .living_rows()
        .filter(|(_, state)| state.living.is_some_and(|living| living.is_alive()))
        .map(|(at, _)| at)
        .collect();
    let mut hits: Vec<(Placed, Hit)> = Vec::new();
    for at in places {
        let Some(state) = rows.state_mut(at) else {
            continue;
        };
        let Some(living) = state.living.as_mut() else {
            continue;
        };
        let position = state.body.position;
        let feet = BlockPos::new(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        );
        let eye = BlockPos::new(
            position.x.floor() as i32,
            (position.y + 1.62).floor() as i32,
            position.z.floor() as i32,
        );
        let in_water = is_water(&view, definitions, feet) || is_water(&view, definitions, eye);
        let in_lava = is_lava(&view, definitions, feet) || is_lava(&view, definitions, eye);
        let in_fire = is_block(&view, definitions, feet, "minecraft:fire")
            || is_block(&view, definitions, feet, "minecraft:soul_fire");
        let in_powder_snow = is_block(&view, definitions, feet, "minecraft:powder_snow")
            || is_block(&view, definitions, eye, "minecraft:powder_snow");
        let in_cactus = touches_cactus(&view, definitions, position);
        let in_berry_bush = is_block(&view, definitions, feet, "minecraft:sweet_berry_bush");
        let in_wall = view.block(eye).ok().is_some_and(|state| {
            definitions
                .state(state)
                .flags
                .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
                && definitions.state(state).fluid.is_none()
        });

        if in_water {
            living.fire_ticks = 0;
            living.air -= 1;
            if in_lava {
                living.fire_ticks = 300;
            }
        } else if living.air < 300 {
            living.air = (living.air + 4).min(300);
        }
        if in_lava && !living.fire_immune {
            if living.fire_ticks <= 0 {
                living.fire_ticks = 300;
            }
        } else if in_fire && !living.fire_immune && living.fire_ticks <= 0 {
            living.fire_ticks = 160;
        }
        if living.fire_ticks > 0 {
            living.fire_ticks -= 1;
            living.on_fire = true;
            if fire_damage && living.fire_ticks % 20 == 0 {
                hits.push((
                    at,
                    Hit::from_source(1.0, DamageSource::new(damage_types::ON_FIRE))
                        .with_flags(flags(damage_types::ON_FIRE)),
                ));
            }
        } else {
            living.on_fire = false;
        }
        if in_lava && !living.fire_immune {
            hits.push((
                at,
                Hit::from_source(4.0, DamageSource::new(damage_types::LAVA))
                    .with_flags(flags(damage_types::LAVA)),
            ));
        }
        if in_water && living.air <= -20 {
            living.air = 0;
            if drowning_damage {
                hits.push((
                    at,
                    Hit::from_source(2.0, DamageSource::new(damage_types::DROWN))
                        .with_flags(flags(damage_types::DROWN)),
                ));
            }
        }
        if in_powder_snow {
            living.frozen_ticks = living.frozen_ticks.saturating_add(1).min(300);
            if freeze_damage && living.frozen_ticks >= 140 && (living.frozen_ticks - 140) % 40 == 0
            {
                hits.push((
                    at,
                    Hit::from_source(1.0, DamageSource::new(damage_types::FREEZE))
                        .with_flags(flags(damage_types::FREEZE)),
                ));
            }
        } else if living.frozen_ticks > 0 {
            living.frozen_ticks = (living.frozen_ticks - 2).max(0);
        }
        if in_cactus {
            hits.push((
                at,
                Hit::from_source(1.0, DamageSource::new(damage_types::CACTUS))
                    .with_flags(flags(damage_types::CACTUS)),
            ));
        }
        if in_wall {
            hits.push((
                at,
                Hit::from_source(1.0, DamageSource::new(damage_types::IN_WALL))
                    .with_flags(flags(damage_types::IN_WALL)),
            ));
        }
        if in_berry_bush
            && state.body.velocity.x * state.body.velocity.x
                + state.body.velocity.z * state.body.velocity.z
                > 0.003
        {
            hits.push((
                at,
                Hit::from_source(1.0, DamageSource::new(damage_types::SWEET_BERRY_BUSH))
                    .with_flags(flags(damage_types::SWEET_BERRY_BUSH)),
            ));
        }
        if position.y < min_y - 64.0 {
            hits.push((
                at,
                Hit::from_source(4.0, DamageSource::new(damage_types::OUT_OF_WORLD))
                    .with_flags(flags(damage_types::OUT_OF_WORLD)),
            ));
        }
    }
    for (at, hit) in hits {
        hurt_mob(&mut rows, &mut changes, at, hit, tick);
    }
}

fn fall_multiplier(
    view: Option<&DimensionView<'_>>,
    blocks: Option<&Blocks>,
    below: BlockPos,
) -> f32 {
    let Some((view, blocks)) = view.zip(blocks) else {
        return 1.0;
    };
    let Some(owner) = block_state(view, below).and_then(|state| blocks.owner(state)) else {
        return 1.0;
    };
    let name = owner.identifier.as_str();
    match name {
        "minecraft:slime_block" => 0.0,
        "minecraft:honey_block" | "minecraft:hay_block" => 0.2,
        _ if name.ends_with("_bed") => 0.5,
        _ => 1.0,
    }
}

fn block_state(view: &DimensionView<'_>, at: BlockPos) -> Option<BlockStateId> {
    view.block(at).ok()
}

fn is_water(view: &DimensionView<'_>, blocks: &Blocks, at: BlockPos) -> bool {
    block_state(view, at)
        .and_then(|state| blocks.state(state).fluid)
        .is_some_and(|fluid| fluid.fluid == Fluid::Water.id())
}

fn is_lava(view: &DimensionView<'_>, blocks: &Blocks, at: BlockPos) -> bool {
    block_state(view, at)
        .and_then(|state| blocks.state(state).fluid)
        .is_some_and(|fluid| fluid.fluid == Fluid::Lava.id())
}

fn is_block(view: &DimensionView<'_>, blocks: &Blocks, at: BlockPos, name: &str) -> bool {
    block_state(view, at)
        .and_then(|state| blocks.owner(state))
        .is_some_and(|owner| owner.identifier.as_str() == name)
}

fn touches_cactus(view: &DimensionView<'_>, blocks: &Blocks, position: bevy_math::DVec3) -> bool {
    let min_y = position.y.floor() as i32;
    for y in min_y..=min_y + 1 {
        for x in [position.x - 0.3, position.x + 0.3] {
            for z in [position.z - 0.3, position.z + 0.3] {
                let at = BlockPos::new(x.floor() as i32, y, z.floor() as i32);
                if is_block(view, blocks, at, "minecraft:cactus") {
                    return true;
                }
            }
        }
    }
    false
}

#[expect(
    clippy::too_many_lines,
    reason = "the loaded player data of one joined player is decoded in one place"
)]
fn attach_player_combat(
    mut commands: Commands,
    mut players: Query<
        (Entity, &Transform, Option<&LoadedPlayerDat>),
        (With<Player>, Without<Health>),
    >,
    table: Option<Res<ItemTable>>,
) {
    for (entity, transform, loaded) in &mut players {
        let loaded = loaded.cloned().unwrap_or_default();
        commands
            .entity(entity)
            .load_inventory(loaded_inventory_values(&loaded, table.as_deref()));
        let selected = u8::try_from(loaded.selected_item_slot).unwrap_or(0).min(8);
        let rest = &loaded.rest;
        let health_value = rest
            .get("Health")
            .and_then(number_float)
            .filter(|health| health.is_finite())
            .unwrap_or(20.0)
            .max(0.0);
        let food_level = number_int(rest, "foodLevel").unwrap_or(20).clamp(0, 20) as u32;
        let saturation: f32 = rest
            .get("foodSaturationLevel")
            .and_then(number_float)
            .unwrap_or(5.0)
            .max(0.0);
        let exhaustion: f32 = rest
            .get("foodExhaustionLevel")
            .and_then(number_float)
            .unwrap_or(0.0)
            .clamp(0.0, 40.0);
        let level = number_int(rest, "XpLevel").unwrap_or(0).max(0);
        let progress: f32 = rest
            .get("XpP")
            .and_then(number_float)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let total = number_int(rest, "XpTotal").unwrap_or(0).max(0);
        let air = number_int(rest, "Air").unwrap_or(300).max(0);
        let fire = number_int(rest, "Fire").unwrap_or(0).max(0);
        let frozen = number_int(rest, "TicksFrozen").unwrap_or(0).max(0);
        let hurt = number_int(rest, "HurtTime").unwrap_or(0).clamp(0, 255) as u8;
        let death = number_int(rest, "DeathTime").unwrap_or(0).clamp(0, 20) as u8;
        let mut effects = ActiveEffects::new();
        if let Some(list) = rest.get_list("active_effects") {
            for tag in list {
                let Some(compound) = tag.extract_compound() else {
                    continue;
                };
                let Some(name) = compound.get_string("id") else {
                    continue;
                };
                let Some(effect) = mob_effect_by_name(name) else {
                    continue;
                };
                let amplifier = number_int(compound, "amplifier")
                    .unwrap_or(0)
                    .clamp(0, i32::from(u8::MAX)) as u8;
                let duration = number_int(compound, "duration")
                    .unwrap_or(0)
                    .max(0)
                    .unsigned_abs();
                if duration == 0 {
                    continue;
                }
                effects.add(
                    ActiveEffect::new(effect, amplifier, duration)
                        .with_ambient(
                            compound
                                .get("ambient")
                                .and_then(NbtTag::extract_bool)
                                .unwrap_or(false),
                        )
                        .with_particles(
                            compound
                                .get("show_particles")
                                .and_then(NbtTag::extract_bool)
                                .unwrap_or(true),
                        )
                        .with_icon(
                            compound
                                .get("show_icon")
                                .and_then(NbtTag::extract_bool)
                                .unwrap_or(true),
                        ),
                );
            }
        }
        let max_health = 20.0
            + effects
                .amplifier(MobEffect::HealthBoost)
                .map_or(0.0, |level| 4.0 * f32::from(level.saturating_add(1)));
        let mut health = Health::new(max_health);
        health.current = health_value.min(max_health);
        health.absorption = rest
            .get("AbsorptionAmount")
            .and_then(number_float)
            .unwrap_or(0.0)
            .max(0.0);
        let mut food = FoodData::new(food_level, saturation);
        food.exhaustion = exhaustion;
        food.tick_timer = number_int(rest, "foodTickTimer").unwrap_or(0).max(0) as u32;
        let experience = PlayerExperience::new(level, progress, total);
        let mut vitals = CombatVitals::at(*transform);
        vitals.on_ground = true;
        let cooldown = DamageCooldown {
            hurt_time: hurt,
            ..DamageCooldown::default()
        };
        commands.entity(entity).insert((
            health,
            food,
            experience,
            AirSupply { air, max_air: 300 },
            FireState {
                remaining_fire_ticks: fire,
            },
            FreezeState {
                ticks_frozen: frozen,
            },
            effects,
            cooldown,
            CombatPosition(transform.translation),
            vitals,
        ));
        commands.entity(entity).insert((
            Selected(selected),
            PlayerInputState::default(),
            EntityEquipment::new(),
            Armor::default(),
            ArmorToughness::default(),
            KnockbackResistance(0.0),
            AttackDamage::default(),
            CombatSync::default(),
            EnvironmentSync::default(),
        ));
        if death > 0 {
            commands
                .entity(entity)
                .insert(DeathState { death_time: death });
        }
        if let Some(point) = respawn_point(rest) {
            commands.entity(entity).insert(point);
        }
    }
}

fn apply_attribute(
    modifier: &CombatModifier,
    armor: &mut f64,
    toughness: &mut f64,
    resistance: &mut f64,
) {
    let value = match modifier.attribute {
        value if value == Attribute::Armor as u16 => armor,
        value if value == Attribute::ArmorToughness as u16 => toughness,
        value if value == Attribute::KnockbackResistance as u16 => resistance,
        _ => return,
    };
    match modifier.operation {
        ModifierOperation::AddValue => *value += modifier.amount,
        ModifierOperation::AddMultipliedBase | ModifierOperation::AddMultipliedTotal => {
            *value *= 1.0 + modifier.amount;
        }
    }
}

fn apply_attack_modifier(modifier: &CombatModifier, value: &mut f64) {
    match modifier.operation {
        ModifierOperation::AddValue => *value += modifier.amount,
        ModifierOperation::AddMultipliedBase | ModifierOperation::AddMultipliedTotal => {
            *value *= 1.0 + modifier.amount;
        }
    }
}

fn sync_player_combat_state(
    mut players: Query<
        (
            &Transform,
            &Slots,
            &Selected,
            &mut CombatPosition,
            &mut Armor,
            &mut ArmorToughness,
            &mut KnockbackResistance,
            &mut AttackDamage,
            &mut EntityEquipment,
        ),
        With<Player>,
    >,
    stacks: Query<(&Kind, Option<&AttributeModifiers>)>,
) {
    for (
        transform,
        slots,
        selected,
        mut position,
        mut armor,
        mut toughness,
        mut resistance,
        mut attack,
        mut equipment,
    ) in &mut players
    {
        position.0 = transform.translation;
        let worn = [
            (EquipmentSlot::MainHand, usize::from(selected.0)),
            (EquipmentSlot::OffHand, 40),
            (EquipmentSlot::Feet, 36),
            (EquipmentSlot::Legs, 37),
            (EquipmentSlot::Chest, 38),
            (EquipmentSlot::Head, 39),
        ];
        let mut armor_value = 0.0;
        let mut toughness_value = 0.0;
        let mut resistance_value = 0.0;
        let mut attack_value = 1.0;
        for (slot, index) in worn {
            let stack = slots.get(index);
            equipment.set(
                slot,
                stack
                    .and_then(|stack| stacks.get(stack).ok())
                    .map(|(kind, _)| u32::from(kind.0)),
            );
            let Some(modifiers) = stack
                .and_then(|stack| stacks.get(stack).ok())
                .and_then(|(_, modifiers)| modifiers)
            else {
                continue;
            };
            for modifier in modifiers.modifiers.iter() {
                if !slot_group_applies(modifier.slot, slot) {
                    continue;
                }
                let combat_modifier = CombatModifier {
                    attribute: modifier.attribute.0,
                    amount: modifier.amount,
                    operation: modifier.operation,
                    slot: modifier.slot,
                };
                apply_attribute(
                    &combat_modifier,
                    &mut armor_value,
                    &mut toughness_value,
                    &mut resistance_value,
                );
                if slot == EquipmentSlot::MainHand
                    && combat_modifier.attribute == Attribute::AttackDamage as u16
                {
                    apply_attack_modifier(&combat_modifier, &mut attack_value);
                }
            }
        }
        armor.0 = (armor_value as f32).clamp(0.0, 30.0);
        toughness.0 = (toughness_value as f32).clamp(0.0, 20.0);
        resistance.0 = resistance_value.clamp(0.0, 1.0);
        attack.0 = attack_value.max(0.0) as f32;
    }
}

fn sync_player_environment_metadata(
    mut players: Query<
        (
            &Owner,
            &InDimension,
            &AirSupply,
            &FireState,
            &FreezeState,
            &mut EnvironmentSync,
        ),
        With<Player>,
    >,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, dimension, air, fire, freeze, mut sync) in &mut players {
        let on_fire = fire.remaining_fire_ticks > 0;
        if sync.air == Some(air.air)
            && sync.fire == Some(on_fire)
            && sync.frozen == Some(freeze.ticks_frozen)
        {
            continue;
        }
        sync.air = Some(air.air);
        sync.fire = Some(on_fire);
        sync.frozen = Some(freeze.ticks_frozen);
        let metadata = Metadata(vec![
            MetadataEntry {
                index: entity_data_index::SHARED_FLAGS,
                value: MetaDataValue::Byte(if on_fire { 1 } else { 0 }),
            },
            MetadataEntry {
                index: entity_data_index::AIR_SUPPLY,
                value: MetaDataValue::VarInt(VarInt(air.air)),
            },
            MetadataEntry {
                index: entity_data_index::TICKS_FROZEN,
                value: MetaDataValue::VarInt(VarInt(freeze.ticks_frozen)),
            },
        ]);
        packets.write(to_target(
            PacketTarget::AllInDim(dimension.0),
            PacketPayload::SetEntityData(ClientboundSetEntityData {
                entity_id: VarInt(player_network_id(owner.0)),
                metadata,
            }),
        ));
    }
}

fn save_player_combat(
    mut players: Query<
        (
            &Health,
            &FoodData,
            &PlayerExperience,
            &AirSupply,
            &FireState,
            &FreezeState,
            Option<&ActiveEffects>,
            Option<&DeathState>,
            Option<&DamageCooldown>,
            &mut LoadedPlayerDat,
        ),
        With<Player>,
    >,
) {
    for (health, food, experience, air, fire, freeze, effects, death, cooldown, mut loaded) in
        &mut players
    {
        let rest = &mut loaded.rest;
        rest.put_float("Health", health.current);
        rest.put_float("AbsorptionAmount", health.absorption);
        rest.put_int("foodLevel", food.food_level as i32);
        rest.put_float("foodSaturationLevel", food.saturation);
        rest.put_float("foodExhaustionLevel", food.exhaustion);
        rest.put_int("XpLevel", experience.level);
        rest.put_float("XpP", experience.progress);
        rest.put_int("XpTotal", experience.total_experience);
        rest.put_short(
            "Air",
            air.air.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
        );
        rest.put_short(
            "Fire",
            fire.remaining_fire_ticks
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
        );
        rest.put_int("TicksFrozen", freeze.ticks_frozen);
        rest.put_short(
            "DeathTime",
            i16::from(death.map_or(0, |death| death.death_time)),
        );
        rest.put_short(
            "HurtTime",
            i16::from(cooldown.map_or(0, |cooldown| cooldown.hurt_time)),
        );

        let saved_effects: Vec<NbtTag> = effects
            .into_iter()
            .flat_map(|effects| effects.iter())
            .filter(|effect| effect.duration > 0)
            .map(|effect| {
                let mut compound = NbtCompound::new();
                compound.put_string("id", effect.effect.as_str().to_owned());
                compound.put_byte("amplifier", effect.amplifier.min(i8::MAX as u8) as i8);
                compound.put_int("duration", effect.duration.min(i32::MAX as u32) as i32);
                compound.put_bool("ambient", effect.ambient);
                compound.put_bool("show_particles", effect.show_particles);
                compound.put_bool("show_icon", effect.show_icon);
                NbtTag::Compound(compound)
            })
            .collect();
        rest.put_list("active_effects", saved_effects);
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "a Bevy system: the player combat state and the packets of its clients are one pass"
)]
fn sync_player_combat(
    players: Query<
        (
            &Owner,
            &Health,
            &FoodData,
            &PlayerExperience,
            &Armor,
            &ArmorToughness,
            &AttackDamage,
            Option<&ActiveEffects>,
            &mut CombatSync,
        ),
        With<Player>,
    >,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, health, food, experience, armor, toughness, attack, effects, mut sync) in players {
        let health_changed = sync.health != Some(health.current);
        let food_changed = sync.food != Some((food.food_level, food.saturation));
        let attributes = (
            health.max.to_bits(),
            armor.0.to_bits(),
            toughness.0.to_bits(),
            attack.0.to_bits(),
        );
        if sync.attributes != Some(attributes) {
            sync.attributes = Some(attributes);
            packets.write(to(
                owner.0,
                PacketPayload::UpdateAttributes(ClientboundUpdateAttributes {
                    entity_id: VarInt(player_network_id(owner.0) as i32),
                    attributes: vec![
                        AttributeSnapshot {
                            attribute: Attribute::MaxHealth,
                            base: f64::from(health.max),
                            modifiers: Vec::new(),
                        },
                        AttributeSnapshot {
                            attribute: Attribute::Armor,
                            base: f64::from(armor.0),
                            modifiers: Vec::new(),
                        },
                        AttributeSnapshot {
                            attribute: Attribute::ArmorToughness,
                            base: f64::from(toughness.0),
                            modifiers: Vec::new(),
                        },
                        AttributeSnapshot {
                            attribute: Attribute::AttackDamage,
                            base: f64::from(attack.0),
                            modifiers: Vec::new(),
                        },
                    ],
                }),
            ));
        }
        if health_changed || food_changed {
            sync.health = Some(health.current);
            sync.food = Some((food.food_level, food.saturation));
            packets.write(to(
                owner.0,
                PacketPayload::SetHealth(ClientboundSetHealth {
                    health: health.current,
                    food: VarInt(food.food_level as i32),
                    saturation: food.saturation,
                }),
            ));
        }

        let xp = (
            experience.level,
            experience.progress.to_bits(),
            experience.total_experience,
        );
        if sync.experience != Some(xp) {
            sync.experience = Some(xp);
            packets.write(to(
                owner.0,
                PacketPayload::SetExperience(ClientboundSetExperience {
                    experience_progress: experience.progress,
                    total_experience: VarInt(experience.total_experience),
                    experience_level: VarInt(experience.level),
                }),
            ));
        }

        let active: Vec<(MobEffect, u8, u32, u8)> = effects
            .into_iter()
            .flat_map(|effects| effects.iter())
            .map(|effect| {
                let flags = u8::from(effect.ambient)
                    | (u8::from(effect.show_particles) << 1)
                    | (u8::from(effect.show_icon) << 2);
                (effect.effect, effect.amplifier, effect.duration, flags)
            })
            .collect();
        for &(effect, amplifier, duration, flags) in &active {
            let previous = sync
                .effects
                .iter()
                .find(|(active, _, _, _)| *active == effect);
            let stale = previous.is_none_or(|&(_, old_amplifier, old_duration, old_flags)| {
                amplifier != old_amplifier || flags != old_flags || duration > old_duration
            });
            if !stale {
                continue;
            }
            packets.write(to(
                owner.0,
                PacketPayload::UpdateMobEffect(ClientboundUpdateMobEffect {
                    entity_id: VarInt(owner.0.0 as i32 * 2 + 1),
                    effect: VarInt(effect.id() as i32),
                    amplifier: VarInt(i32::from(amplifier)),
                    duration: VarInt(duration.min(i32::MAX as u32) as i32),
                    flags,
                }),
            ));
        }
        for (effect, _, _, _) in &sync.effects {
            if !active.iter().any(|(active, _, _, _)| active == effect) {
                packets.write(to(
                    owner.0,
                    PacketPayload::RemoveMobEffect(ClientboundRemoveMobEffect {
                        entity_id: VarInt(owner.0.0 as i32 * 2 + 1),
                        effect: VarInt(effect.id() as i32),
                    }),
                ));
            }
        }
        sync.effects = active;
    }
}

fn mob_name(mob: &Mob) -> Text<mcrs_minecraft_text::ItemHover> {
    match &mob.custom_name {
        Some(name) => Text::text(name.clone()),
        None => Text::translate(
            format!(
                "entity.{}",
                mob.kind.entity_type.as_static_str().replace(':', ".")
            ),
            Vec::new(),
        ),
    }
}

fn projectile_name(kind: ProjectileKind) -> Text<mcrs_minecraft_text::ItemHover> {
    Text::translate(kind.display_key(), Vec::new())
}

fn announce_player_deaths(
    mut deaths: MessageReader<DeathEvent>,
    players: Query<(&Owner, &EntityUuid, Option<&GameProfile>), With<Player>>,
    mobs: Option<Res<Mobs>>,
    projectiles: Query<(&EntityUuid, &Projectile)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for death in deaths.read() {
        let Ok((owner, _, profile)) = players.get(death.entity) else {
            continue;
        };
        let player_id = (owner.0.0 as u32 * 2 + 1) as i32;
        packets.write(to(
            owner.0,
            PacketPayload::EntityEvent(ClientboundEntityEvent {
                entity_id: player_id,
                entity_status: ClientboundEntityEvent::DIED,
            }),
        ));
        let key = format!(
            "death.attack.{}",
            damage_types::message_id(death.damage_type)
        );
        let name = profile
            .map(|profile| Text::<mcrs_minecraft_text::ItemHover>::text(profile.username.clone()))
            .unwrap_or_else(|| Text::text("Player"));
        let cause = death
            .source
            .causing_entity
            .or(death.source.direct_entity)
            .and_then(|key| {
                players
                    .iter()
                    .find(|(_, uuid, _)| EntityKey(uuid.0.as_u128()) == key)
                    .and_then(|(_, _, profile)| profile.cloned())
                    .map(|profile| Text::text(profile.username))
                    .or_else(|| {
                        mobs.as_deref()
                            .and_then(|mobs| mobs.0.values().find(|mob| mob.uuid == key.0))
                            .map(mob_name)
                    })
                    .or_else(|| {
                        projectiles
                            .iter()
                            .find(|(uuid, _)| EntityKey(uuid.0.as_u128()) == key)
                            .map(|(_, projectile)| projectile_name(projectile.kind))
                    })
            });
        let mut args = vec![name];
        if let Some(cause) = cause {
            args.push(cause);
        }
        let message = Text::translate(key, args);
        if let Ok(message) = RawText::from_text(&message) {
            packets.write(to(
                owner.0,
                PacketPayload::PlayerCombatKill(ClientboundPlayerCombatKill {
                    player_id: VarInt(player_id),
                    message,
                }),
            ));
        }
    }
}

fn send_player_knockback(
    mut players: Query<(&Owner, &mut Velocity), (With<Player>, Changed<Velocity>)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, mut velocity) in &mut players {
        if velocity.0.length_squared() < 1.0e-4 {
            continue;
        }
        packets.write(to(
            owner.0,
            PacketPayload::SetEntityMotion(ClientboundSetEntityMotion {
                entity_id: VarInt(player_network_id(owner.0)),
                velocity: LpVec3(velocity.0),
            }),
        ));
        velocity.0 = DVec3::ZERO;
    }
}

fn hurt_yaw(target: DVec3, source: Option<DVec3>) -> f32 {
    source.map_or(0.0, |source| {
        let offset = source - target;
        f64::atan2(-offset.x, offset.z).to_degrees() as f32
    })
}

fn send_player_damage(
    mut dealt: MessageReader<DamageDealt>,
    players: Query<(&Owner, &EntityUuid, Option<&Transform>), With<Player>>,
    entities: Query<(&EntityUuid, Option<&GlobalEntityId>)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for event in dealt.read() {
        let Ok((owner, _, transform)) = players.get(event.target) else {
            continue;
        };
        let player_id = (owner.0.0 as u32 * 2 + 1) as i32;
        let network_id = |key: Option<EntityKey>| {
            key.and_then(|key| {
                players
                    .iter()
                    .find(|(_, uuid, _)| EntityKey(uuid.0.as_u128()) == key)
                    .map(|(owner, _, _)| player_network_id(owner.0))
                    .or_else(|| {
                        entities.iter().find_map(|(uuid, global)| {
                            (EntityKey(uuid.0.as_u128()) == key)
                                .then(|| global.and_then(|id| i32::try_from(id.0.0).ok()))
                                .flatten()
                        })
                    })
            })
            .unwrap_or(0)
        };
        if event.outcome != mcrs_minecraft_damage::HitOutcome::Full {
            continue;
        }
        packets.write(to(
            owner.0,
            PacketPayload::DamageEvent(ClientboundDamageEvent {
                entity_id: VarInt(player_id),
                damage_type: VarInt(event.hit.damage_type as i32),
                cause_entity_id: VarInt(network_id(event.hit.source.causing_entity)),
                direct_entity_id: VarInt(network_id(event.hit.source.direct_entity)),
                source_position: event.hit.source.source_position,
            }),
        ));
        if event.hit.knockback > 0.0
            && !event.hit.flags.no_knockback
            && event.hit.source.source_position.is_some()
        {
            packets.write(to(
                owner.0,
                PacketPayload::HurtAnimation(ClientboundHurtAnimation {
                    entity_id: VarInt(player_id),
                    yaw: hurt_yaw(
                        transform.map_or(DVec3::ZERO, |transform| transform.translation),
                        event.hit.source.source_position,
                    ),
                }),
            ));
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct RespawnPlayer {
    owner: &'static Owner,
    health: &'static mut Health,
    food: &'static mut FoodData,
    experience: &'static mut PlayerExperience,
    transform: &'static mut Transform,
    teleport: &'static mut TeleportState,
    vitals: &'static mut CombatVitals,
    sync: &'static mut CombatSync,
    in_dimension: &'static InDimension,
    air: &'static mut AirSupply,
    fire: &'static mut FireState,
    freeze: &'static mut FreezeState,
    environment_sync: &'static mut EnvironmentSync,
    effects: Option<&'static mut ActiveEffects>,
    respawn_point: Option<&'static PlayerRespawnPoint>,
    loaded: Option<&'static mut LoadedPlayerDat>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: the player, its dimension, the teleport state and the world clock are separate parameters"
)]
fn respawn_players(
    mut commands: Commands,
    mut requests: MessageReader<RespawnRequest>,
    mut players: Query<RespawnPlayer, (With<Player>, With<DeathState>)>,
    dimensions: Query<(
        &DimensionTypeId,
        &ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    )>,
    game_modes: Query<&PlayerGameMode>,
    rules: Option<Res<GameRules>>,
    save: Option<Res<WorldSave>>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ChangeDimensionRequest>,
) {
    for request in requests.read() {
        let Ok(RespawnPlayerItem {
            owner,
            mut health,
            mut food,
            mut experience,
            mut transform,
            mut teleport,
            mut vitals,
            mut sync,
            in_dimension,
            mut air,
            mut fire,
            mut freeze,
            mut environment_sync,
            effects,
            respawn_point,
            loaded,
        }) = players.get_mut(request.player)
        else {
            continue;
        };
        let Ok((dimension_type, dimension_key)) = dimensions.get(in_dimension.0) else {
            continue;
        };
        let level = save
            .as_deref()
            .and_then(|save| read_level_dat(&save.0).ok())
            .map(|level| level.spawn);
        let (target, spawn, rotation) = respawn_choice(
            respawn_point,
            level,
            (*dimension_key).clone(),
            vitals.respawn_at,
        );
        let known = *transform;
        let destination = Transform::from_translation(spawn).with_rotation(rotation);
        let keep_inventory = rules
            .as_deref()
            .is_some_and(|rules| rules.is_on(GameRule::KeepInventory));
        health.current = 20.0;
        health.max = 20.0;
        health.absorption = 0.0;
        *food = FoodData::default();
        if !keep_inventory {
            *experience = PlayerExperience::default();
        }
        if let Some(mut effects) = effects {
            effects.clear();
        }
        *air = AirSupply::default();
        fire.remaining_fire_ticks = 0;
        freeze.ticks_frozen = 0;
        *environment_sync = EnvironmentSync::default();
        sync.health = None;
        sync.food = None;
        sync.experience = None;
        sync.attributes = None;
        sync.effects.clear();
        vitals.last_y = spawn.y;
        vitals.fall_distance = 0.0;
        vitals.on_ground = true;
        commands.entity(request.player).remove::<DeathState>();

        if let Some(mut loaded) = loaded {
            write_combat_reset(&mut loaded.rest, keep_inventory);
        }

        if target != *dimension_key {
            changes.write(ChangeDimensionRequest {
                player: request.player,
                to: target,
                transform: destination,
            });
            continue;
        }

        let game_mode = game_modes
            .get(request.player)
            .map_or(GameMode::Survival, |mode| mode.0);
        packets.write(to(
            owner.0,
            PacketPayload::Respawn(ClientboundRespawn {
                player_spawn_info: PlayerSpawnInfo::new(dimension_type.0, target, game_mode),
                data_to_keep: 0,
            }),
        ));
        let teleport_id = teleport.sent(destination, *time);
        let telegram = teleport_to(known, destination);
        packets.write(to(
            owner.0,
            PacketPayload::PlayerPosition(position_packet(teleport_id, &telegram)),
        ));
        *transform = destination;
    }
}

fn respawn_choice(
    bed: Option<&PlayerRespawnPoint>,
    level: Option<mcrs_minecraft_save::RespawnData>,
    current: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    fallback: Transform,
) -> (
    ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    DVec3,
    Rotation,
) {
    if let Some(point) = bed {
        return (
            point.dimension.clone(),
            point.position,
            Rotation::new(point.yaw, point.pitch),
        );
    }
    if let Some(spawn) = level {
        return (
            spawn.dimension,
            DVec3::new(
                f64::from(spawn.pos[0]) + 0.5,
                f64::from(spawn.pos[1]),
                f64::from(spawn.pos[2]) + 0.5,
            ),
            Rotation::new(spawn.yaw, spawn.pitch),
        );
    }
    (current, fallback.translation, fallback.rotation)
}

fn write_combat_reset(rest: &mut NbtCompound, keep_inventory: bool) {
    rest.put_float("Health", 20.0);
    rest.put_float("AbsorptionAmount", 0.0);
    rest.put_int("foodLevel", 20);
    rest.put_float("foodSaturationLevel", 5.0);
    rest.put_float("foodExhaustionLevel", 0.0);
    rest.put_short("Air", 300);
    rest.put_short("Fire", 0);
    rest.put_int("TicksFrozen", 0);
    rest.put_short("DeathTime", 0);
    rest.put_short("HurtTime", 0);
    rest.put_list("active_effects", Vec::new());
    if !keep_inventory {
        rest.put_int("XpLevel", 0);
        rest.put_float("XpP", 0.0);
        rest.put_int("XpTotal", 0);
    }
}

fn number_int(rest: &NbtCompound, key: &str) -> Option<i32> {
    let tag = rest.get(key)?;
    tag.extract_int()
        .or_else(|| tag.extract_byte().map(i32::from))
        .or_else(|| tag.extract_short().map(i32::from))
        .or_else(|| {
            tag.extract_long()
                .and_then(|value| i32::try_from(value).ok())
        })
        .or_else(|| tag.extract_float().map(|value| value as i32))
}

fn number_float(tag: &NbtTag) -> Option<f32> {
    tag.extract_float()
        .or_else(|| tag.extract_double().map(|value| value as f32))
        .or_else(|| tag.extract_int().map(|value| value as f32))
        .or_else(|| tag.extract_long().map(|value| value as f32))
}

fn respawn_point(rest: &NbtCompound) -> Option<PlayerRespawnPoint> {
    let respawn = rest.get("respawn")?.extract_compound()?;
    let dimension = respawn.get_string("dimension")?;
    let dimension = mcrs_minecraft_resource::ResourceLocationBuf::read(dimension).ok()?;
    let position = respawn
        .get_int_array("pos")
        .and_then(|pos| <[i32; 3]>::try_from(pos).ok())?;
    let yaw = respawn.get("yaw").and_then(number_float).unwrap_or(0.0);
    let pitch = respawn.get("pitch").and_then(number_float).unwrap_or(0.0);
    Some(PlayerRespawnPoint {
        dimension: ResourceKeyBuf::from_location(dimension),
        position: DVec3::new(
            f64::from(position[0]) + 0.5,
            f64::from(position[1]),
            f64::from(position[2]) + 0.5,
        ),
        yaw,
        pitch,
    })
}

/// The combat state of a player: saved health, hunger and experience, the HUD packets that
/// carry them to the client, and the death and respawn life cycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct CombatRuntimePlugin;

impl Plugin for CombatRuntimePlugin {
    fn build(&self, app: &mut App) {
        let rules = app
            .world()
            .get_resource::<CurrentRegistries>()
            .map_or_else(DamageTypeRules::builtin, |registries| {
                DamageTypeRules::from_registries(registries)
            });
        app.insert_resource(DamageTypeRulesResource(rules));
        app.init_resource::<WorldBorderState>();
        app.add_message::<ExperienceDropped>();
        app.add_observer(note_ground_pos);
        app.add_observer(note_ground_pos_rot);
        app.add_observer(note_ground_rot);
        app.add_observer(note_ground_status);
        app.add_observer(on_client_command)
            .add_observer(on_player_attack)
            .add_observer(on_change_difficulty)
            .add_observer(on_player_input)
            .add_observer(on_player_action)
            .add_observer(on_set_carried_item)
            .add_observer(on_player_use_item_on);
        app.add_systems(
            FixedUpdate,
            (
                attach_player_combat,
                sync_player_combat_state,
                tick_player_hunger,
                tick_player_environment,
                tick_mob_environment,
                sync_player_environment_metadata,
                spawn_block_experience,
            )
                .chain()
                .in_set(CombatSet::Pre),
        );
        app.add_systems(
            FixedUpdate,
            (
                clear_loaded_inventory_on_death,
                respawn_players,
                sync_player_combat,
                save_player_inventory,
                save_player_combat,
            )
                .chain()
                .in_set(CombatSet::Post),
        );
        app.add_systems(
            FixedUpdate,
            (
                send_equipment_snapshot,
                send_equipment_changes,
                send_player_damage,
                send_player_knockback,
                announce_player_deaths,
            )
                .chain()
                .in_set(CombatSet::Send),
        );
    }
}

#[derive(Clone, Copy)]
struct AttackReach {
    eye: DVec3,
    reach: f64,
}

impl AttackReach {
    fn of(transform: &Transform, mode: GameMode) -> Self {
        let reach = if mode == GameMode::Creative { 5.0 } else { 3.0 };
        Self {
            eye: transform.translation + DVec3::Y * mcrs_minecraft_entity::PLAYER_EYE_HEIGHT,
            reach,
        }
    }

    fn reaches(self, position: DVec3, width: f64, height: f64) -> bool {
        let half = width / 2.0;
        let min = position - DVec3::new(half, 0.0, half);
        let max = position + DVec3::new(half, height, half);
        let closest = self.eye.clamp(min, max);
        self.eye.distance_squared(closest) <= self.reach * self.reach
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: the attacker, the target rows, the rules and the damage events are separate parameters"
)]
fn on_player_attack(
    event: On<Received<ServerboundAttack>>,
    mut players: Query<
        (
            &Transform,
            Option<&EntityUuid>,
            Option<&AttackDamage>,
            Option<&ActiveEffects>,
            Option<&PlayerGameMode>,
            Option<&mut FoodData>,
            &InDimension,
        ),
        With<Player>,
    >,
    targets: Query<(Entity, &Transform, &Owner, Option<&Health>, &InDimension), With<Player>>,
    mut rows: ResMut<EntityRows>,
    mut changes: ResMut<MobChanges>,
    mut damage: MessageWriter<DamageRequest>,
    rules: Option<Res<DamageTypeRulesResource>>,
    clock: Res<BirthClock>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((transform, uuid, attack, effects, mode, food, dimension)) =
        players.get_mut(event.entity)
    else {
        return;
    };
    let mode = mode.map_or(GameMode::Survival, |mode| mode.0);
    if mode == GameMode::Spectator {
        return;
    }
    let reach = AttackReach::of(transform, mode);
    let mut source = DamageSource::new(damage_types::PLAYER_ATTACK);
    if let Some(uuid) = uuid {
        let key = EntityKey(uuid.0.as_u128());
        source = source.with_causing(key).with_direct(key);
    }
    source.source_position = Some(reach.eye);
    let amount = (attack.map_or(1.0, |attack| attack.0)
        + effects.map_or(0.0, ActiveEffects::attack_damage_bonus))
    .max(0.0);
    if let Some(mut food) = food
        && mode != GameMode::Creative
    {
        food.add_exhaustion(0.1);
    }
    let hit_flags = rules.as_deref().map_or(
        DamageTypeFlags::builtin_for(damage_types::PLAYER_ATTACK),
        |rules| rules.0.flags(damage_types::PLAYER_ATTACK),
    );

    let mob = rows
        .living_rows()
        .find(|(at, _)| at.id.0 as i64 == i64::from(packet.entity_id.0))
        .and_then(|(at, state)| {
            let row = rows.tile(at.tile)?.rows().find(|row| row.id == at.id)?;
            let scale = state.living.map_or(1.0, |living| living.attributes.scale);
            Some((
                at,
                state.body.position,
                f64::from(row.motion.dimensions.width) * scale,
                f64::from(row.motion.dimensions.height) * scale,
            ))
        });
    if let Some((at, position, width, height)) = mob {
        if !reach.reaches(position, width, height) {
            return;
        }
        let hit = Hit::from_source(amount, source).with_flags(hit_flags);
        hurt_mob(&mut rows, &mut changes, at, hit, clock.0);
        return;
    }

    for (target, target_transform, owner, health, target_dimension) in &targets {
        if target_dimension.0 != dimension.0
            || player_network_id(owner.0) != packet.entity_id.0
            || health.is_some_and(Health::is_dead)
            || !reach.reaches(target_transform.translation, 0.6, 1.8)
        {
            continue;
        }
        let hit = Hit::from_source(amount, source).with_flags(hit_flags);
        damage.write(DamageRequest::new(target, hit));
        return;
    }
}

fn clear_loaded_inventory_on_death(
    mut events: MessageReader<crate::death_drops::DropInventoryOnDeathEvent>,
    mut players: Query<&mut LoadedPlayerDat>,
) {
    for event in events.read() {
        let Ok(mut loaded) = players.get_mut(event.entity) else {
            continue;
        };
        loaded.inventory.clear();
        loaded.equipment.clear();
    }
}

fn save_player_inventory(
    mut players: Query<(&Slots, &mut LoadedPlayerDat, Option<&DeathState>), With<Player>>,
    stacks: Query<(&Kind, &Count)>,
    items: Option<Res<Items>>,
) {
    let Some(items) = items else {
        return;
    };
    for (slots, mut loaded, death) in &mut players {
        if death.is_some() {
            continue;
        }
        loaded.inventory.clear();
        for (index, stack) in slots.iter().enumerate() {
            if index > usize::from(slots::OFFHAND) {
                continue;
            }
            let Some((kind, count)) = stack.and_then(|stack| stacks.get(stack).ok()) else {
                continue;
            };
            let Some(id) = mcrs_minecraft_resource::Id::<Item>::checked(kind.0, items.len()) else {
                continue;
            };
            let Some(entry) = items.get(id) else {
                continue;
            };
            let Ok(slot) = u8::try_from(index) else {
                continue;
            };
            loaded
                .inventory
                .push(mcrs_minecraft_protocol_game::item::ItemStackWithSlot {
                    slot,
                    stack: ItemStackValue {
                        item: ResourceKeyBuf::from_location(entry.identifier.clone()),
                        count: i32::from(count.get()).into(),
                        components: Default::default(),
                    },
                });
        }
    }
}

fn send_equipment_snapshot(
    players: Query<(&Owner, &InDimension, &EntityEquipment), With<Player>>,
    viewers: Query<(&Owner, &InDimension), With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, dimension, equipment) in &players {
        let slots: Vec<_> = equipment
            .iter()
            .map(|(slot, item)| {
                let stack = item.map_or_else(ProtoStack::default, |item| ProtoStack {
                    id: WireId::<Item>::new(u16::try_from(item).unwrap_or(u16::MAX)),
                    count: 1,
                    components: Default::default(),
                });
                (slot, stack)
            })
            .collect();
        for (&Owner(session), seen) in &viewers {
            if seen.0 != dimension.0 {
                continue;
            }
            packets.write(to(
                session,
                PacketPayload::SetEquipment(ClientboundSetEquipment {
                    entity_id: VarInt(player_network_id(owner.0)),
                    slots: slots.clone(),
                }),
            ));
        }
    }
}

fn send_equipment_changes(
    mut events: MessageReader<EquipmentChangedEvent>,
    targets: Query<(&InDimension, Option<&Owner>)>,
    viewers: Query<(&Owner, &InDimension)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for event in events.read() {
        let Ok((target_dimension, Some(owner))) = targets.get(event.entity) else {
            continue;
        };
        let stack = event
            .item_id
            .map_or_else(ProtoStack::default, |item| ProtoStack {
                id: WireId::<Item>::new(u16::try_from(item).unwrap_or(u16::MAX)),
                count: 1,
                components: Default::default(),
            });
        for (&Owner(session), &InDimension(dimension)) in &viewers {
            if dimension != target_dimension.0 {
                continue;
            }
            packets.write(to(
                session,
                PacketPayload::SetEquipment(ClientboundSetEquipment {
                    entity_id: VarInt(player_network_id(owner.0)),
                    slots: vec![(event.slot, stack.clone())],
                }),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};

    #[derive(Resource, Default)]
    struct SeenPackets(Vec<PacketPayload>);

    fn record_packets(
        mut packets: MessageReader<OutboundPlayerPacket>,
        mut seen: ResMut<SeenPackets>,
    ) {
        seen.0
            .extend(packets.read().map(|packet| packet.data.clone()));
    }

    fn dimension_key(name: &str) -> ResourceKeyBuf<mcrs_minecraft_dimension::Dimension> {
        ResourceKeyBuf::from_location(
            mcrs_minecraft_resource::ResourceLocationBuf::read(name).expect("a valid dimension id"),
        )
    }

    #[test]
    fn dmg_net_001_and_dmg_net_006_only_full_knockback_hits_reach_the_client() {
        let mut app = App::new();
        app.add_message::<DamageDealt>();
        app.add_message::<OutboundPlayerPacket>();
        app.init_resource::<SeenPackets>();
        app.add_systems(Update, (send_player_damage, record_packets).chain());
        let target = app
            .world_mut()
            .spawn((
                Player,
                Owner(PlayerSession(1)),
                EntityUuid(mcrs_minecraft_protocol::uuid::Uuid::from_u128(7)),
            ))
            .id();
        let mut source = DamageSource::new(crate::damage_types::PLAYER_ATTACK);
        source.source_position = Some(DVec3::new(1.0, 64.0, 0.0));

        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DamageDealt>>();
        messages.write(DamageDealt {
            target,
            hit: Hit::from_source(1.0, source),
            outcome: mcrs_minecraft_damage::HitOutcome::Partial,
            damage: 1.0,
        });
        app.update();
        assert!(app.world().resource::<SeenPackets>().0.is_empty());

        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DamageDealt>>();
        messages.write(DamageDealt {
            target,
            hit: Hit::from_source(2.0, source),
            outcome: mcrs_minecraft_damage::HitOutcome::Full,
            damage: 2.0,
        });
        app.update();
        let packets = &app.world().resource::<SeenPackets>().0;
        assert_eq!(packets.len(), 2);
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, PacketPayload::DamageEvent(_)))
        );
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, PacketPayload::HurtAnimation(_)))
        );

        let mut fall = DamageSource::new(crate::damage_types::FALL);
        fall.source_position = Some(DVec3::new(1.0, 64.0, 0.0));
        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DamageDealt>>();
        messages.write(DamageDealt {
            target,
            hit: Hit::from_source(3.0, fall),
            outcome: mcrs_minecraft_damage::HitOutcome::Full,
            damage: 3.0,
        });
        app.update();
        let packets = &app.world().resource::<SeenPackets>().0;
        assert_eq!(packets.len(), 3);
        assert_eq!(
            packets
                .iter()
                .filter(|packet| matches!(packet, PacketPayload::HurtAnimation(_)))
                .count(),
            1
        );
    }

    #[test]
    fn an_attack_uses_the_game_mode_entity_range() {
        let transform = Transform::from_xyz(0.0, 64.0, 0.0);
        let survival = AttackReach::of(&transform, GameMode::Survival);
        let creative = AttackReach::of(&transform, GameMode::Creative);
        assert!((survival.eye.y - 65.62).abs() < 1e-6);
        assert!(survival.reaches(DVec3::new(0.0, 64.0, 2.5), 0.6, 1.8));
        assert!(!survival.reaches(DVec3::new(0.0, 64.0, 4.0), 0.6, 1.8));
        assert!(creative.reaches(DVec3::new(0.0, 64.0, 4.5), 0.6, 1.8));
    }

    #[test]
    fn a_loaded_player_keeps_its_health_and_hunger() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        app.add_plugins(CombatRuntimePlugin);
        app.add_message::<OutboundPlayerPacket>();
        app.add_message::<ChangeDimensionRequest>();
        app.insert_resource(GameTime(0));

        let mut loaded = LoadedPlayerDat::default();
        loaded.rest.put_float("Health", 7.0);
        loaded.rest.put_int("foodLevel", 10);
        loaded.rest.put_float("foodSaturationLevel", 0.0);
        loaded.rest.put_int("XpLevel", 4);
        let entity = app
            .world_mut()
            .spawn((
                Player,
                Owner(PlayerSession(1)),
                Transform::from_xyz(0.0, 64.0, 0.0),
                loaded,
            ))
            .id();

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(FixedUpdate);

        let health = app.world().get::<Health>(entity).expect("health");
        assert_eq!(health.current, 7.0);
        let food = app.world().get::<FoodData>(entity).expect("food");
        assert_eq!(food.food_level, 10);
        let experience = app
            .world()
            .get::<PlayerExperience>(entity)
            .expect("experience");
        assert_eq!(experience.level, 4);

        app.world_mut().get_mut::<Health>(entity).unwrap().current = 5.0;
        app.world_mut().run_schedule(FixedUpdate);
        let loaded = app
            .world()
            .get::<LoadedPlayerDat>(entity)
            .expect("saved state");
        assert_eq!(
            loaded.rest.get("Health").and_then(NbtTag::extract_float),
            Some(5.0)
        );
        assert_eq!(number_int(&loaded.rest, "foodLevel"), Some(10));
    }

    #[test]
    fn dmg_save_001_hurt_death_air_and_fire_survive_a_reload() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        app.add_plugins(CombatRuntimePlugin);
        app.add_message::<OutboundPlayerPacket>();
        app.add_message::<ChangeDimensionRequest>();
        app.insert_resource(GameTime(0));

        let mut loaded = LoadedPlayerDat::default();
        loaded.rest.put_short("HurtTime", 5);
        loaded.rest.put_short("DeathTime", 3);
        loaded.rest.put_short("Air", 120);
        loaded.rest.put_short("Fire", 40);
        let entity = app
            .world_mut()
            .spawn((
                Player,
                Owner(PlayerSession(1)),
                Transform::from_xyz(0.0, 64.0, 0.0),
                loaded,
            ))
            .id();

        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(
            app.world().get::<DamageCooldown>(entity).unwrap().hurt_time,
            4
        );
        assert_eq!(app.world().get::<DeathState>(entity).unwrap().death_time, 4);
        assert_eq!(app.world().get::<AirSupply>(entity).unwrap().air, 120);
        assert!(
            app.world()
                .get::<FireState>(entity)
                .unwrap()
                .remaining_fire_ticks
                > 0
        );

        let loaded = app.world().get::<LoadedPlayerDat>(entity).unwrap();
        assert_eq!(
            loaded.rest.get("HurtTime").and_then(NbtTag::extract_short),
            Some(4)
        );
        assert_eq!(
            loaded.rest.get("Air").and_then(NbtTag::extract_short),
            Some(120)
        );
        assert!(
            loaded
                .rest
                .get("Fire")
                .and_then(NbtTag::extract_short)
                .unwrap_or(0)
                > 0
        );
        assert!(
            loaded
                .rest
                .get("DeathTime")
                .and_then(NbtTag::extract_short)
                .unwrap_or(0)
                >= 4
        );
    }

    #[test]
    fn a_loaded_health_boost_effect_restores_the_maximum_before_the_health() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        app.add_plugins(CombatRuntimePlugin);
        app.add_message::<OutboundPlayerPacket>();
        app.add_message::<ChangeDimensionRequest>();
        app.insert_resource(GameTime(0));

        let mut effect = mcrs_minecraft_nbt::compound::NbtCompound::new();
        effect.put_string("id", "minecraft:health_boost".to_owned());
        effect.put_int("duration", 200);
        effect.put_byte("amplifier", 1);
        let mut loaded = LoadedPlayerDat::default();
        loaded.rest.put_float("Health", 24.0);
        loaded.rest.put_list("active_effects", vec![effect.into()]);
        let entity = app
            .world_mut()
            .spawn((
                Player,
                Owner(PlayerSession(1)),
                Transform::from_xyz(0.0, 64.0, 0.0),
                loaded,
            ))
            .id();

        app.world_mut().run_schedule(FixedUpdate);

        let health = app.world().get::<Health>(entity).expect("health");
        assert_eq!(health.max, 28.0);
        assert_eq!(health.current, 24.0);
    }

    #[test]
    fn a_bed_in_another_dimension_wins_over_the_level_spawn() {
        let bed = PlayerRespawnPoint {
            dimension: dimension_key("minecraft:the_nether"),
            position: DVec3::new(4.5, 70.0, -2.5),
            yaw: 45.0,
            pitch: 5.0,
        };
        let level = mcrs_minecraft_save::RespawnData {
            dimension: dimension_key("minecraft:overworld"),
            pos: [0, 64, 0],
            yaw: 0.0,
            pitch: 0.0,
        };
        let (target, position, rotation) = respawn_choice(
            Some(&bed),
            Some(level),
            dimension_key("minecraft:overworld"),
            Transform::IDENTITY,
        );
        assert_eq!(target.as_str(), "minecraft:the_nether");
        assert_eq!(position, DVec3::new(4.5, 70.0, -2.5));
        assert_eq!(rotation.yaw(), 45.0);
        assert_eq!(rotation.pitch(), 5.0);
    }

    #[test]
    fn the_level_spawn_crosses_a_dimension_without_a_bed() {
        let level = mcrs_minecraft_save::RespawnData {
            dimension: dimension_key("minecraft:overworld"),
            pos: [8, 80, -8],
            yaw: 180.0,
            pitch: 0.0,
        };
        let (target, position, _) = respawn_choice(
            None,
            Some(level),
            dimension_key("minecraft:the_nether"),
            Transform::IDENTITY,
        );
        assert_eq!(target.as_str(), "minecraft:overworld");
        assert_eq!(position, DVec3::new(8.5, 80.0, -7.5));
    }

    #[test]
    fn dmg_net_006_a_mob_cause_uses_its_type_name() {
        use mcrs_minecraft_entity::EntityTypeData;
        use mcrs_minecraft_protocol_game::entity::MobLooks;

        let mob = Mob {
            kind: &EntityTypeData::PIG,
            uuid: 7,
            looks: MobLooks::default(),
            custom_name: None,
            tags: Vec::new(),
        };
        assert_eq!(
            mob_name(&mob),
            Text::translate("entity.minecraft.pig", Vec::new())
        );

        let named = Mob {
            custom_name: Some("Bacon".to_owned()),
            ..mob
        };
        assert_eq!(mob_name(&named), Text::text("Bacon"));
    }

    #[test]
    fn dmg_net_003_a_hurt_animation_points_from_the_source() {
        assert!(
            (hurt_yaw(DVec3::ZERO, Some(DVec3::new(0.0, 0.0, -1.0))).abs() - 180.0).abs() < 1.0e-4
        );
        assert!((hurt_yaw(DVec3::ZERO, Some(DVec3::new(1.0, 0.0, 0.0))) + 90.0).abs() < 1.0e-4);
    }

    #[test]
    fn a_legacy_short_fire_tag_is_read() {
        let mut rest = NbtCompound::new();
        rest.put_short("Fire", 120);
        assert_eq!(number_int(&rest, "Fire"), Some(120));
    }

    #[test]
    fn a_saved_respawn_point_is_read_from_player_data() {
        let mut rest = NbtCompound::new();
        let mut respawn = NbtCompound::new();
        respawn.put_string("dimension", "minecraft:overworld".to_owned());
        respawn.put("pos", NbtTag::IntArray(vec![12, 64, -4]));
        respawn.put_float("yaw", 90.0);
        respawn.put_float("pitch", 10.0);
        rest.put_component("respawn", respawn);
        let point = respawn_point(&rest).expect("respawn point");
        assert_eq!(point.dimension.as_str(), "minecraft:overworld");
        assert_eq!(point.position, DVec3::new(12.5, 64.0, -3.5));
        assert_eq!(point.yaw, 90.0);
        assert_eq!(point.pitch, 10.0);
    }
}
