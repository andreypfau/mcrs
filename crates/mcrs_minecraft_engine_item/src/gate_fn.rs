use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::GateQuery;

pub type GateFn = fn(&World, &GateQuery) -> Filter;
