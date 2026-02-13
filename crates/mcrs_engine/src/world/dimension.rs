use crate::entity::despawn::Despawned;
use crate::entity::player::Player;
use crate::world::chunk::ticket::ChunkTicketsCommands;
use crate::world::chunk::{ChunkIndex, ChunkPlugin};
use bevy_app::{App, FixedPostUpdate, Plugin, PreStartup};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    Added, Bundle, Changed, Commands, Component, ContainsEntity, Entity, Has, IntoScheduleConfigs,
    Mut, Query, Ref, With,
};
use std::collections::BTreeSet;

pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ChunkPlugin);
        app.add_systems(PreStartup, spawn_dimension);
        app.add_systems(
            FixedPostUpdate,
            (add_old_in_dimension, update_index, update_old_in_dimensions).chain(),
        );
        app.add_systems(FixedPostUpdate, update_time);
    }
}

#[derive(Bundle, Default)]
pub struct DimensionBundle {
    pub dimension: Dimension,
    pub chunk_index: ChunkIndex,
    pub chunk_tickets: ChunkTicketsCommands,
    pub players: DimensionPlayers,
}

#[derive(Component, Default)]
pub struct Dimension;

#[derive(Component, Clone, Default, Deref, Debug)]
pub struct DimensionPlayers(BTreeSet<Entity>);

#[derive(Component, Clone, Debug, Copy, PartialEq, Eq, DerefMut, Deref)]
pub struct InDimension(pub Entity);

impl ContainsEntity for InDimension {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Deref, Debug, PartialEq, Eq)]
pub struct OldInDimension(Entity);

impl ContainsEntity for OldInDimension {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Component, Deref, DerefMut)]
pub struct DimensionTime(pub u64);

/// Unique identifier for a dimension (e.g., "minecraft:overworld", "minecraft:the_nether")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Component, Deref, DerefMut)]
pub struct DimensionId(pub String);

impl DimensionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Configuration derived from the dimension type, containing Y-level and section metadata.
/// Used for chunk loading and column view calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct DimensionTypeConfig {
    /// Minimum Y coordinate for this dimension (e.g., -64 for overworld, 0 for nether)
    pub min_y: i32,
    /// Total height in blocks (e.g., 384 for overworld, 256 for nether)
    pub height: u32,
    /// Number of chunk sections (height / 16)
    pub section_count: u32,
}

impl DimensionTypeConfig {
    /// Create a new DimensionTypeConfig from min_y and height.
    /// Section count is automatically calculated as height / 16.
    pub fn new(min_y: i32, height: u32) -> Self {
        Self {
            min_y,
            height,
            section_count: height / 16,
        }
    }

    /// Returns the maximum Y coordinate (min_y + height - 1)
    pub fn max_y(&self) -> i32 {
        self.min_y + self.height as i32 - 1
    }
}

impl Default for DimensionTypeConfig {
    fn default() -> Self {
        // Default to overworld values
        Self::new(-64, 384)
    }
}

fn update_time(mut dimension_time: Query<Mut<DimensionTime>>) {
    dimension_time.iter_mut().for_each(|mut dimension_time| {
        **dimension_time = dimension_time.wrapping_add(1);
    });
}

fn spawn_dimension(mut commands: Commands) {
    commands.spawn(DimensionBundle::default());
}

#[allow(clippy::type_complexity)]
fn update_index(
    entities: Query<(Entity, Has<Despawned>, &OldInDimension, Ref<InDimension>), With<Player>>,
    mut dimensions: Query<&mut DimensionPlayers>,
) {
    entities
        .iter()
        .for_each(|(player, is_despawned, old_in_dimension, in_dimension)| {
            if is_despawned {
                if let Ok(mut viewers) = dimensions.get_mut(**old_in_dimension) {
                    let removed = viewers.0.remove(&player);
                    debug_assert!(removed);
                }
            } else if in_dimension.is_changed() {
                if let Ok(mut viewers) = dimensions.get_mut(**old_in_dimension) {
                    let removed = viewers.0.remove(&player);
                    debug_assert!(removed);
                }

                if let Ok(mut viewers) = dimensions.get_mut(**in_dimension) {
                    let inserted = viewers.0.insert(player);
                    debug_assert!(inserted);
                }
            }
        })
}

#[allow(clippy::type_complexity)]
fn add_old_in_dimension(
    mut commands: Commands,
    new_players: Query<(Entity, &InDimension), (With<Player>, Added<InDimension>)>,
    mut dimensions: Query<&mut DimensionPlayers>,
) {
    new_players.iter().for_each(|(entity, in_dimension)| {
        commands
            .entity(entity)
            .insert(OldInDimension(**in_dimension));
        let Ok(mut dim) = dimensions.get_mut(**in_dimension) else {
            return;
        };
        dim.0.insert(entity);
    });
}

fn update_old_in_dimensions(
    mut clients: Query<(&mut OldInDimension, &InDimension), Changed<InDimension>>,
) {
    clients
        .iter_mut()
        .for_each(|(mut old_in_dimension, in_dimension)| {
            old_in_dimension.0 = **in_dimension;
        });
}
