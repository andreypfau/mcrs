use bevy_ecs::prelude::{Event, Resource};
use mcrs_minecraft_environment::{ClockEdit, WorldClock};
use mcrs_minecraft_resource::Id;

#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub struct EditClock {
    pub clock: Id<WorldClock>,
    pub edit: ClockEdit,
}

#[derive(
    Resource, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct GameTime(pub i64);
