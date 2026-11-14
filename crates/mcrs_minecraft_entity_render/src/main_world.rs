use bevy::prelude::*;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_animation::{EntityAnimation, EntityQuery};
use mcrs_minecraft_entity_visual::{
    Age, ClientEntity, Equipment, EquippableAsset, HURT_TICKS, Living, Motion, RenderInputs,
    RenderInputsQuery, ServerTicking, SyncedData,
};
use mcrs_minecraft_item::component::equippable::{EquipmentSlot, Equippable};
use mcrs_minecraft_item::definition::Items;
use mcrs_minecraft_render_controller::{RenderQuery, TextureChoice};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_volume::LightVolume;
use mcrs_molang::{ScalarVm, Value, string_hash};
use mcrs_voxel::BlockPos;

use crate::cpu_inputs::input_record;
use crate::entity_assets::{EntityAssets, texture_identifier};
use crate::frame_queries::{FrameQueries, VariantEntry, pose_name};
use crate::player_skin::{PlayerModel, SkinSlot, skin_entry};
use crate::records::{
    CULL_FULL_RIG_ONLY, CULL_VALID, CullRecord, MAX_LAYERS, STATE_BABY, STATE_DYING,
    STATE_HIDDEN_SHIFT, TickRecord, f16_up,
};
use crate::slot_upload::SlotArena;

/// The slot of a client entity that the entity passes draw, and its definition.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntitySlot {
    pub slot: u32,
    pub definition: u16,
}

/// The animation state of an entity whose definition reads values that the CPU computes.
#[derive(Component, Clone, Debug)]
pub struct CpuAnimation(pub EntityAnimation);

/// Clips that a recorded frame started, with the ticks since their start: each plays with the
/// weight 1 and that time, in place of what the play list gives. A frame that a client draws from
/// the inputs of a record uses it (`docs/spec/client-frame-capture.md`, CFC-CMP-002).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct RecordedClips(pub Vec<RecordedClip>);

#[derive(Clone, Debug, PartialEq)]
pub struct RecordedClip {
    /// The short name of the `animate` entry.
    pub entry: String,
    pub elapsed_ticks: f32,
}

/// The client tick and the partial tick of the entities in this frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct EntityClock {
    /// The world ticks that the client ran.
    pub tick: u32,
    pub partial_tick: f32,
}

#[derive(Resource, Default)]
pub(crate) struct EntityVm(ScalarVm);

/// The step that the cull radius grows by for the movement between two ticks, in blocks.
const MOVEMENT_MARGIN: f32 = 0.25;
/// The hurt start of an entity that was never hurt: far before any age.
const NEVER_HURT: f32 = -1.0e9;

/// Whether the server runs ticks: a frozen world runs only the steps that it asks for.
pub(crate) fn server_ticks(ticking: Option<Res<ServerTicking>>) -> bool {
    ticking.is_none_or(|ticking| ticking.runs())
}

pub(crate) fn advance_clock(mut clock: ResMut<EntityClock>) {
    clock.tick = clock.tick.wrapping_add(1);
}

pub(crate) fn frame_clock(
    time: Res<Time<Fixed>>,
    ticking: Option<Res<ServerTicking>>,
    mut clock: ResMut<EntityClock>,
) {
    let overstep = time.overstep_fraction();
    let partial = ticking.map_or(overstep, |ticking| ticking.partial_tick(overstep));
    clock.partial_tick = partial.clamp(0.0, 1.0);
}

/// Gives each client entity of a type with a client entity definition a slot.
pub(crate) fn assign_slots(
    mut commands: Commands,
    assets: Option<Res<EntityAssets>>,
    mut arena: ResMut<SlotArena>,
    entities: Query<(Entity, &ClientEntity), Without<EntitySlot>>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (entity, client) in &entities {
        let Some(&definition) = assets.kinds.get(&client.kind) else {
            continue;
        };
        let slot = arena.allocate();
        let mut spawned = commands.entity(entity);
        spawned.insert(EntitySlot {
            slot,
            definition: definition as u16,
        });
        let uses_input = assets
            .library
            .definitions
            .get(definition)
            .and_then(|tables| tables.program.as_ref().ok())
            .is_some_and(|program| program.inputs.is_used());
        if let (true, Some(entry)) = (uses_input, assets.pack.entities.get(definition)) {
            spawned.insert(CpuAnimation(EntityAnimation::new(&entry.rig)));
        }
    }
}

