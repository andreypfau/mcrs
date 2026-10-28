use crate::entity::player::Player;
use crate::tick::TickSet;
use crate::world::lifecycle::level::SectionLevels;
use crate::world::lifecycle::ticket::SectionTickets;
use crate::world::storage::column::ColumnIndex;
use crate::world::storage::section::SectionIndex;
use crate::world::storage::section::SectionPlugin;
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::{
    Added, Bundle, Changed, Commands, Component, ContainsEntity, Entity, IntoScheduleConfigs, On,
    Query, Ref, With,
};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{ResourceKey, SectionPos};
use mcrs_minecraft_dimension::DimensionType;
use std::collections::BTreeSet;

pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                TickSet::Input,
                TickSet::Simulate,
                TickSet::WorldWrite,
                TickSet::Project,
                TickSet::Output,
            )
                .chain(),
        );
        app.add_plugins(SectionPlugin);
        app.add_plugins(crate::world::storage::column::ColumnPlugin);
        // Dimension entities are spawned by the game layer; this plugin only
        // maintains them once they exist.
        app.add_systems(
            FixedPostUpdate,
            (add_old_in_dimension, update_index, update_old_in_dimensions).chain(),
        );
        app.add_observer(forget_removed_player);
    }
}

#[derive(Bundle)]
pub struct DimensionBundle {
    pub dimension: Dimension,
    pub key: ResourceKey<mcrs_minecraft_dimension::Dimension>,
    pub type_config: DimensionTypeConfig,
    pub chunk_index: SectionIndex,
    pub tickets: SectionTickets,
    pub levels: SectionLevels,
    pub players: DimensionPlayers,
    pub column_index: ColumnIndex,
}

impl DimensionBundle {
    pub fn new(
        key: ResourceKey<mcrs_minecraft_dimension::Dimension>,
        type_config: DimensionTypeConfig,
    ) -> Self {
        let min_section_y = type_config.min_y >> SectionPos::BITS;
        let max_section_y = min_section_y + type_config.section_count as i32 - 1;
        Self {
            dimension: Dimension,
            key,
            type_config,
            chunk_index: SectionIndex::default(),
            tickets: SectionTickets::default(),
            levels: SectionLevels::bounded(min_section_y..=max_section_y),
            players: DimensionPlayers::default(),
            column_index: ColumnIndex::default(),
        }
    }
}

#[derive(Component, Default)]
pub struct Dimension;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HasSkyLight;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HasWeather;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimensionTypeId(pub Id<DimensionType>);

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

/// The vertical extent of a dimension, supplied by the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct DimensionTypeConfig {
    pub min_y: i32,
    pub height: u32,
    pub section_count: u32,
}

impl DimensionTypeConfig {
    pub fn new(min_y: i32, height: u32) -> Self {
        Self {
            min_y,
            height,
            section_count: height >> SectionPos::BITS,
        }
    }

    /// Returns the maximum Y coordinate (min_y + height - 1)
    pub fn max_y(&self) -> i32 {
        self.min_y + self.height as i32 - 1
    }
}

fn update_index(
    entities: Query<(Entity, &OldInDimension, Ref<InDimension>), With<Player>>,
    mut dimensions: Query<&mut DimensionPlayers>,
) {
    entities
        .iter()
        .filter(|(.., in_dimension)| in_dimension.is_changed())
        .for_each(|(player, old_in_dimension, in_dimension)| {
            if let Ok(mut viewers) = dimensions.get_mut(**old_in_dimension) {
                let removed = viewers.0.remove(&player);
                debug_assert!(removed);
            }
            if let Ok(mut viewers) = dimensions.get_mut(**in_dimension) {
                let inserted = viewers.0.insert(player);
                debug_assert!(inserted);
            }
        })
}

/// A player that leaves before `update_index` ran is still in the set of its old dimension.
fn forget_removed_player(
    remove: On<Remove, InDimension>,
    players: Query<(&InDimension, Option<&OldInDimension>), With<Player>>,
    mut dimensions: Query<&mut DimensionPlayers>,
) {
    let player = remove.event().entity;
    let Ok((in_dimension, old_in_dimension)) = players.get(player) else {
        return;
    };
    for dimension in [Some(in_dimension.0), old_in_dimension.map(|old| old.0)]
        .into_iter()
        .flatten()
    {
        if let Ok(mut viewers) = dimensions.get_mut(dimension) {
            viewers.0.remove(&player);
        }
    }
}

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

pub(crate) fn update_old_in_dimensions(
    mut clients: Query<(&mut OldInDimension, &InDimension), Changed<InDimension>>,
) {
    clients
        .iter_mut()
        .for_each(|(mut old_in_dimension, in_dimension)| {
            old_in_dimension.0 = **in_dimension;
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_despawned_is_gone_from_the_players_of_every_dimension() {
        let mut app = App::new();
        app.add_plugins(DimensionPlugin);
        let [a, b] = [(); 2].map(|()| app.world_mut().spawn(DimensionPlayers::default()).id());
        let [staying, moving] =
            [(); 2].map(|()| app.world_mut().spawn((Player, InDimension(a))).id());
        app.world_mut().run_schedule(FixedPostUpdate);

        app.world_mut().get_mut::<InDimension>(moving).unwrap().0 = b;
        app.world_mut().despawn(moving);
        app.world_mut().despawn(staying);

        for dimension in [a, b] {
            assert!(
                app.world()
                    .get::<DimensionPlayers>(dimension)
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
