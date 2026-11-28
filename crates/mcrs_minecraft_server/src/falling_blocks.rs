use std::sync::LazyLock;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::prelude::World;
use mcrs_minecraft_engine_world::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_falling_block::{ConcretePairsFile, PackConcretePairs};
use mcrs_minecraft_vanilla_falling_block::FallingBlockPlugin;

use crate::entity_motion::add_entity_motion;

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
