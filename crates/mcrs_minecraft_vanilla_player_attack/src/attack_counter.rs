use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::observer::On;
use bevy_ecs::query::Has;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::DeferredWorld;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::loaded_client::UnloadedClient;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPunch;

use crate::birth_tick::birth_tick;
use crate::ignores_attacks::ignores_attacks;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(on_add = start_counting)]
pub struct AttackCounter {
    pub reset_at: u64,
}

impl AttackCounter {
    #[must_use]
    pub fn ticks(self, now: u64) -> u32 {
        u32::try_from(now.saturating_sub(self.reset_at)).unwrap_or(u32::MAX)
    }
}

fn start_counting(mut world: DeferredWorld, context: HookContext) {
    let now = birth_tick(world.get_resource::<BirthClock>());
    if let Some(mut counter) = world.get_mut::<AttackCounter>(context.entity) {
        counter.reset_at = now;
    }
}

pub(crate) fn reset_on_punch(
    received: On<Received<ServerboundPunch>>,
    clock: Option<Res<BirthClock>>,
    mut players: Query<(
        &mut AttackCounter,
        &PlayerGameMode,
        Option<&Vitals>,
        Has<UnloadedClient>,
    )>,
) {
    if received.packet().is_none() {
        return;
    }
    let Ok((mut counter, mode, vitals, unloaded)) = players.get_mut(received.entity) else {
        return;
    };
    if ignores_attacks(unloaded, mode, vitals) {
        return;
    }
    counter.set_if_neq(AttackCounter {
        reset_at: birth_tick(clock.as_deref()),
    });
}
