use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::DamageSet;
use crate::Damaged;
use crate::Died;
use crate::PendingHits;
use crate::RespawnRequest;
use crate::apply_hits::apply_hits;
use crate::bake_damage_types::bake_damage_types;
use crate::pending_hits::has_pending_hits;
use crate::tick_vitals::tick_vitals;
use crate::vitals_work::VitalsWork;
use crate::vitals_work::has_vitals_work;
use crate::vitals_work::track_inserted_vitals;

#[derive(Clone, Copy, Debug, Default)]
pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingHits>();
        app.init_resource::<DamageTypeRules>();
        app.init_resource::<VitalsWork>();
        app.add_observer(track_inserted_vitals);
        app.add_message::<Damaged>();
        app.add_message::<Died>();
        app.add_message::<RespawnRequest>();
        app.configure_sets(
            FixedUpdate,
            (
                DamageSet::PacketCollect,
                DamageSet::PacketFilter,
                DamageSet::PacketApply,
                DamageSet::PacketReport,
            )
                .chain()
                .after(TickSet::ClientInput)
                .before(TickSet::Simulate),
        );
        app.configure_sets(
            FixedUpdate,
            (
                DamageSet::Collect,
                DamageSet::Filter,
                DamageSet::Apply,
                DamageSet::Report,
            )
                .chain()
                .in_set(TickSet::WorldWrite)
                .after(MotionPass),
        );
        app.add_systems(
            FixedUpdate,
            (
                bake_damage_types
                    .run_if(resource_exists_and_changed::<CurrentRegistries>)
                    .in_set(TickSet::DomainInput),
                tick_vitals
                    .run_if(has_vitals_work)
                    .in_set(TickSet::Simulate)
                    .before(MotionPass),
                apply_hits
                    .run_if(has_pending_hits)
                    .in_set(DamageSet::PacketApply),
                apply_hits.run_if(has_pending_hits).in_set(DamageSet::Apply),
            ),
        );
    }
}