pub(crate) fn release_slot(
    removed: On<Remove<EntitySlot>>,
    slots: Query<&EntitySlot>,
    mut arena: ResMut<SlotArena>,
) {
    if let Ok(slot) = slots.get(removed.entity) {
        arena.release(slot.slot);
    }
}

/// The yaws in degrees and the walk of an entity in a tick record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Facing {
    body_yaw: f32,
    head_yaw: f32,
    walk_position: f32,
    walk_speed: f32,
}

impl Facing {
    /// A dying entity keeps its yaws and stops its walk.
    fn of(motion: &Motion, living: Option<&Living>, dying: bool) -> Self {
        match living {
            Some(living) if !dying => Self {
                body_yaw: living.body_yaw,
                head_yaw: living.head_yaw,
                walk_position: living.walk.position * living.walk.scale,
                walk_speed: living.walk.speed,
            },
            Some(living) => Self {
                body_yaw: living.body_yaw,
                head_yaw: living.head_yaw,
                ..Self::default()
            },
            None => Self {
                body_yaw: motion.current.yaw,
                head_yaw: motion.current.yaw,
                ..Self::default()
            },
        }
    }
}

/// Writes the tick record, the cull record and the tick values of the state of each entity
/// that changed in a tick.
pub(crate) fn write_ticks(
    clock: Res<EntityClock>,
    assets: Option<Res<EntityAssets>>,
    columns: Option<Res<ColumnStore>>,
    mut arena: ResMut<SlotArena>,
    entities: Query<
        (
            Ref<EntitySlot>,
            &ClientEntity,
            &Motion,
            &Age,
            Option<&Living>,
            &SyncedData,
        ),
        Or<(
            Changed<Motion>,
            Changed<Living>,
            Changed<Age>,
            Changed<SyncedData>,
            Added<EntitySlot>,
        )>,
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    for (slot, client, motion, age, living, data) in &entities {
        let position = motion.current.position;
        let block = position.floor();
        let fraction = ((position - block) * 65536.0)
            .as_uvec3()
            .min(glam::UVec3::splat(65535));
        let dying = data.is_dying();
        let facing = Facing::of(motion, living, dying);
        let pitch = (motion.current.pitch / 180.0 * 32767.0)
            .round()
            .clamp(-32767.0, 32767.0) as i16;
        let block = block.as_ivec3();
        let record = TickRecord {
            block: block.to_array(),
            fraction: [fraction.x as u16, fraction.y as u16, fraction.z as u16],
            body_yaw: TickRecord::turn_from_degrees(facing.body_yaw),
            head_yaw: TickRecord::turn_from_degrees(facing.head_yaw),
            pitch,
            walk_position: facing.walk_position.to_bits(),
            walk_speed: facing.walk_speed.to_bits(),
        };
        let fresh = slot.is_added();
        let moved = arena
            .tick(slot.slot)
            .is_none_or(|held| held.block != record.block)
            || fresh;
        arena.write_tick(slot.slot, clock.tick, record, fresh);

        let scale = data.scale as f32;
        let radius = assets
            .radii
            .get(usize::from(slot.definition))
            .copied()
            .unwrap_or(1.0)
            * scale
            + MOVEMENT_MARGIN;
        let death_time = living.map_or(0, |living| living.death_time);
        arena.write_cull(
            slot.slot,
            CullRecord {
                block: block.to_array(),
                radius: f16_up(radius),
                flags: CULL_VALID
                    | if death_time > 0 || data.upside_down {
                        CULL_FULL_RIG_ONLY
                    } else {
                        0
                    },
            },
        );

        let Some(mut state) = arena.state(slot.slot).copied() else {
            continue;
        };
        let age_now = age.0 as i32;
        state.clock_offset = clock.tick as i32 - age_now;
        state.hurt_start = match living {
            Some(living) if living.hurt_time > 0 => {
                (age_now - (HURT_TICKS as i32 - living.hurt_time as i32)) as f32
            }
            _ if fresh => NEVER_HURT,
            _ => state.hurt_start,
        };
        if death_time > 0 {
            state.flags |= STATE_DYING;
            state.death_start = (age_now - death_time as i32) as f32;
        } else {
            state.flags &= !STATE_DYING;
        }
        state.scale = f16_up(scale);
        if moved {
            let type_data = client.type_data();
            let eye = type_data.map_or(0.0, |type_data| {
                type_data.eye_height
                    * if data.baby {
                        type_data.baby_scale.unwrap_or(1.0)
                    } else {
                        1.0
                    }
            });
            let eye = (position + glam::DVec3::Y * f64::from(eye) * data.scale)
                .floor()
                .as_ivec3();
            let light = columns
                .as_deref()
                .and_then(|columns| columns.light(BlockPos::new(eye.x, eye.y, eye.z)).ok())
                .unwrap_or(0x0f);
            state.sky_light = light & 0x0f;
            state.block_light = light >> 4;
            if data.is_on_fire(client.type_data()) {
                state.block_light = 15;
            }
        }
        arena.write_state(slot.slot, state);
    }
}

