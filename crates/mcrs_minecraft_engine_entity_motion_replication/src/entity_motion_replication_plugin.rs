use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_engine::OwnAttributesSet;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_entity_motion::MotionRelease;
use mcrs_minecraft_engine_mob_attributes::AttributeChanged;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::assign_ids::Assigned;
use crate::assign_ids::assign_ids;
use crate::departures::Departures;
use crate::departures::depart;
use crate::hold_unseen::hold_unseen;
use crate::mob_news::MobNews;
use crate::mob_news::has_mob_spawns;
use crate::mob_news::take_mob_news;
use crate::publish_mob_attributes::publish_mob_attributes;
use crate::publish_motion::has_motion_work;
use crate::publish_motion::publish_motion;

#[derive(Clone, Copy, Debug, Default)]
pub struct EntityMotionReplicationPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPublish;

impl Plugin for EntityMotionReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assigned>();
        app.init_resource::<Departures>();
        app.init_resource::<MobNews>();
        app.init_resource::<MobChanges>();
        app.init_resource::<TileEvents>();
        app.add_message::<AttributeChanged>();
        app.add_message::<Damaged>();
        app.add_observer(hold_unseen);
        app.add_observer(depart);
        app.configure_sets(
            FixedUpdate,
            MotionPublish.in_set(TickSet::Project).after(MobPublish),
        );
        app.configure_sets(FixedUpdate, OwnAttributesSet.before(MobPublish));
        app.add_systems(
            FixedUpdate,
            assign_ids
                .run_if(resource_exists::<CellEntityIds>)
                .in_set(MotionRelease),
        );
        app.add_systems(
            FixedUpdate,
            take_mob_news
                .run_if(has_mob_spawns.or_else(on_message::<Damaged>))
                .in_set(MobPublish),
        );
        app.add_systems(
            FixedUpdate,
            (
                publish_motion.run_if(has_motion_work),
                publish_mob_attributes.run_if(on_message::<AttributeChanged>),
            )
                .chain()
                .in_set(MotionPublish),
        );
    }
}
