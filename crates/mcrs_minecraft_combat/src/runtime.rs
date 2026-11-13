use std::collections::HashSet;

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryData;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::BlockStateFlags;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_damage::{
    DamageSource, DamageTypeRules, Hit, border_damage, fall_damage as compute_fall_damage, ignite,
    is_in_void, tick_air, tick_fire, tick_freeze,
};
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget, to, to_target,
};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::teleport::{TeleportState, position_packet, teleport_to};
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_item::{Selected, Slots};
use mcrs_minecraft_engine_physics::{Mob, Mobs, PhysicsTables};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
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
use mcrs_minecraft_engine_world::world_settings::{Difficulty, WorldBorderSettings};
use mcrs_minecraft_entity::attribute_instance::{AttributeOperation, fold_attribute};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{AttributeModifiers, Kind, ModifierOperation, SlotGroup};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_physics::{BlockAction, BlockFlags, Vitals};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAttack, ServerboundChangeDifficulty, ServerboundPlayerAction,
    ServerboundPlayerInput, ServerboundSetCarriedItem,
};
use mcrs_minecraft_registries::{CurrentRegistries, WireId};

use crate::CombatSet;

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

use crate::components::Dying as DeathState;
use crate::components::{
    Armor, ArmorToughness, AttackDamage, KnockbackResistance, PlayerExperience, PlayerHitRecord,
};
use crate::components::{
    PlayerEffects as ActiveEffects, PlayerFood as FoodData, PlayerVitals as Health,
};
use crate::damage_rules::DamageTypeRulesResource;
use crate::equipment::EntityEquipment;
use crate::events::{DamageDealt, DamageRequest, DamageTarget, DeathEvent, RespawnRequest};
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct EnvironmentTimers {
    last_position: DVec3,
    fall_distance: f64,
    on_ground: bool,
    jumping: bool,
    respawn_at: Transform,
}

