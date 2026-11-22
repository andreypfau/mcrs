use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::Vitals;

use crate::vitals_work::VitalsWork;

pub(crate) fn tick_vitals(mut work: ResMut<VitalsWork>, mut vitals: Query<&mut Vitals>) {
    work.0.retain(|&entity| {
        let Ok(mut vitals) = vitals.get_mut(entity) else {
            return false;
        };
        let next = counted_down(*vitals);
        vitals.set_if_neq(next);
        next.is_counting()
    });
}

fn counted_down(mut vitals: Vitals) -> Vitals {
    vitals.hurt_time = vitals.hurt_time.saturating_sub(1);
    vitals.damage_cooldown = vitals.damage_cooldown.saturating_sub(1);
    vitals.death_time = vitals.death_time.map(|ticks| ticks.saturating_add(1));
    vitals
}
