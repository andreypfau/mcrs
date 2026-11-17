use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_environment::WorldClocks;

use crate::banlist::{BanList, EditBans};
use crate::command_handoff::CommandHandoff;
use crate::coordinator::clock::EditClock;
use crate::coordinator::ops::{EditOps, OpList};
use crate::player_order::OrderedPlayer;
use crate::server_edit::{self, ServerEdit};
use crate::whitelist::{EditWhitelist, Whitelist};

#[derive(Resource, Default)]
pub struct DimMail {
    pub(crate) orders: Vec<OrderedPlayer>,
    pub(crate) clock_edits: Vec<EditClock>,
    pub(crate) whitelist_edits: Vec<EditWhitelist>,
    pub(crate) ban_edits: Vec<EditBans>,
    pub(crate) ops_edits: Vec<EditOps>,
    pub(crate) server_edits: Vec<ServerEdit>,
    pub(crate) handoffs: Vec<CommandHandoff>,
}

impl DimMail {
    pub fn order(&mut self, order: OrderedPlayer) {
        self.orders.push(order);
    }

    pub fn hand_off(&mut self, handoff: CommandHandoff) {
        self.handoffs.push(handoff);
    }
}

pub fn edit_clock(world: &mut World, edit: EditClock) {
    if let Some(mut clocks) = world.get_resource_mut::<WorldClocks>()
        && let Some(state) = clocks.get_mut(edit.clock)
    {
        state.apply(edit.edit);
    }
    world
        .get_resource_or_init::<DimMail>()
        .clock_edits
        .push(edit);
}

pub fn edit_whitelist(world: &mut World, edit: EditWhitelist) {
    if edit != EditWhitelist::Reload
        && let Some(mut whitelist) = world.get_resource_mut::<Whitelist>()
    {
        whitelist.apply(&edit);
    }
    world
        .get_resource_or_init::<DimMail>()
        .whitelist_edits
        .push(edit);
}

pub fn edit_bans(world: &mut World, edit: EditBans) {
    if let Some(mut bans) = world.get_resource_mut::<BanList>() {
        bans.apply(&edit);
    }
    world.get_resource_or_init::<DimMail>().ban_edits.push(edit);
}

pub fn edit_ops(world: &mut World, edit: EditOps) {
    if let Some(mut ops) = world.get_resource_mut::<OpList>() {
        ops.apply(&edit);
    }
    world.get_resource_or_init::<DimMail>().ops_edits.push(edit);
}

pub fn edit_server(world: &mut World, edit: ServerEdit) {
    server_edit::apply_edit(world, edit);
    world
        .get_resource_or_init::<DimMail>()
        .server_edits
        .push(edit);
}
