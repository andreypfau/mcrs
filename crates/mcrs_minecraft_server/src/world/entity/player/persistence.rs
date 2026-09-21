use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, With};
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::{Op, Slot, Transaction};
use mcrs_minecraft_item::inventory::slots;
use mcrs_minecraft_item::{Items, SelectedHotbarSlot, SlotTable, stack_to_value};
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::{DimensionId, InDimension};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::ItemStackWithSlot;
use mcrs_minecraft_world::save::{PlayerDat, WORLD_VERSION, read_player_dat, write_player_dat};
use tracing::{error, warn};

use crate::WorldSave;
use crate::login::GameProfile;

/// Vanilla's own autosave cadence, in ticks.
pub const AUTOSAVE_INTERVAL: u32 = 6000;

/// Root keys of the loaded file mcrs does not model, written back unchanged.
#[derive(Component, Default, Debug, Clone)]
pub struct LoadedPlayerDat(pub NbtCompound);

/// The file on disk could not be parsed; saving would replace it with the
/// empty inventory the player joined with.
#[derive(Component, Debug, Clone, Copy)]
pub struct UnreadablePlayerDat;

const EQUIPMENT: [(&str, u16); 5] = [
    ("feet", slots::ARMOR_FEET),
    ("legs", slots::ARMOR_LEGS),
    ("chest", slots::ARMOR_CHEST),
    ("head", slots::ARMOR_HEAD),
    ("offhand", slots::OFFHAND),
];

const VANILLA_INVENTORY_SIZE: u8 = 36;

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
    for entry in dat
        .inventory
        .iter()
        .filter(|entry| entry.slot < VANILLA_INVENTORY_SIZE)
    {
        let Some(cell) = slots::from_inventory_index(entry.slot) else {
            continue;
        };
        place(world, player, cell, &entry.stack);
    }
    for (key, stack) in &dat.equipment {
        match EQUIPMENT.iter().find(|(name, _)| name == key) {
            Some((_, cell)) => place(world, player, *cell, stack),
            None => warn!(%uuid, "equipment slot {key} is not a player slot; dropped"),
        }
    }
    let selected = match u8::try_from(dat.selected_item_slot) {
        Ok(slot) if slot <= 8 => slot,
        _ => {
            warn!(%uuid, "SelectedItemSlot {} out of range; using 0", dat.selected_item_slot);
            0
        }
    };
    world
        .entity_mut(player)
        .insert((SelectedHotbarSlot(selected), LoadedPlayerDat(dat.rest)));
}

fn place(
    world: &mut World,
    player: Entity,
    cell: u16,
    stack: &mcrs_minecraft_protocol::item::ItemStackValue,
) {
    Transaction(vec![Op::Spawn {
        value: stack.clone(),
        to: Slot::new(player, cell),
    }])
    .apply(world);
}

pub fn save_player(world: &World, player: Entity) -> PlayerDat {
    let items = world.resource::<Items>();
    let table = world.get::<SlotTable>(player);
    let mut inventory = Vec::new();
    let mut equipment = std::collections::BTreeMap::new();
    if let Some(table) = table {
        for (cell, stack) in table.iter() {
            let stack = stack_to_value(world, stack, items);
            if let Some((name, _)) = EQUIPMENT.iter().find(|(_, c)| *c == cell) {
                equipment.insert((*name).to_owned(), stack);
            } else if let Some(slot) = slots::inventory_index(cell) {
                inventory.push(ItemStackWithSlot { slot, stack });
            }
        }
        inventory.sort_by_key(|entry| entry.slot);
    }
    let transform = world
        .get::<Transform>(player)
        .copied()
        .unwrap_or(Transform::IDENTITY);
    let rotation: Rotation = transform.rotation;
    let dimension = world
        .get::<InDimension>(player)
        .and_then(|dim| world.get::<DimensionId>(dim.0))
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "minecraft:overworld".to_owned());
    PlayerDat {
        data_version: WORLD_VERSION,
        pos: transform.translation.to_array(),
        rotation: [rotation.yaw(), rotation.pitch()],
        dimension,
        inventory,
        selected_item_slot: world
            .get::<SelectedHotbarSlot>(player)
            .map_or(0, |selected| i32::from(selected.0)),
        equipment,
        rest: world
            .get::<LoadedPlayerDat>(player)
            .map(|loaded| loaded.0.clone())
            .unwrap_or_default(),
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
    if let Err(err) = write_player_dat(&save.0, uuid, &save_player(world, player)) {
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
