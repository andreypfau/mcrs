use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::GateQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GateId(pub(crate) u16);

pub type GateFn = fn(&World, &GateQuery) -> Filter;
