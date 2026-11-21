use bevy_ecs::component::Component;
use mcrs_minecraft_swing::SwingTracker;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobSwing(pub SwingTracker);
