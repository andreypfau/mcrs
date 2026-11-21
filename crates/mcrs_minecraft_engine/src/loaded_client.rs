use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Commands, Query};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerLoaded;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoadedClientTickSet;

pub struct LoadedClientPlugin;

impl Plugin for LoadedClientPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.try_register_required_components::<Player, UnloadedClient>() {
            tracing::error!("a player does not require its loaded client state: {error}");
        }
        app.add_observer(on_player_loaded);
        app.add_systems(
            FixedUpdate,
            tick_loaded_clients
                .run_if(any_with_component::<UnloadedClient>)
                .in_set(LoadedClientTickSet)
                .in_set(TickSet::Simulate),
        );
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct UnloadedClient(u8);

impl UnloadedClient {
    pub const FALLBACK_TICKS: u8 = 60;

    pub fn tick(&mut self) -> bool {
        self.0 = self.0.saturating_add(1);
        self.0 >= Self::FALLBACK_TICKS
    }
}

fn on_player_loaded(
    received: On<Received<ServerboundPlayerLoaded>>,
    players: Query<(), (With<Owner>, With<UnloadedClient>)>,
    mut commands: Commands,
) {
    if received.packet().is_none() {
        return;
    }
    if players.contains(received.entity) {
        commands.entity(received.entity).remove::<UnloadedClient>();
    }
}

fn tick_loaded_clients(
    mut players: Query<(Entity, &mut UnloadedClient), With<Owner>>,
    mut commands: Commands,
) {
    for (entity, mut unloaded) in &mut players {
        if unloaded.tick() {
            commands.entity(entity).remove::<UnloadedClient>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_time::{TimePlugin, TimeUpdateStrategy};
    use bytes::Bytes;
    use mcrs_minecraft_engine_world::session::PlayerSession;

    #[test]
    fn loaded_client_loads_on_packet_or_60_tick_fallback_and_rejects_malformed_packet() {
        let mut app = App::new();
        app.add_plugins((TimePlugin, LoadedClientPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        app.update();
        let by_ticks = app
            .world_mut()
            .spawn((Player, Owner(PlayerSession(1))))
            .id();
        let by_packet = app
            .world_mut()
            .spawn((Player, Owner(PlayerSession(2))))
            .id();
        assert!(
            app.world().get::<UnloadedClient>(by_ticks).is_some(),
            "a freshly spawned player waits to load"
        );

        app.world_mut()
            .trigger(Received::<ServerboundPlayerLoaded>::new(
                by_packet,
                Bytes::from_static(b"extra"),
            ));
        app.world_mut().flush();
        assert!(
            app.world().get::<UnloadedClient>(by_packet).is_some(),
            "malformed ServerboundPlayerLoaded must not mark client loaded"
        );

        app.world_mut()
            .trigger(Received::<ServerboundPlayerLoaded>::new(
                by_packet,
                Bytes::new(),
            ));
        app.world_mut().flush();
        assert!(
            app.world().get::<UnloadedClient>(by_packet).is_none(),
            "valid ServerboundPlayerLoaded immediately removes UnloadedClient"
        );

        for _ in 0..59 {
            app.update();
            assert!(
                app.world().get::<UnloadedClient>(by_ticks).is_some(),
                "client stays unloaded before tick 60"
            );
        }
        app.update();
        assert!(
            app.world().get::<UnloadedClient>(by_ticks).is_none(),
            "client loads at tick 60 fallback"
        );
    }
}
