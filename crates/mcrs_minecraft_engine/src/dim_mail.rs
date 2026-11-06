//! What a dimension world sends other parts besides the packets of its clients: the orders for
//! players of other worlds, the clock changes for the coordinator and the command runs that go
//! on elsewhere. The world hands them over in the output of its tick.

use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_environment::world_clock::WorldClocks;

use crate::command_handoff::CommandHandoff;
use crate::coordinator::clock::EditClock;
use crate::player_order::OrderedPlayer;

/// The messages of this tick for other parts, each class in the order of writing.
#[derive(Resource, Default)]
pub struct DimMail {
    pub(crate) orders: Vec<OrderedPlayer>,
    pub(crate) clock_edits: Vec<EditClock>,
    pub(crate) handoffs: Vec<CommandHandoff>,
}

impl DimMail {
    /// Sends `order` to the world that holds its player.
    pub fn order(&mut self, order: OrderedPlayer) {
        self.orders.push(order);
    }

    /// Sends the rest of a command run to the world of its dimension.
    pub fn hand_off(&mut self, handoff: CommandHandoff) {
        self.handoffs.push(handoff);
    }
}

/// Applies `edit` to the copy of the clocks in this world at once, so the rest of the tick reads
/// it, and sends it to the coordinator, which owns the clocks and tells every world.
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
