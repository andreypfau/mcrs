use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::ApplyDeferred;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::EntityMotionPlugin;
use crate::MobPublish;
use crate::crowd_hits::has_crowds;
use crate::crowd_hits::hit_crowds;
use crate::despawn_expired::despawn_expired;
use crate::despawn_expired::has_expired;
use crate::entity_writes::EntityWrites;
use crate::entity_writes::apply_entity_writes;
use crate::entity_writes::has_writes;
use crate::mob_changes::MobChanges;
use crate::mob_changes::clear_mob_changes;
use crate::mob_index::MobIndex;
use crate::mob_index::forget_mob;
use crate::mob_spawn::WaitingSpawns;
use crate::mob_spawn::spawn_mob_entities;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::row_physics_plugin::CrowdDamageType;
use crate::row_physics_plugin::build_tables;
use crate::row_physics_plugin::find_crowd_damage_type;

#[derive(Clone, Copy, Debug, Default)]
pub struct EntityPhysicsPlugin;

impl Plugin for EntityPhysicsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EntityMotionPlugin>() {
            app.add_plugins(EntityMotionPlugin);
        }
        app.init_resource::<BirthClock>();
        app.init_resource::<CellEntityIds>();
        app.init_resource::<MobIndex>();
        app.init_resource::<MobChanges>();
        app.init_resource::<EntityWrites>();
        app.init_resource::<CrowdDamageType>();
        app.init_resource::<WaitingSpawns>();
        app.add_message::<MobSpawnRequest>();
        app.add_observer(forget_mob);
        app.add_systems(
            FixedUpdate,
            (
                build_tables.run_if(
                    resource_exists::<Blocks>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                ),
                find_crowd_damage_type.run_if(resource_exists_and_changed::<CurrentRegistries>),
                apply_entity_writes.run_if(has_writes),
                spawn_mob_entities,
                ApplyDeferred,
            )
                .chain()
                .in_set(TickSet::DomainInput),
        );
        app.add_systems(
            FixedUpdate,
            (
                hit_crowds.run_if(has_crowds),
                despawn_expired.run_if(has_expired),
            )
                .chain()
                .in_set(TickSet::Simulate)
                .after(MotionPass),
        );
        app.configure_sets(
            FixedUpdate,
            MobPublish
                .in_set(TickSet::Project)
                .before(clear_mob_changes),
        );
        app.add_systems(FixedUpdate, clear_mob_changes.in_set(TickSet::Project));
    }
}
