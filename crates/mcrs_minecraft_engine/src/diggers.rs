use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

/// The players whose `BreakSession` holds a dig or a delayed break, in the order that they
/// started. The block break systems alone write it: a start action adds its player, and the step
/// of the tick drops a player that is idle or gone.
#[derive(Resource, Default)]
pub(crate) struct Diggers(pub(crate) Vec<Entity>);
