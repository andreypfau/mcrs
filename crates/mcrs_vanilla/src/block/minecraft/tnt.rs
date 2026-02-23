use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "tnt",
    protocol_id: 176,
    base_state_id: 2140,
    properties: [&state_properties::UNSTABLE],
    default: { unstable: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::FIRE)
        .with_strength(0.0)
        .ignited_by_lava()
        .instant_break()
}

use crate::block_state;
use crate::entity::explosive::primed_tnt::{Detonator, PrimedTntBundle, DEFAULT_FUSE_DURATION};
use crate::entity::player::InstantBuild;
use crate::entity::EntityOwner;
use crate::explosion::BlockExplodedEvent;
use crate::player_action::PlayerWillDestroyBlock;
use bevy_app::Plugin;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::On;
use bevy_ecs::query::{Has, With};
use bevy_ecs::system::{Commands, Local, Query};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::dimension::InDimension;
use rand::rngs::SmallRng;
use rand::{Rng, RngExt, SeedableRng};

pub struct TntBlockPlugin;

impl Plugin for TntBlockPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_message::<PlayerWillDestroyBlock>();
        app.add_systems(bevy_app::FixedUpdate, player_will_destroy_tnt);
        app.add_observer(tnt_block_exploded);
    }
}

fn player_will_destroy_tnt(
    mut messages: MessageReader<PlayerWillDestroyBlock>,
    player: Query<(Has<InstantBuild>, &InDimension), With<Player>>,
    mut commands: Commands,
) {
    let unstable_state = block_state!(BLOCK, { unstable: true });
    messages.read().for_each(|event| {
        if event.block_state != unstable_state {
            return;
        }
        let Some((instant_build, dim)) = player.get(event.player).ok() else {
            return;
        };
        if instant_build {
            return;
        }
        commands.spawn((
            PrimedTntBundle::new(
                *dim,
                Transform::from_translation(event.block_pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)),
            ),
            EntityOwner(event.player),
            Detonator(event.player),
        ));
    });
}

fn tnt_block_exploded(
    event: On<BlockExplodedEvent>,
    mut commands: Commands,
    mut local_rng: Local<Option<SmallRng>>,
) {
    let blk = event.block_state_id;
    let default_state = BLOCK.default_state_id;
    let unstable_state = block_state!(BLOCK, { unstable: true });
    if blk != default_state && blk != unstable_state {
        return;
    }
    let rng = local_rng.get_or_insert_with(|| SmallRng::from_rng(&mut rand::rng()));
    let fuse = DEFAULT_FUSE_DURATION / 4 + rng.random_range(0..(DEFAULT_FUSE_DURATION / 8));

    let bundle = PrimedTntBundle::new(
        InDimension(event.dimension),
        Transform::from_translation(event.block_pos.as_dvec3() + DVec3::new(0.5, 0.0, 0.5)),
    )
    .with_fuse(fuse);

    let mut entity = commands.spawn(bundle);
    if let Some(detonator) = event.detonator {
        entity.insert(Detonator(detonator));
    }
}