#[derive(Clone, Copy, Debug, Default)]
struct Touched {
    fire: bool,
    cactus: bool,
    berry: bool,
    hot_floor: bool,
    fall: f64,
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

impl EnvironmentTimers {
    fn at(transform: Transform) -> Self {
        Self {
            last_position: transform.translation,
            fall_distance: 0.0,
            on_ground: false,
            jumping: false,
            respawn_at: transform,
        }
    }
}

#[derive(Resource, Default)]
struct WorldSpawn {
    value: Option<mcrs_minecraft_save::RespawnData>,
    loaded: bool,
}

#[derive(Component, Debug, Default)]
struct EnvironmentSync {
    air: Option<i32>,
    fire: Option<bool>,
    frozen: Option<i32>,
    flags: Option<i8>,
}

#[derive(Component, Debug, Default)]
struct CombatSync {
    health: Option<f32>,
    food: Option<(u32, f32)>,
    experience: Option<(i32, u32, i32)>,
    effects: Vec<(MobEffect, u8, u32, u8)>,
    attributes: Option<(u32, u32, u32, u32)>,
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

fn note_ground_pos(
    on: On<Received<ServerboundMovePlayerPos>>,
    mut players: Query<&mut EnvironmentTimers>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_pos_rot(
    on: On<Received<ServerboundMovePlayerPosRot>>,
    mut players: Query<&mut EnvironmentTimers>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_rot(
    on: On<Received<ServerboundMovePlayerRot>>,
    mut players: Query<&mut EnvironmentTimers>,
) {
    if let Some(packet) = on.packet()
        && let Ok(mut vitals) = players.get_mut(on.entity)
    {
        vitals.on_ground = packet.flags.on_ground();
    }
}

fn note_ground_status(
    on: On<Received<ServerboundMovePlayerStatusOnly>>,
    mut players: Query<&mut EnvironmentTimers>,
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
    op_levels: Query<&PlayerOpLevel>,
    mut difficulty: ResMut<Difficulty>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    if !op_levels.get(on.entity).is_ok_and(|level| level.0 >= 2) {
        return;
    }
    difficulty.0 = packet.difficulty.into();
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
    mut players: Query<&mut Selected>,
) {
    let Some(packet) = on.packet() else {
        return;
    };
    let Ok(mut selected) = players.get_mut(on.entity) else {
        return;
    };
    let Ok(slot) = u8::try_from(packet.slot) else {
        return;
    };
    if slot > 8 {
        return;
    }
    selected.0 = slot;
}

fn tick_player_hunger(
    mut players: Query<(Entity, &mut Health, &mut FoodData), (With<Player>, Without<DeathState>)>,
    rules: Option<Res<GameRules>>,
    difficulty: Option<Res<Difficulty>>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
    time: Option<Res<GameTime>>,
    mut damage: MessageWriter<DamageRequest>,
) {
    let natural_regeneration = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::NaturalHealthRegeneration));
    let age = time.as_deref().map_or(0, |time| time.0.max(0) as u64);
    for (entity, mut health, mut food) in &mut players {
        let food_tick = food.0.tick(
            &health.0,
            natural_regeneration,
            difficulty
                .as_deref()
                .map_or(Difficulty::default().0, |value| value.0),
            age,
        );
        if food_tick.heal > 0.0 {
            health.heal(food_tick.heal);
        }
        if food_tick.starve
            && health.is_alive()
            && let Some(hit) = hit_of(type_rules.as_deref(), "starve", 1.0)
        {
            damage.write(DamageRequest::player(entity, hit));
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
            &mut EnvironmentTimers,
        ),
        (With<Player>, Without<DeathState>),
    >,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    tables: Option<Res<PhysicsTables>>,
    dimensions: Query<&DimensionTypeConfig>,
    rules: Option<Res<GameRules>>,
    border: Option<Res<WorldBorderSettings>>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
    time: Option<Res<GameTime>>,
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
    let age = time.as_deref().map_or(0, |time| time.0.max(0) as u64);
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
        mut health,
        mut food,
        mut timers,
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

        let mut eye_in_water = false;
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
            eye_in_water = is_water(view, definitions, eye);
            in_water = is_water(view, definitions, feet);
            in_lava = is_lava(view, definitions, feet) || is_lava(view, definitions, eye);
            if let Some(tables) = tables.as_deref() {
                let feet_flags = block_flags(view, tables, feet);
                let eye_flags = block_flags(view, tables, eye);
                in_fire = feet_flags.contains(BlockFlags::FIRE);
                in_powder_snow = feet_flags.contains(BlockFlags::POWDER_SNOW)
                    || eye_flags.contains(BlockFlags::POWDER_SNOW);
                in_berry_bush = feet_flags.contains(BlockFlags::SWEET_BERRY_BUSH);
                on_magma = block_flags(view, tables, below).contains(BlockFlags::HOT_FLOOR);
                in_cactus = touches_cactus(view, tables, position);
            }
            in_wall = view.block(eye).ok().is_some_and(|state| {
                definitions
                    .state(state)
                    .flags
                    .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
                    && definitions.state(state).fluid.is_none()
            });
        }

        let travelled = DVec3::new(
            position.x - timers.last_position.x,
            0.0,
            position.z - timers.last_position.z,
        )
        .length();
        if travelled > 0.0 && travelled < 10.0 {
            if sprinting {
                food.add_exhaustion(0.1 * travelled as f32);
            } else if in_water {
                food.add_exhaustion(0.01 * travelled as f32);
            }
        }
        let jumping = input.is_some_and(|input| input.jumping);
        if jumping && !timers.jumping && timers.on_ground {
            food.add_exhaustion(if sprinting { 0.2 } else { 0.05 });
        }
        timers.jumping = jumping;

        if vulnerable && fall_damage && !flying.is_some_and(|flying| flying.0) {
            let dy = position.y - timers.last_position.y;
            if dy.abs() > 8.0 {
                timers.fall_distance = 0.0;
            } else if dy < 0.0 && !in_water && !in_lava && !in_powder_snow {
                timers.fall_distance += -dy;
            }
            if timers.on_ground {
                let multiplier = fall_multiplier(view.as_ref(), tables.as_deref(), below);
                let fall = compute_fall_damage(timers.fall_distance, 3.0, multiplier);
                if fall > 0.0
                    && let Some(hit) = hit_of(type_rules.as_deref(), "fall", fall)
                {
                    damage.write(DamageRequest::player(entity, hit));
                }
                timers.fall_distance = 0.0;
            }
        } else {
            timers.fall_distance = 0.0;
        }
        timers.last_position = position;

        let fire_resistant = effects.is_some_and(|effects| effects.has(MobEffect::FireResistance));
        if in_water {
            health.fire_ticks = 0;
        }
        if let Ok(config) = dimensions.get(in_dimension.0)
            && is_in_void(position.y, f64::from(config.min_y))
            && let Some(hit) = hit_of(type_rules.as_deref(), "out_of_world", 4.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if !vulnerable {
            continue;
        }
        let water_breathing = effects.is_some_and(|effects| effects.has(MobEffect::WaterBreathing));
        if tick_air(
            &mut health.0,
            300,
            eye_in_water && !water_breathing,
            !eye_in_water,
        ) && drowning_damage
            && let Some(hit) = hit_of(type_rules.as_deref(), "drown", 2.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if in_lava {
            ignite(&mut health.0, 300, fire_resistant);
            if !fire_resistant && let Some(hit) = hit_of(type_rules.as_deref(), "lava", 4.0) {
                damage.write(DamageRequest::player(entity, hit));
            }
        }
        if in_fire {
            ignite(&mut health.0, 160, fire_resistant);
        }
        if fire_damage
            && tick_fire(&mut health.0, fire_resistant, in_lava)
            && let Some(hit) = hit_of(type_rules.as_deref(), "on_fire", 1.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if freeze_damage
            && tick_freeze(&mut health.0, in_powder_snow, true, false, age)
            && let Some(hit) = hit_of(type_rules.as_deref(), "freeze", 1.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if in_cactus && let Some(hit) = hit_of(type_rules.as_deref(), "cactus", 1.0) {
            damage.write(DamageRequest::player(entity, hit));
        }
        if in_wall && let Some(hit) = hit_of(type_rules.as_deref(), "in_wall", 1.0) {
            damage.write(DamageRequest::player(entity, hit));
        }
        if in_berry_bush
            && velocity.is_some_and(|velocity| {
                velocity.0.x * velocity.0.x + velocity.0.z * velocity.0.z > 0.003
            })
            && let Some(hit) = hit_of(type_rules.as_deref(), "sweet_berry_bush", 1.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if on_magma
            && !input.is_some_and(|input| input.sneaking)
            && let Some(hit) = hit_of(type_rules.as_deref(), "hot_floor", 1.0)
        {
            damage.write(DamageRequest::player(entity, hit));
        }
        if let Some(border) = border.as_deref() {
            let amount = border_damage(&border.0, position, 0.6, 1.8);
            if amount > 0.0
                && let Some(hit) = hit_of(type_rules.as_deref(), "outside_border", amount)
            {
                damage.write(DamageRequest::player(entity, hit));
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
    clock: Option<Res<BirthClock>>,
    world: WorldView,
    dimensions: Query<Entity, With<Dimension>>,
    dimension_types: Query<&DimensionTypeConfig>,
    definitions: Option<Res<Blocks>>,
    tables: Option<Res<PhysicsTables>>,
    rules: Option<Res<GameRules>>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
    mut damage: MessageWriter<DamageRequest>,
) {
    let Some(mut rows) = rows else {
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
    let tick = clock.as_deref().map_or(0, |clock| clock.0);
    let places: Vec<Placed> = rows
        .living_rows()
        .filter(|(_, state)| state.living.is_some_and(|living| living.is_alive()))
        .map(|(at, _)| at)
        .collect();
    let actions: Vec<_> = rows.actions().to_vec();
    let mut touched: std::collections::BTreeMap<u64, Touched> = std::collections::BTreeMap::new();
    for action in actions {
        let block = match action.action {
            BlockAction::Step(at) => block_state_bits(Some(&view), tables.as_deref(), at),
            BlockAction::Inside(at) => block_state_bits(Some(&view), tables.as_deref(), at),
            BlockAction::Fall { at, .. } => block_state_bits(Some(&view), tables.as_deref(), at),
            BlockAction::Bounce(_) => continue,
        };
        let entry = touched.entry(action.at.id.0).or_default();
        match action.action {
            BlockAction::Step(_) => {
                entry.hot_floor = block.is_some_and(|block| {
                    block
                        .flags
                        .contains(mcrs_minecraft_physics::BlockFlags::HOT_FLOOR)
                });
            }
            BlockAction::Inside(_) => {
                if let Some(block) = block {
                    entry.fire |= block
                        .flags
                        .contains(mcrs_minecraft_physics::BlockFlags::FIRE);
                    entry.cactus |= block
                        .flags
                        .contains(mcrs_minecraft_physics::BlockFlags::CACTUS);
                    entry.berry |= block
                        .flags
                        .contains(mcrs_minecraft_physics::BlockFlags::SWEET_BERRY_BUSH);
                }
            }
            BlockAction::Fall { distance, .. } => {
                entry.fall += distance
                    * f64::from(block.map_or(1.0, |block| block.motion.fall_damage_multiplier));
            }
            BlockAction::Bounce(_) => {}
        }
    }
    let mut hits: Vec<(Placed, Hit)> = Vec::new();
    for at in places {
        let mut touched = touched.remove(&at.id.0).unwrap_or_default();
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
        let feet_in_water = is_water(&view, definitions, feet);
        let eye_in_water = is_water(&view, definitions, eye);
        if let Some(tables) = tables.as_deref() {
            let flags = block_flags(&view, tables, feet);
            touched.fire |= flags.contains(BlockFlags::FIRE);
            touched.cactus |= flags.contains(BlockFlags::CACTUS);
            touched.berry |= flags.contains(BlockFlags::SWEET_BERRY_BUSH);
            touched.hot_floor |=
                block_flags(&view, tables, BlockPos::new(feet.x, feet.y - 1, feet.z))
                    .contains(BlockFlags::HOT_FLOOR);
        }
        let in_lava = is_lava(&view, definitions, feet) || is_lava(&view, definitions, eye);
        let in_powder_snow = is_block(&view, definitions, feet, "minecraft:powder_snow")
            || is_block(&view, definitions, eye, "minecraft:powder_snow");
        let in_wall = view.block(eye).ok().is_some_and(|state| {
            definitions
                .state(state)
                .flags
                .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
                && definitions.state(state).fluid.is_none()
        });

        if feet_in_water {
            living.vitals.fire_ticks = 0;
        }
        if in_lava {
            ignite(&mut living.vitals, 300, living.fire_immune);
        } else if touched.fire {
            ignite(&mut living.vitals, 160, living.fire_immune);
        }
        if fire_damage
            && tick_fire(&mut living.vitals, living.fire_immune, in_lava)
            && let Some(hit) = hit_of(type_rules.as_deref(), "on_fire", 1.0)
        {
            hits.push((at, hit));
        }
        if in_lava
            && !living.fire_immune
            && let Some(hit) = hit_of(type_rules.as_deref(), "lava", 4.0)
        {
            hits.push((at, hit));
        }
        if tick_air(&mut living.vitals, 300, eye_in_water, !eye_in_water)
            && drowning_damage
            && let Some(hit) = hit_of(type_rules.as_deref(), "drown", 2.0)
        {
            hits.push((at, hit));
        }
        if freeze_damage
            && tick_freeze(
                &mut living.vitals,
                in_powder_snow,
                true,
                false,
                tick.max(0) as u64,
            )
            && let Some(hit) = hit_of(type_rules.as_deref(), "freeze", 1.0)
        {
            hits.push((at, hit));
        }
        if touched.cactus
            && let Some(hit) = hit_of(type_rules.as_deref(), "cactus", 1.0)
        {
            hits.push((at, hit));
        }
        if touched.hot_floor
            && let Some(hit) = hit_of(type_rules.as_deref(), "hot_floor", 1.0)
        {
            hits.push((at, hit));
        }
        if touched.fall > 3.0
            && let Some(hit) = hit_of(
                type_rules.as_deref(),
                "fall",
                compute_fall_damage(touched.fall, 3.0, 1.0),
            )
        {
            hits.push((at, hit));
        }
        if in_wall && let Some(hit) = hit_of(type_rules.as_deref(), "in_wall", 1.0) {
            hits.push((at, hit));
        }
        if touched.berry
            && state.body.velocity.x * state.body.velocity.x
                + state.body.velocity.z * state.body.velocity.z
                > 0.003
            && let Some(hit) = hit_of(type_rules.as_deref(), "sweet_berry_bush", 1.0)
        {
            hits.push((at, hit));
        }
        if is_in_void(position.y, min_y)
            && let Some(hit) = hit_of(type_rules.as_deref(), "out_of_world", 4.0)
        {
            hits.push((at, hit));
        }
    }
    for (at, hit) in hits {
        damage.write(DamageRequest::row(at.id, hit));
    }
}

fn hit_of(rules: Option<&DamageTypeRulesResource>, path: &str, amount: f32) -> Option<Hit> {
    let damage_type = rules?.0.id_path(path)?;
    Some(Hit::new(amount, damage_type))
}

fn fall_multiplier(
    view: Option<&DimensionView<'_>>,
    tables: Option<&PhysicsTables>,
    below: BlockPos,
) -> f32 {
    block_state_bits(view, tables, below).map_or(1.0, |block| block.motion.fall_damage_multiplier)
}

fn block_state_bits(
    view: Option<&DimensionView<'_>>,
    tables: Option<&PhysicsTables>,
    at: BlockPos,
) -> Option<mcrs_minecraft_physics::StateCollision> {
    let state = block_state(view?, at)?;
    Some(tables?.collision().state(state))
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

fn touches_cactus(view: &DimensionView<'_>, tables: &PhysicsTables, position: DVec3) -> bool {
    let min_y = position.y.floor() as i32;
    for y in min_y..=min_y + 1 {
        for x in [position.x - 0.3, position.x + 0.3] {
            for z in [position.z - 0.3, position.z + 0.3] {
                let at = BlockPos::new(x.floor() as i32, y, z.floor() as i32);
                if block_flags(view, tables, at).contains(BlockFlags::CACTUS) {
                    return true;
                }
            }
        }
    }
    false
}

fn block_flags(view: &DimensionView<'_>, tables: &PhysicsTables, at: BlockPos) -> BlockFlags {
    block_state(view, at).map_or(BlockFlags::empty(), |state| {
        tables.collision().state(state).flags
    })
}

fn attach_player_combat(
    mut commands: Commands,
    players: Query<(Entity, &Transform), (With<Player>, Without<Health>)>,
) {
    for (entity, transform) in &players {
        let mut vitals = EnvironmentTimers::at(*transform);
        vitals.on_ground = true;
        commands.entity(entity).insert((
            Health::new(20.0, 20.0),
            FoodData::default(),
            PlayerExperience::default(),
            PlayerHitRecord::default(),
            ActiveEffects::new(),
            vitals,
        ));
        commands.entity(entity).insert_if_new(Selected(0));
        commands.entity(entity).insert((
            PlayerInputState::default(),
            EntityEquipment::new(),
            Armor::default(),
            ArmorToughness::default(),
            KnockbackResistance(0.0),
            AttackDamage(1.0),
            CombatSync::default(),
            EnvironmentSync::default(),
        ));
    }
}

fn sync_player_combat_state(
    mut players: Query<
        (
            &Slots,
            &Selected,
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
    for (slots, selected, mut armor, mut toughness, mut resistance, mut attack, mut equipment) in
        &mut players
    {
        let worn = [
            (EquipmentSlot::MainHand, usize::from(selected.0)),
            (EquipmentSlot::OffHand, 40),
            (EquipmentSlot::Feet, 36),
            (EquipmentSlot::Legs, 37),
            (EquipmentSlot::Chest, 38),
            (EquipmentSlot::Head, 39),
        ];
        let mut armor_modifiers = Vec::new();
        let mut toughness_modifiers = Vec::new();
        let mut resistance_modifiers = Vec::new();
        let mut attack_modifiers = Vec::new();
        for (slot, index) in worn {
            let stack = slots.get(index);
            let next = stack
                .and_then(|stack| stacks.get(stack).ok())
                .map(|(kind, _)| u32::from(kind.0));
            if equipment.get(slot) != next {
                equipment.set(slot, next);
            }
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
                let operation = match modifier.operation {
                    ModifierOperation::AddValue => AttributeOperation::AddValue,
                    ModifierOperation::AddMultipliedBase => AttributeOperation::AddMultipliedBase,
                    ModifierOperation::AddMultipliedTotal => AttributeOperation::AddMultipliedTotal,
                };
                let folded = (operation, modifier.amount);
                match modifier.attribute.0 {
                    value if value == Attribute::Armor as u16 => armor_modifiers.push(folded),
                    value if value == Attribute::ArmorToughness as u16 => {
                        toughness_modifiers.push(folded);
                    }
                    value if value == Attribute::KnockbackResistance as u16 => {
                        resistance_modifiers.push(folded);
                    }
                    value
                        if value == Attribute::AttackDamage as u16
                            && slot == EquipmentSlot::MainHand =>
                    {
                        attack_modifiers.push(folded);
                    }
                    _ => {}
                }
            }
        }
        let next_armor = (fold_attribute(0.0, armor_modifiers) as f32).clamp(0.0, 30.0);
        let next_toughness = (fold_attribute(0.0, toughness_modifiers) as f32).clamp(0.0, 20.0);
        let next_resistance = fold_attribute(0.0, resistance_modifiers).clamp(0.0, 1.0);
        let next_attack = (fold_attribute(1.0, attack_modifiers) as f32).max(0.0);
        if armor.armor != next_armor {
            armor.armor = next_armor;
        }
        if toughness.0 != next_toughness {
            toughness.0 = next_toughness;
        }
        if resistance.0 != next_resistance {
            resistance.0 = next_resistance;
        }
        if attack.0 != next_attack {
            attack.0 = next_attack;
        }
    }
}

fn sync_player_environment_metadata(
    mut players: Query<
        (
            &Owner,
            &InDimension,
            &Health,
            &mut EnvironmentSync,
            Option<&PlayerInputState>,
            Option<&ActiveEffects>,
            Option<&PlayerFlying>,
        ),
        With<Player>,
    >,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, dimension, health, mut sync, input, effects, flying) in &mut players {
        let on_fire = health.fire_ticks > 0;
        let mut flags = 0;
        if on_fire {
            flags |= entity_data_index::SHARED_FLAG_ON_FIRE;
        }
        if input.is_some_and(|input| input.sneaking) {
            flags |= entity_data_index::SHARED_FLAG_CROUCHING;
        }
        if input.is_some_and(|input| input.sprinting) {
            flags |= entity_data_index::SHARED_FLAG_SPRINTING;
        }
        if effects.is_some_and(|effects| effects.has(MobEffect::Invisibility)) {
            flags |= entity_data_index::SHARED_FLAG_INVISIBLE;
        }
        if flying.is_some_and(|flying| flying.0) {
            flags |= entity_data_index::SHARED_FLAG_FLYING;
        }
        if sync.air == Some(health.air)
            && sync.fire == Some(on_fire)
            && sync.frozen == Some(health.frozen_ticks)
            && sync.flags == Some(flags)
        {
            continue;
        }
        sync.air = Some(health.air);
        sync.fire = Some(on_fire);
        sync.frozen = Some(health.frozen_ticks);
        sync.flags = Some(flags);
        let metadata = Metadata(vec![
            MetadataEntry {
                index: entity_data_index::SHARED_FLAGS,
                value: MetaDataValue::Byte(flags),
            },
            MetadataEntry {
                index: entity_data_index::AIR_SUPPLY,
                value: MetaDataValue::VarInt(VarInt(health.air)),
            },
            MetadataEntry {
                index: entity_data_index::TICKS_FROZEN,
                value: MetaDataValue::VarInt(VarInt(health.frozen_ticks)),
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
        let health_changed = sync.health != Some(health.health);
        let food_changed = sync.food != Some((food.food_level, food.saturation));
        let attributes = (
            health.max_health.to_bits(),
            armor.armor.to_bits(),
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
                            base: f64::from(health.max_health),
                            modifiers: Vec::new(),
                        },
                        AttributeSnapshot {
                            attribute: Attribute::Armor,
                            base: f64::from(armor.armor),
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
            sync.health = Some(health.health);
            sync.food = Some((food.food_level, food.saturation));
            packets.write(to(
                owner.0,
                PacketPayload::SetHealth(ClientboundSetHealth {
                    health: health.health,
                    food: VarInt(food.food_level as i32),
                    saturation: food.saturation,
                }),
            ));
        }

        let xp = (
            experience.level,
            experience.progress.to_bits(),
            experience.total,
        );
        if sync.experience != Some(xp) {
            sync.experience = Some(xp);
            packets.write(to(
                owner.0,
                PacketPayload::SetExperience(ClientboundSetExperience {
                    experience_progress: experience.progress,
                    total_experience: VarInt(experience.total),
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
                    effect: VarInt(effect.id().number() as i32),
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
                        effect: VarInt(effect.id().number() as i32),
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

fn announce_player_deaths(
    mut deaths: MessageReader<DeathEvent>,
    players: Query<(&Owner, &InDimension, &EntityUuid, Option<&GameProfile>), With<Player>>,
    mobs: Option<Res<Mobs>>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
    viewers: Query<(&Owner, &InDimension)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for death in deaths.read() {
        let DamageTarget::Player(target) = death.target else {
            continue;
        };
        let Ok((owner, dimension, _, profile)) = players.get(target) else {
            continue;
        };
        let player_id = (owner.0.0 as u32 * 2 + 1) as i32;
        packets.write(to_target(
            players_in_dimension(&viewers, dimension.0),
            PacketPayload::EntityEvent(ClientboundEntityEvent {
                entity_id: player_id,
                entity_status: ClientboundEntityEvent::DIED,
            }),
        ));
        let key = format!(
            "death.attack.{}",
            type_rules
                .as_deref()
                .and_then(|rules| rules.0.message_id(death.source.damage_type))
                .unwrap_or("generic")
        );
        let name = profile
            .map(|profile| Text::<mcrs_minecraft_text::ItemHover>::text(profile.username.clone()))
            .unwrap_or_else(|| Text::text("Player"));
        let cause = death
            .source
            .cause
            .causing
            .or(death.source.cause.direct)
            .and_then(|key| {
                players
                    .iter()
                    .find(|(_, _, uuid, _)| EntityKey(uuid.0.as_u128()) == key)
                    .and_then(|(_, _, _, profile)| profile.cloned())
                    .map(|profile| Text::text(profile.username))
                    .or_else(|| {
                        mobs.as_deref()
                            .and_then(|mobs| mobs.0.values().find(|mob| mob.uuid == key.0))
                            .map(mob_name)
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
    players: Query<(&Owner, &Velocity), (With<Player>, Changed<Velocity>)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, velocity) in &players {
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
    }
}

fn players_in_dimension(
    viewers: &Query<(&Owner, &InDimension)>,
    dimension: Entity,
) -> PacketTarget {
    PacketTarget::PlayerSet(
        viewers
            .iter()
            .filter(|(_, in_dimension)| in_dimension.0 == dimension)
            .map(|(owner, _)| owner.0)
            .collect(),
    )
}

fn hurt_yaw(target: DVec3, source: Option<DVec3>) -> f32 {
    source.map_or(0.0, |source| {
        let offset = source - target;
        f64::atan2(-offset.x, offset.z).to_degrees() as f32
    })
}

fn send_player_damage(
    mut dealt: MessageReader<DamageDealt>,
    players: Query<(&Owner, &InDimension, &EntityUuid, Option<&Transform>), With<Player>>,
    entities: Query<(&EntityUuid, Option<&GlobalEntityId>)>,
    type_rules: Option<Res<DamageTypeRulesResource>>,
    viewers: Query<(&Owner, &InDimension)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for event in dealt.read() {
        let DamageTarget::Player(target) = event.target else {
            continue;
        };
        let Ok((owner, dimension, _, transform)) = players.get(target) else {
            continue;
        };
        let player_id = (owner.0.0 as u32 * 2 + 1) as i32;
        let network_id = |key: Option<EntityKey>| {
            key.and_then(|key| {
                players
                    .iter()
                    .find(|(_, _, uuid, _)| EntityKey(uuid.0.as_u128()) == key)
                    .map(|(owner, _, _, _)| player_network_id(owner.0))
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
        packets.write(to_target(
            players_in_dimension(&viewers, dimension.0),
            PacketPayload::DamageEvent(ClientboundDamageEvent {
                entity_id: VarInt(player_id),
                damage_type: VarInt(event.hit.damage_type as i32),
                cause_entity_id: VarInt(network_id(event.hit.source.cause.causing)),
                direct_entity_id: VarInt(network_id(event.hit.source.cause.direct)),
                source_position: event.hit.source.source_position,
            }),
        ));
        let no_knockback = type_rules
            .as_deref()
            .is_some_and(|rules| rules.0.flags(event.hit.damage_type).no_knockback);
        if event.hit.knockback > 0.0 && !no_knockback && event.hit.source.source_position.is_some()
        {
            packets.write(to_target(
                players_in_dimension(&viewers, dimension.0),
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

fn load_world_spawn(save: Option<Res<WorldSave>>, mut world_spawn: ResMut<WorldSpawn>) {
    if world_spawn.loaded {
        return;
    }
    let Some(save) = save else {
        return;
    };
    world_spawn.value = read_level_dat(&save.0).ok().map(|level| level.spawn);
    world_spawn.loaded = true;
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
    vitals: &'static mut EnvironmentTimers,
    sync: &'static mut CombatSync,
    in_dimension: &'static InDimension,
    environment_sync: &'static mut EnvironmentSync,
    effects: Option<&'static mut ActiveEffects>,
    respawn_point: Option<&'static PlayerRespawnPoint>,
    velocity: Option<&'static mut Velocity>,
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
    world_spawn: Res<WorldSpawn>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ChangeDimensionRequest>,
) {
    let mut seen = HashSet::new();
    for request in requests.read() {
        if !seen.insert(request.player) {
            continue;
        }
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
            mut environment_sync,
            effects,
            respawn_point,
            velocity,
        }) = players.get_mut(request.player)
        else {
            continue;
        };
        let Ok((dimension_type, dimension_key)) = dimensions.get(in_dimension.0) else {
            continue;
        };
        let known = *transform;
        let (target, destination) = respawn_destination(
            respawn_point,
            world_spawn.value.clone(),
            (*dimension_key).clone(),
            vitals.respawn_at,
        );
        let spawn = destination.translation;
        let keep_inventory = rules
            .as_deref()
            .is_some_and(|rules| rules.is_on(GameRule::KeepInventory));
        health.0 = Vitals {
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            last_damage: 0.0,
            damage_cooldown: 0,
            hurt_time: 0,
            death_time: None,
            fire_ticks: 0,
            air: 300,
            frozen_ticks: 0,
        };
        *food = FoodData::default();
        if !keep_inventory {
            *experience = PlayerExperience::default();
        }
        if let Some(mut effects) = effects {
            effects.clear();
        }
        if let Some(mut velocity) = velocity {
            velocity.0 = DVec3::ZERO;
        }
        *environment_sync = EnvironmentSync::default();
        sync.health = None;
        sync.food = None;
        sync.experience = None;
        sync.attributes = None;
        sync.effects.clear();
        vitals.last_position = spawn;
        vitals.jumping = false;
        vitals.fall_distance = 0.0;
        vitals.on_ground = true;
        commands.entity(request.player).remove::<DeathState>();

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

fn respawn_destination(
    bed: Option<&PlayerRespawnPoint>,
    level: Option<mcrs_minecraft_save::RespawnData>,
    current: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    fallback: Transform,
) -> (
    ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    Transform,
) {
    let (target, spawn, rotation) = respawn_choice(bed, level, current, fallback);
    (
        target,
        Transform::from_translation(spawn).with_rotation(rotation),
    )
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

/// The combat state of a player: saved health, hunger and experience, the HUD packets that
/// carry them to the client, and the death and respawn life cycle.
pub(crate) fn build(app: &mut App) {
    let rules = app
        .world()
        .get_resource::<CurrentRegistries>()
        .map_or_else(DamageTypeRules::default, |registries| {
            DamageTypeRules::from_registries(registries)
        });
    app.insert_resource(DamageTypeRulesResource(rules));
    app.init_resource::<WorldSpawn>();
    app.init_resource::<WorldBorderSettings>();
    app.init_resource::<Difficulty>();
    app.add_observer(note_ground_pos);
    app.add_observer(note_ground_pos_rot);
    app.add_observer(note_ground_rot);
    app.add_observer(note_ground_status);
    app.add_observer(on_client_command)
        .add_observer(on_player_attack)
        .add_observer(on_change_difficulty)
        .add_observer(on_player_input)
        .add_observer(on_player_action)
        .add_observer(on_set_carried_item);
    app.add_systems(
        FixedUpdate,
        (
            attach_player_combat,
            sync_player_combat_state,
            tick_player_hunger,
            tick_player_environment,
            tick_mob_environment,
            sync_player_environment_metadata,
        )
            .chain()
            .in_set(CombatSet::Pre),
    );
    app.add_systems(
        FixedUpdate,
        (load_world_spawn, respawn_players, sync_player_combat)
            .chain()
            .in_set(CombatSet::Post),
    );
    app.add_systems(
        FixedUpdate,
        (
            send_equipment_snapshots,
            send_equipment_to_new_viewers,
            send_player_damage,
            send_player_knockback,
            announce_player_deaths,
        )
            .chain()
            .in_set(CombatSet::Send),
    );
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
    rows: Res<EntityRows>,
    mut damage: MessageWriter<DamageRequest>,
    rules: Option<Res<DamageTypeRulesResource>>,
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
    let damage_type = rules
        .as_deref()
        .and_then(|rules| rules.0.id_path("player_attack"))
        .unwrap_or(0);
    let mut source = DamageSource::new(damage_type);
    if let Some(uuid) = uuid {
        let key = EntityKey(uuid.0.as_u128());
        source = source.with_cause(mcrs_minecraft_entity_key::Cause::new(key, key));
    }
    source.source_position = Some(reach.eye);
    let amount = (attack.map_or(1.0, |attack| attack.0)
        + effects.map_or(0.0, |effects| effects.attack_damage_bonus()))
    .max(0.0);
    if let Some(mut food) = food
        && mode != GameMode::Creative
    {
        food.add_exhaustion(0.1);
    }
    let mob = u64::try_from(packet.entity_id.0)
        .ok()
        .and_then(|id| rows.placed(mcrs_minecraft_entity_key::EntityId(id)))
        .and_then(|at| {
            let state = rows.state_of(at)?;
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
        damage.write(DamageRequest::row(at.id, Hit::from_source(amount, source)));
        return;
    }

    for (target, target_transform, owner, health, target_dimension) in &targets {
        if target_dimension.0 != dimension.0
            || player_network_id(owner.0) != packet.entity_id.0
            || health.is_some_and(|health| !health.is_alive())
            || !reach.reaches(target_transform.translation, 0.6, 1.8)
        {
            continue;
        }
        let hit = Hit::from_source(amount, source);
        damage.write(DamageRequest::player(target, hit));
        return;
    }
}

fn equipment_packet(owner: &Owner, equipment: &EntityEquipment) -> PacketPayload {
    let slots = equipment
        .iter()
        .map(|(slot, item)| {
            (
                slot,
                item.map_or_else(ProtoStack::default, |item| ProtoStack {
                    id: WireId::<Item>::new(u16::try_from(item).unwrap_or(u16::MAX)),
                    count: 1,
                    components: Default::default(),
                }),
            )
        })
        .collect();
    PacketPayload::SetEquipment(ClientboundSetEquipment {
        entity_id: VarInt(player_network_id(owner.0)),
        slots,
    })
}

fn send_equipment_snapshots(
    targets: Query<
        (&Owner, &InDimension, &EntityEquipment),
        (
            With<Player>,
            Or<(Added<EntityEquipment>, Changed<EntityEquipment>)>,
        ),
    >,
    viewers: Query<(&Owner, &InDimension)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, dimension, equipment) in &targets {
        let packet = equipment_packet(owner, equipment);
        for (&Owner(session), &InDimension(viewer_dimension)) in &viewers {
            if viewer_dimension == dimension.0 {
                packets.write(to(session, packet.clone()));
            }
        }
    }
}

fn send_equipment_to_new_viewers(
    new_viewers: Query<(&Owner, &InDimension), Added<Owner>>,
    targets: Query<(&Owner, &InDimension, &EntityEquipment), With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (&Owner(session), &InDimension(viewer_dimension)) in &new_viewers {
        for (owner, dimension, equipment) in &targets {
            if dimension.0 == viewer_dimension {
                packets.write(to(session, equipment_packet(owner, equipment)));
            }
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
    fn shared_flags_keep_crouch_and_sprint_when_fire_is_off() {
        let mut app = App::new();
        app.add_message::<OutboundPlayerPacket>();
        app.init_resource::<SeenPackets>();
        app.add_systems(
            Update,
            (sync_player_environment_metadata, record_packets).chain(),
        );
        app.world_mut().spawn((
            Player,
            Owner(PlayerSession(1)),
            InDimension(Entity::PLACEHOLDER),
            Health::new(20.0, 20.0),
            EnvironmentSync::default(),
            PlayerInputState {
                sneaking: true,
                sprinting: true,
                jumping: false,
            },
        ));
        app.update();
        let packets = &app.world().resource::<SeenPackets>().0;
        let flags = packets.iter().find_map(|packet| match packet {
            PacketPayload::SetEntityData(packet) => packet
                .metadata
                .0
                .iter()
                .find(|entry| entry.index == entity_data_index::SHARED_FLAGS)
                .and_then(|entry| match entry.value {
                    MetaDataValue::Byte(flags) => Some(flags),
                    _ => None,
                }),
            _ => None,
        });
        assert_eq!(
            flags,
            Some(
                entity_data_index::SHARED_FLAG_CROUCHING | entity_data_index::SHARED_FLAG_SPRINTING
            )
        );
    }

    #[test]
    fn equipment_snapshots_are_not_resent_every_tick() {
        let mut app = App::new();
        app.add_message::<OutboundPlayerPacket>();
        app.init_resource::<SeenPackets>();
        app.add_systems(Update, (send_equipment_snapshots, record_packets).chain());
        let mut equipment = EntityEquipment::new();
        equipment.set(
            mcrs_minecraft_item::component::equippable::EquipmentSlot::MainHand,
            Some(1),
        );
        app.world_mut().spawn((
            Player,
            Owner(PlayerSession(1)),
            InDimension(Entity::PLACEHOLDER),
            equipment,
        ));
        app.world_mut().spawn((
            Player,
            Owner(PlayerSession(2)),
            InDimension(Entity::PLACEHOLDER),
        ));
        app.update();
        assert_eq!(app.world().resource::<SeenPackets>().0.len(), 2);
        app.world_mut().resource_mut::<SeenPackets>().0.clear();
        app.update();
        assert!(app.world().resource::<SeenPackets>().0.is_empty());
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
                InDimension(Entity::PLACEHOLDER),
                EntityUuid(mcrs_minecraft_protocol::uuid::Uuid::from_u128(7)),
            ))
            .id();
        let mut source = DamageSource::new(0);
        source.source_position = Some(DVec3::new(1.0, 64.0, 0.0));

        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DamageDealt>>();
        messages.write(DamageDealt {
            target: DamageTarget::Player(target),
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
            target: DamageTarget::Player(target),
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

        let mut fall = DamageSource::new(1);
        fall.source_position = Some(DVec3::new(1.0, 64.0, 0.0));
        let mut fall_hit = Hit::from_source(3.0, fall);
        fall_hit.knockback = 0.0;
        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DamageDealt>>();
        messages.write(DamageDealt {
            target: DamageTarget::Player(target),
            hit: fall_hit,
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
            ..Default::default()
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
            ..Default::default()
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
}
