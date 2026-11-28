use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::ApplyDeferred;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_damage::DamagePlugin;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_entity_motion::MotionEffects;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::EntityMotionPlugin;
use crate::MobPublish;
use crate::crowd_damage_type::CrowdDamageType;
use crate::crowd_damage_type::find_crowd_damage_type;
use crate::crowd_hits::has_crowds;
use crate::crowd_hits::hit_crowds;
use crate::despawn_expired::despawn_expired;
use crate::despawn_expired::has_expired;
use crate::entity_writes::EntityWrites;
use crate::entity_writes::apply_entity_writes;
use crate::entity_writes::has_writes;
use crate::mob_changes::MobChanges;
use crate::mob_changes::clear_mob_changes;
use crate::mob_changes::has_mob_changes;
use crate::mob_index::MobIndex;
use crate::mob_index::forget_mob;
use crate::mob_spawn::WaitingSpawns;
use crate::mob_spawn::has_waiting_spawns;
use crate::mob_spawn::spawn_mob_entities;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::physics_tables::build_tables;

#[derive(Clone, Copy, Debug, Default)]
pub struct EntityPhysicsPlugin;

impl Plugin for EntityPhysicsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EntityMotionPlugin>() {
            app.add_plugins(EntityMotionPlugin);
        }
        if !app.is_plugin_added::<DamagePlugin>() {
            app.add_plugins(DamagePlugin);
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
                spawn_mob_entities
                    .run_if(on_message::<MobSpawnRequest>.or_else(has_waiting_spawns)),
                ApplyDeferred,
            )
                .chain()
                .in_set(TickSet::DomainInput),
        );
        app.add_systems(
            FixedUpdate,
            (
                despawn_expired
                    .run_if(has_expired)
                    .in_set(MotionPass)
                    .after(MotionEffects),
                hit_crowds.run_if(has_crowds).in_set(DamageSet::Collect),
                apply_entity_writes
                    .run_if(has_writes)
                    .in_set(TickSet::Simulate)
                    .before(MotionPass),
                apply_entity_writes
                    .run_if(has_writes)
                    .in_set(TickSet::WorldWrite)
                    .after(DamageSet::Report),
            ),
        );
        app.configure_sets(
            FixedUpdate,
            MobPublish
                .in_set(TickSet::Project)
                .before(clear_mob_changes),
        );
        app.add_systems(
            FixedUpdate,
            clear_mob_changes
                .run_if(has_mob_changes)
                .in_set(TickSet::Project),
        );
    }
}
