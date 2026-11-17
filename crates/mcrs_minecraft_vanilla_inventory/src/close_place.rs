use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::close_inventory_menu::Carried;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    Refuses,
    Permits { limit: u8 },
}

pub(crate) fn close_place(
    world: &World,
    player: Entity,
    holders: PlayerHolders,
    from: Carried,
    index: u16,
) -> Place {
    let (holder, source) = match from {
        Carried::Cursor => (holders.cursor, 0),
        Carried::CraftGrid(source) => (holders.craft_grid, source),
    };
    let stack = world
        .get::<Slots>(holder)
        .and_then(|slots| slots.get(usize::from(source)));
    let (Some(stack), Some(policy)) = (stack, world.get::<SlotPolicy>(player)) else {
        return Place::OPEN;
    };
    let query = GateQuery {
        slot: SlotRef {
            holder: player,
            index,
        },
        actor: player,
        stack,
    };
    let access = policy.access(index);
    match access.place.answer(world, &query) {
        Filter::Permits => Place::Permits {
            limit: access.limit,
        },
        Filter::Refuses => Place::Refuses,
    }
}

impl Place {
    pub(crate) const OPEN: Self = Self::Permits { limit: u8::MAX };
}
