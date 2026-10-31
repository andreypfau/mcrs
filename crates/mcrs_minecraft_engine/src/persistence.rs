use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, With};
use bevy_ecs::world::World;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::{ItemStackValue, ItemStackWithSlot};
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::ResourceKey;
use mcrs_minecraft_save::{PlayerDat, read_player_dat, write_player_dat};
use std::collections::BTreeMap;
use tracing::error;

use crate::session::GameProfile;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;

/// Vanilla's own autosave cadence, in ticks.
pub const AUTOSAVE_INTERVAL: u32 = 6000;

/// The parts of the loaded file mcrs does not simulate, written back unchanged.
#[derive(Component, Default, Debug, Clone)]
pub struct LoadedPlayerDat {
    pub inventory: Vec<ItemStackWithSlot<Named>>,
    pub selected_item_slot: i32,
    pub equipment: BTreeMap<String, ItemStackValue<Named>>,
    pub rest: NbtCompound,
}

/// The file on disk could not be parsed; saving would replace it with the
/// empty player the session joined with.
#[derive(Component, Debug, Clone, Copy)]
pub struct UnreadablePlayerDat;

pub fn load_player(world: &mut World, player: Entity) {
    let Some(save) = world.get_resource::<WorldSave>() else {
        return;
    };
    let Some(uuid) = world.get::<GameProfile>(player).map(|profile| profile.id) else {
        return;
    };
    let dat = match read_player_dat(&save.0, uuid) {
        Ok(Some(dat)) => dat,
        Ok(None) => return,
        Err(err) => {
            error!(%uuid, "player data unreadable, joining empty and never saved: {err}");
            world.entity_mut(player).insert(UnreadablePlayerDat);
            return;
        }
    };
    world.entity_mut(player).insert(LoadedPlayerDat {
        inventory: dat.inventory,
        selected_item_slot: dat.selected_item_slot,
        equipment: dat.equipment,
        rest: dat.rest,
    });
}

pub fn save_player(world: &World, player: Entity) -> PlayerDat {
    let transform = world
        .get::<Transform>(player)
        .copied()
        .unwrap_or(Transform::IDENTITY);
    let dimension = world
        .get::<InDimension>(player)
        .and_then(|dim| world.get::<ResourceKey<Dimension>>(dim.0))
        .cloned()
        .unwrap_or_else(|| mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into());
    let loaded = world
        .get::<LoadedPlayerDat>(player)
        .cloned()
        .unwrap_or_default();
    PlayerDat {
        data_version: VERSION.world_version,
        pos: transform.translation.to_array(),
        rotation: [transform.rotation.yaw(), transform.rotation.pitch()],
        dimension,
        inventory: loaded.inventory,
        selected_item_slot: loaded.selected_item_slot,
        equipment: loaded.equipment,
        rest: loaded.rest,
    }
}

pub fn write_player(world: &World, player: Entity) {
    let Some(save) = world.get_resource::<WorldSave>() else {
        return;
    };
    let Some(uuid) = world.get::<GameProfile>(player).map(|profile| profile.id) else {
        return;
    };
    if world.get::<UnreadablePlayerDat>(player).is_some() {
        return;
    }
    let dat = save_player(world, player);
    if let Err(err) = write_player_dat(&save.0, uuid, &dat) {
        error!(%uuid, "player data not saved: {err}");
    }
}

pub fn autosave_players(world: &mut World) {
    let players: Vec<Entity> = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .collect();
    for player in players {
        write_player(world, player);
    }
}
