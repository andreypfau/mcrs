use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_item::DirtyStacks;

/// Every stack mutation of a tick runs in `Mutate`; `Sync` then reads the
/// `DirtyStacks` queue once, so readers outside these sets see a table that
/// is at most one tick stale, never half-applied.
#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum StackSet {
    Mutate,
    Sync,
}

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(FixedUpdate, (StackSet::Mutate, StackSet::Sync).chain())
            .init_resource::<DirtyStacks>();
    }
}