/// The equipment asset identifier of the stack in `slot`.
fn equipment_asset(
    equipment: Option<&Equipment>,
    slot: EquipmentSlot,
    items: Option<&Items>,
) -> Option<String> {
    match equipment?.asset(slot) {
        EquippableAsset::None => None,
        EquippableAsset::Patched(asset) => Some(asset.to_string()),
        EquippableAsset::ItemDefault(item) => {
            let items = items?;
            let id = Id::checked(item.number(), items.len())?;
            let equippable = items.get(id)?.prototype.get::<Equippable>()?;
            equippable.asset_id.as_ref().map(ToString::to_string)
        }
    }
}

/// The value of a query of a render controller, from the tick state of an entity and its
/// variant entry.
fn render_query(
    query: &RenderQuery,
    assets: &EntityAssets,
    variant: Option<VariantEntry>,
    data: &SyncedData,
    equipment: Option<&Equipment>,
    items: Option<&Items>,
) -> Option<Value> {
    let flag = |value: bool| Value::Number(if value { 1.0 } else { 0.0 });
    let text = |value: &str| Value::String(string_hash(value));
    let empty = || Value::String(string_hash(""));
    Some(match query {
        RenderQuery::Entity(EntityQuery::IsBaby) => flag(data.baby),
        RenderQuery::Entity(EntityQuery::Variant) => {
            Value::Number(variant.map_or(0.0, |variant| variant.id))
        }
        RenderQuery::Entity(EntityQuery::VariantIdentifier) => {
            variant.map_or_else(empty, |variant| Value::String(variant.identifier))
        }
        RenderQuery::Entity(EntityQuery::VariantAsset) => {
            variant.map_or_else(empty, |variant| Value::String(variant.asset))
        }
        RenderQuery::Entity(EntityQuery::VariantModel) => {
            variant.map_or_else(empty, |variant| Value::String(variant.model))
        }
        RenderQuery::Entity(EntityQuery::Pose) => text(pose_name(data.pose)),
        RenderQuery::Entity(EntityQuery::IsSaddled) => {
            flag(equipment.is_some_and(Equipment::is_saddled))
        }
        RenderQuery::Entity(EntityQuery::IsAlive) => flag(!data.is_dying()),
        RenderQuery::EquipmentTexture { slot, layer } => {
            let asset = slot
                .parse::<EquipmentSlot>()
                .ok()
                .and_then(|slot| equipment_asset(equipment, slot, items));
            let texture =
                asset.and_then(|asset| assets.equipment_textures.get(&(asset, layer.clone())));
            text(texture.map_or("", String::as_str))
        }
        RenderQuery::Entity(_) => Value::Number(0.0),
    })
}

type LayerInputs = (
    &'static EntitySlot,
    &'static ClientEntity,
    &'static SyncedData,
    Option<&'static Equipment>,
    Option<&'static PlayerModel>,
    Option<&'static SkinSlot>,
);

