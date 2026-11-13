use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_engine_physics::EntityPass;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::death_drops::{DropInventoryOnDeathEvent, drop_dead_inventories, handle_death_drops};
use crate::effects::{
    ApplyEffectEvent, RemoveEffectEvent, handle_apply_effect_events, tick_active_effects,
};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, KnockbackRequest, RespawnRequest};
use crate::experience::ExperienceGainedEvent;
use crate::life_cycle::{
    apply_knockback, process_damage_requests, tick_death, tick_player_cooldowns,
};

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatSet {
    Pre,
    World,
    Outcome,
    Progress,
    Post,
    Send,
}

/// The Bevy plugin providing health, combat, status effects, experience, equipment and the
/// entity life cycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Register events / messages
        crate::runtime::build(app);
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<RespawnRequest>();
        app.add_message::<KnockbackRequest>();
        app.add_message::<ApplyEffectEvent>();
        app.add_message::<RemoveEffectEvent>();
        app.add_message::<DropInventoryOnDeathEvent>();
        app.add_message::<ExperienceGainedEvent>();

        app.configure_sets(
            FixedUpdate,
            (
                CombatSet::Pre,
                CombatSet::World,
                CombatSet::Outcome,
                CombatSet::Post,
                CombatSet::Progress,
                CombatSet::Send,
            )
                .chain()
                .after(EntityPass)
                .in_set(TickSet::Simulate),
        );
        app.add_systems(
            FixedUpdate,
            (handle_apply_effect_events, tick_active_effects)
                .chain()
                .in_set(CombatSet::World),
        );
        app.add_systems(
            FixedUpdate,
            (
                process_damage_requests,
                apply_knockback,
                tick_player_cooldowns,
                handle_death_drops,
                drop_dead_inventories,
                tick_death,
            )
                .chain()
                .in_set(CombatSet::Outcome),
        );
    }
}
