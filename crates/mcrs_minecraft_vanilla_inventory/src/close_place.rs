use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::PlayerHolders;
use crate::close_inventory_menu::Carried;

/// What the slot of a close does with the stack that the close moves into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    Refuses,
    /// The slot takes the stack up to its slot limit `limit` (ITX-LIM-001).
    Permits {
        limit: u8,
    },
}

/// The answer of the place gate and the slot limit of the slot `index` of `player` for the stack
/// that a close moves out of `from`, by the same [`SlotPolicy`] that the commit reads
/// (INV-CLS-005).
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
    /// A slot without a slot policy: the maximum stack size alone bounds it.
    pub(crate) const OPEN: Self = Self::Permits { limit: u8::MAX };
}
