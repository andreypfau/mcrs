pub mod features;
pub mod modern_carvers;
pub mod routers;
pub mod structures;

use bevy_app::{App, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
pub use routers::DimensionRouters;

/// The terrain, carvers, features and structures of every dimension. Without it a dimension
/// generates no column, and a world save is not read either.
pub struct WorldGenPlugin;

impl Plugin for WorldGenPlugin {
    fn build(&self, app: &mut App) {
        crate::require::<Self, crate::DimensionWorldsPlugin>(app);
        app.add_plugins((
            modern_carvers::ModernCarverPlugin,
            features::FeaturePlugin,
            structures::StructurePlugin,
        ));
        // A sub-app takes its router at spawn, and the runner drains the spawn queue right
        // after this update.
        app.add_systems(
            OnEnter(AppState::Playing),
            routers::build_dimension_routers
                .after(structures::build_dimension_structures)
                .before(crate::world::enqueue_dim_spawns),
        );
    }
}
