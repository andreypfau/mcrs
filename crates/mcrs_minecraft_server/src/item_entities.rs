use bevy_app::{App, FixedUpdate, Plugin, SubApp};
use bevy_ecs::prelude::{Commands, Entity, IntoScheduleConfigs, Populated, With, World};
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::{PlayerBody, PoseSystems};
use mcrs_minecraft_engine_item::{ItemPlugin, ItemSet};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_inventory_server::InventoryServerPlugin;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;
use mcrs_minecraft_vanilla_item_entity::{Collect, ItemCollector, ItemEntityPlugin, ItemEntitySet};

use crate::entity_motion::add_entity_motion;

pub struct VanillaItemEntityPlugin;

impl Plugin for VanillaItemEntityPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(build_dimension_items);
    }
}

fn build_dimension_items(host: &World, _: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    let Some(table) = host.get_resource::<ItemTable>().copied() else {
        tracing::warn!("the host has no item table; the dimension world has no item entities");
        return;
    };
    add_entity_motion(sub_app);
    sub_app.insert_resource(table);
    sub_app.add_plugins((
        ItemPlugin::default(),
        ItemEntityPlugin,
        VanillaInventoryPlugin,
        InventoryServerPlugin,
    ));
    sub_app
        .configure_sets(
            FixedUpdate,
            ItemSet.in_set(TickSet::Simulate).after(ItemEntitySet),
        )
        .add_systems(
            FixedUpdate,
            track_collectors
                .in_set(TickSet::Simulate)
                .after(PoseSystems::Select)
                .before(ItemEntitySet),
        );
}

#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that a collector needs"
)]
fn track_collectors(
    mut commands: Commands,
    mut players: Populated<
        (
            Entity,
            &Owner,
            &Transform,
            PlayerBody,
            Option<&PlayerGameMode>,
            Option<&GameProfile>,
            Option<&mut ItemCollector>,
        ),
        With<Player>,
    >,
) {
    for (player, owner, transform, body, game_mode, profile, collector) in &mut players {
        let mode = match game_mode.map_or(GameMode::Survival, |mode| mode.0) {
            GameMode::Survival | GameMode::Adventure => Collect::Survival,
            GameMode::Creative => Collect::Creative,
            GameMode::Spectator => {
                if collector.is_some() {
                    commands.entity(player).remove::<ItemCollector>();
                }
                continue;
            }
        };
        let dimensions = body.dimensions();
        let next = ItemCollector {
            id: EntityId(u64::from(player_network_id(owner.0) as u32)),
            uuid: profile.map_or(0, |profile| profile.id.as_u128()),
            bounds: Aabb::of_body(transform.translation, dimensions.width, dimensions.height),
            vehicle: None,
            mode,
        };
        match collector {
            Some(mut collector) => {
                if *collector != next {
                    *collector = next;
                }
            }
            None => {
                commands.entity(player).insert(next);
            }
        }
    }
}
