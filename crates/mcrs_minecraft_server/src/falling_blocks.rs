use std::sync::LazyLock;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::prelude::World;
use mcrs_minecraft_engine_world::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_falling_block::{ConcretePairsFile, PackConcretePairs};
use mcrs_minecraft_vanilla_falling_block::FallingBlockPlugin;

use crate::entity_motion::add_entity_motion;

// chisle: the pairs of the mcrs pack are built in, so a pack that adds or replaces pairs is not
// read. A loader of the non-registry files of each pack, for plugins, lifts it.
const MCRS_CONCRETE_PAIRS: &str =
    include_str!("../../../assets/mcrs/datapacks/mcrs/mcrs/falling_block/concrete_pairs.json");

static CONCRETE_PAIRS: LazyLock<Vec<PackConcretePairs>> =
    LazyLock::new(
        || match serde_json::from_str::<ConcretePairsFile>(MCRS_CONCRETE_PAIRS) {
            Ok(file) => vec![PackConcretePairs {
                pack: "mcrs".to_owned(),
                file,
            }],
            Err(error) => {
                tracing::warn!("the concrete pairs of the mcrs pack do not read: {error}");
                Vec::new()
            }
        },
    );

/// The falling block in every dimension world, moved by the motion pass. It needs the tables of
/// [`VanillaEntityPhysicsPlugin`](crate::VanillaEntityPhysicsPlugin); its drops move with
/// [`VanillaItemEntityPlugin`](crate::VanillaItemEntityPlugin).
#[derive(Clone, Copy, Debug, Default)]
pub struct VanillaFallingBlockPlugin;

impl Plugin for VanillaFallingBlockPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(build_dimension_falling_blocks);
    }
}

fn build_dimension_falling_blocks(_: &World, _: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    add_entity_motion(sub_app);
    sub_app.add_plugins(FallingBlockPlugin {
        concrete_pairs: CONCRETE_PAIRS.clone(),
        ..Default::default()
    });
}