/// Evaluates the render controllers of each entity whose tick state changed and writes the
/// layers of its state record.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn write_layers(
    assets: Option<Res<EntityAssets>>,
    items: Option<Res<Items>>,
    mut vm: ResMut<EntityVm>,
    mut arena: ResMut<SlotArena>,
    mut last_generation: Local<Option<u32>>,
    mut removed_models: RemovedComponents<PlayerModel>,
    mut removed_slots: RemovedComponents<SkinSlot>,
    changed: Query<
        LayerInputs,
        Or<(
            Changed<SyncedData>,
            Changed<Equipment>,
            Added<EntitySlot>,
            Changed<PlayerModel>,
            Changed<SkinSlot>,
        )>,
    >,
    all: Query<LayerInputs>,
) {
    let Some(assets) = assets else {
        return;
    };
    let rebuilt = *last_generation != Some(assets.generation);
    *last_generation = Some(assets.generation);
    let items = items.as_deref();
    let mut write = |(slot, client, data, equipment, model, skin_slot): (
        &EntitySlot,
        &ClientEntity,
        &SyncedData,
        Option<&Equipment>,
        Option<&PlayerModel>,
        Option<&SkinSlot>,
    )| {
        let (Some(entity), Some(tables)) = (
            assets.pack.entities.get(usize::from(slot.definition)),
            assets.library.definitions.get(usize::from(slot.definition)),
        ) else {
            return;
        };
        let variant = match model {
            Some(model) => Some(skin_entry(*model, skin_slot, &assets)),
            None => assets
                .variant(client.kind, data.variant)
                .map(|variant| variant.entry(data.baby)),
        };
        let choices = entity.controllers.evaluate(&mut vm.0, |query| {
            render_query(query, &assets, variant, data, equipment, items)
        });
        let Some(mut state) = arena.state(slot.slot).copied() else {
            return;
        };
        state.definition = slot.definition;
        state.layer_mask = 0;
        state.layers = [[0; 2]; MAX_LAYERS];
        let mut hidden = 0u16;
        for (layer, choice) in choices.iter().take(MAX_LAYERS).enumerate() {
            let Some(Some(model)) = tables.models.get(choice.geometry) else {
                continue;
            };
            let texture = match choice.texture {
                TextureChoice::Identifier(hash) => assets.texture(hash),
                TextureChoice::Named(short) => {
                    let path = entity
                        .resources
                        .textures
                        .get(short)
                        .map(|(_, path)| path.as_str());
                    assets.texture(string_hash(&texture_identifier(path.unwrap_or(""))))
                }
            };
            if let Some(target) = state.layers.get_mut(layer) {
                *target = [*model, texture];
            }
            state.layer_mask |= 1 << layer;
            hidden |= tables.hidden_mask(choice);
        }
        state.flags = (state.flags & 0xffff & !STATE_BABY)
            | if data.baby { STATE_BABY } else { 0 }
            | (u32::from(hidden) << STATE_HIDDEN_SHIFT);
        state.section = u32::MAX;
        state.input = u32::MAX;
        if tables
            .program
            .as_ref()
            .is_ok_and(|program| program.inputs.is_used())
        {
            state.input = slot.slot;
        }
        arena.write_state(slot.slot, state);
    };
    if rebuilt {
        for entity in &all {
            write(entity);
        }
    } else {
        for entity in &changed {
            write(entity);
        }
        // A removal sets no change filter, so an entity that lost its model or its skin writes here.
        for entity in removed_models.read().chain(removed_slots.read()) {
            if let Ok(entity) = all.get(entity) {
                write(entity);
            }
        }
    }
}

/// Runs the scripts and the play list of each entity whose definition reads CPU values, and
/// writes its input record when a value changed.
pub(crate) fn write_inputs(
    assets: Option<Res<EntityAssets>>,
    clock: Res<EntityClock>,
    mut vm: ResMut<EntityVm>,
    mut arena: ResMut<SlotArena>,
    mut entities: Query<(
        &EntitySlot,
        &mut CpuAnimation,
        Option<&RecordedClips>,
        RenderInputsQuery,
    )>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (slot, mut animation, recorded, inputs) in &mut entities {
        let (Some(entity), Some(Ok(program))) = (
            assets.pack.entities.get(usize::from(slot.definition)),
            assets
                .library
                .definitions
                .get(usize::from(slot.definition))
                .map(|tables| tables.program.as_ref()),
        ) else {
            continue;
        };
        let inputs = RenderInputs::from(inputs);
        let frame = inputs.frame(clock.partial_tick);
        let variant = assets
            .variant(inputs.entity.kind, inputs.data.variant)
            .map(|variant| variant.entry(frame.baby));
        let source = FrameQueries {
            frame: &frame,
            variant,
            time_of_day: 0.0,
            in_water: false,
        };
        animation
            .0
            .update(&entity.rig, &mut vm.0, frame.clock, &source);
        for clip in recorded.iter().flat_map(|recorded| &recorded.0) {
            if let Some(entry) = entity.rig.entry(&clip.entry) {
                animation.0.set_weight(entry, 1.0);
                animation
                    .0
                    .set_time(&entity.rig, entry, clip.elapsed_ticks / 20.0);
            }
        }
        let record = input_record(&program.inputs, &entity.rig, &animation.0, frame.clock);
        arena.write_input(slot.slot, record);
    }
}
