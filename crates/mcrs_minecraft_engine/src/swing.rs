use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_physics::CORPSE_REMOVAL_TICKS;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
use mcrs_minecraft_replication::{PlayerData, SwingRecipients};
use mcrs_minecraft_swing::{SwingModifiers, SwingTracker};

use crate::loaded_client::LoadedClientTickSet;
use crate::player_state::PlayerFileState;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

const DEFAULT_DURATION: i32 = 6;

pub struct PlayerSwingPlugin;

impl Plugin for PlayerSwingPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.try_register_required_components::<Player, PlayerSwing>() {
            tracing::error!("a player does not require its swing state: {error}");
        }
        app.add_message::<SwingPlayer>();
        app.configure_sets(
            FixedUpdate,
            (
                (TickSet::ClientInput, TickSet::Simulate, TickSet::Project).chain(),
                SwingSimulateSet
                    .in_set(TickSet::Simulate)
                    .after(LoadedClientTickSet),
            ),
        );
        app.add_systems(
            FixedUpdate,
            apply_player_swings
                .run_if(on_message::<SwingPlayer>)
                .in_set(TickSet::Project),
        );
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwingSimulateSet;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerSwing(pub SwingTracker);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwingEligibility {
    #[default]
    AliveActor,
    CommandTarget,
}

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SwingPlayer {
    pub player: Entity,
    pub hand: Hand,
    pub animation: SwingAnimationKind,
    pub duration: i32,
    pub recipients: SwingRecipients,
    pub eligibility: SwingEligibility,
}

impl SwingPlayer {
    #[must_use]
    pub const fn new(
        player: Entity,
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
    ) -> Self {
        Self {
            player,
            hand,
            animation,
            duration,
            recipients: SwingRecipients::Observers,
            eligibility: SwingEligibility::AliveActor,
        }
    }

    #[must_use]
    pub const fn default_main(player: Entity) -> Self {
        Self::new(
            player,
            Hand::Main,
            SwingAnimationKind::Whack,
            DEFAULT_DURATION,
        )
    }

    #[must_use]
    pub const fn command(
        player: Entity,
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
    ) -> Self {
        Self {
            player,
            hand,
            animation,
            duration,
            recipients: SwingRecipients::SelfAndObservers,
            eligibility: SwingEligibility::CommandTarget,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct PendingPlayerSwings {
    swings: Vec<SwingPlayer>,
}

pub(crate) fn has_pending_player_swings(pending: Res<PendingPlayerSwings>) -> bool {
    !pending.swings.is_empty()
}

fn player_can_swing(
    eligibility: SwingEligibility,
    mode: Option<&PlayerGameMode>,
    vitals: Option<&Vitals>,
) -> bool {
    match eligibility {
        SwingEligibility::AliveActor => {
            let spectator = mode.is_some_and(|mode| mode.0 == GameMode::Spectator);
            let dead = vitals.is_some_and(|vitals| !vitals.is_alive());
            !spectator && !dead
        }
        SwingEligibility::CommandTarget => {
            let corpse_removed = vitals
                .and_then(|vitals| vitals.death_time)
                .is_some_and(|death_time| death_time >= CORPSE_REMOVAL_TICKS);
            !corpse_removed
        }
    }
}

fn swing_modifiers_for_player(state: Option<&PlayerFileState>) -> SwingModifiers {
    let Some(state) = state else {
        return SwingModifiers::default();
    };
    SwingModifiers::from_effects(
        state
            .active_effects
            .iter()
            .filter(|effect| effect.duration == -1 || effect.duration > 0)
            .map(|effect| (effect.id.as_str(), effect.amplifier)),
    )
}

fn apply_player_swings(
    mut requests: MessageReader<SwingPlayer>,
    clock: Option<Res<BirthClock>>,
    mut players: Query<(
        Option<&PlayerGameMode>,
        Option<&PlayerFileState>,
        Option<&Vitals>,
        &mut PlayerSwing,
    )>,
    mut pending: Option<ResMut<PendingPlayerSwings>>,
) {
    let now = clock.map_or(0, |clock| clock.0);
    for &request in requests.read() {
        let Ok((mode, file_state, vitals, mut swing)) = players.get_mut(request.player) else {
            continue;
        };
        if !player_can_swing(request.eligibility, mode, vitals) {
            continue;
        }
        if !swing.0.can_start(now) {
            continue;
        }
        let modifiers = swing_modifiers_for_player(file_state);
        let base_duration = request.duration.max(0);
        if swing.0.try_start(now, base_duration, modifiers).is_none() {
            continue;
        }
        if let Some(pending) = pending.as_deref_mut() {
            pending.swings.push(SwingPlayer {
                duration: base_duration,
                ..request
            });
        }
    }
}

pub(crate) fn publish_player_swings(
    mut pending: ResMut<PendingPlayerSwings>,
    players: Query<&Replicated>,
    mut events: ResMut<TileEvents>,
) {
    for swing in pending.swings.drain(..) {
        if let Ok(replicated) = players.get(swing.player) {
            events.push(
                replicated.tile,
                replicated.id,
                EntityEvent::Data(
                    PlayerData::Swing {
                        hand: swing.hand,
                        animation: swing.animation,
                        duration: swing.duration,
                        recipients: swing.recipients,
                    }
                    .into(),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::change_detection::DetectChangesMut;
    use bevy_time::{TimePlugin, TimeUpdateStrategy};
    use mcrs_engine_muxer::{EntityId, TileKey};
    use mcrs_minecraft_replication::ReplicationData;

    use crate::bus::InboundPlayerDespawn;
    use crate::dimension_change::DimensionChangeError;
    use crate::player_order::{self, PlayerOrder, SwingOrder};
    use crate::player_state::Effect;
    use crate::player_tracker::PlayerTrackerPlugin;
    use crate::replicated::Visibility;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct EmittedSwing {
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
        recipients: SwingRecipients,
    }

    fn test_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((TimePlugin, PlayerSwingPlugin, PlayerTrackerPlugin))
            .add_message::<InboundPlayerDespawn>()
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        app.update();
        let world = app.world_mut();
        world.insert_resource(BirthClock(10));
        let player = world
            .spawn((
                Player,
                PlayerGameMode(GameMode::Survival),
                PlayerFileState::default(),
                Replicated {
                    id: EntityId(99),
                    tile: TileKey::new(0, 0, 0),
                    game_mode: GameMode::Survival,
                    visibility: Visibility::Shown,
                },
            ))
            .id();
        (app, player)
    }

    fn run_and_take_swings(app: &mut App) -> Vec<EmittedSwing> {
        app.update();
        let deltas = app.world_mut().resource_mut::<TileEvents>().take();
        deltas
            .tiles
            .iter()
            .flat_map(|delta| delta.events.iter())
            .filter_map(|event| match &event.event {
                EntityEvent::Data(ReplicationData::Player(PlayerData::Swing {
                    hand,
                    animation,
                    duration,
                    recipients,
                })) => Some(EmittedSwing {
                    hand: *hand,
                    animation: *animation,
                    duration: *duration,
                    recipients: *recipients,
                }),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn apply_player_swings_dedups_same_tick_and_scales_cooldown_with_unscaled_wire() {
        let (mut app, player) = test_app();
        app.world_mut().write_message(SwingPlayer::new(
            player,
            Hand::Off,
            SwingAnimationKind::Stab,
            10,
        ));
        app.world_mut()
            .write_message(SwingPlayer::default_main(player));
        assert_eq!(
            run_and_take_swings(&mut app),
            vec![EmittedSwing {
                hand: Hand::Off,
                animation: SwingAnimationKind::Stab,
                duration: 10,
                recipients: SwingRecipients::Observers,
            }],
            "SWG-STA-004: at most one swing per player per tick"
        );

        app.world_mut()
            .get_mut::<PlayerFileState>(player)
            .expect("player state")
            .active_effects
            .push(Effect {
                id: "minecraft:mining_fatigue".to_owned(),
                amplifier: 1,
                duration: 40,
                ambient: false,
                show_particles: true,
                show_icon: Some(true),
                hidden_effect: None,
            });
        app.world_mut().insert_resource(BirthClock(15));
        app.world_mut()
            .write_message(SwingPlayer::default_main(player));
        assert!(
            run_and_take_swings(&mut app).is_empty(),
            "SWG-STA-005: D=10 blocks tick T+5"
        );

        app.world_mut().insert_resource(BirthClock(16));
        app.world_mut()
            .write_message(SwingPlayer::default_main(player));
        assert_eq!(
            run_and_take_swings(&mut app),
            vec![EmittedSwing {
                hand: Hand::Main,
                animation: SwingAnimationKind::Whack,
                duration: 6,
                recipients: SwingRecipients::Observers,
            }],
            "SWG-CMP-003: wire duration remains unscaled under mining fatigue"
        );

        app.world_mut().insert_resource(BirthClock(21));
        app.world_mut()
            .write_message(SwingPlayer::default_main(player));
        assert!(
            run_and_take_swings(&mut app).is_empty(),
            "SWG-STA-002/005: mining fatigue 1 expands B=6 to D=10 blocking T+5"
        );

        app.world_mut().insert_resource(BirthClock(22));
        app.world_mut()
            .write_message(SwingPlayer::default_main(player));
        assert_eq!(
            run_and_take_swings(&mut app),
            vec![EmittedSwing {
                hand: Hand::Main,
                animation: SwingAnimationKind::Whack,
                duration: 6,
                recipients: SwingRecipients::Observers,
            }],
            "SWG-STA-002/005: D=10 unlocks at T+6"
        );
    }

    #[test]
    fn player_can_swing_enforces_action_and_command_eligibility_matrix() {
        let vitals = |health, death_time| Vitals {
            death_time,
            ..Vitals::new(health)
        };
        let cases = [
            (SwingEligibility::AliveActor, None, None, true),
            (
                SwingEligibility::AliveActor,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(20.0, None)),
                true,
            ),
            (
                SwingEligibility::AliveActor,
                Some(PlayerGameMode(GameMode::Spectator)),
                Some(vitals(20.0, None)),
                false,
            ),
            (
                SwingEligibility::AliveActor,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(0.0, Some(0))),
                false,
            ),
            (
                SwingEligibility::AliveActor,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(0.0, Some(1))),
                false,
            ),
            (SwingEligibility::CommandTarget, None, None, true),
            (
                SwingEligibility::CommandTarget,
                Some(PlayerGameMode(GameMode::Spectator)),
                Some(vitals(20.0, None)),
                true,
            ),
            (
                SwingEligibility::CommandTarget,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(0.0, Some(0))),
                true,
            ),
            (
                SwingEligibility::CommandTarget,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(0.0, Some(19))),
                true,
            ),
            (
                SwingEligibility::CommandTarget,
                Some(PlayerGameMode(GameMode::Survival)),
                Some(vitals(0.0, Some(20))),
                false,
            ),
        ];
        for (eligibility, mode, vitals, expected) in cases {
            assert_eq!(
                player_can_swing(eligibility, mode.as_ref(), vitals.as_ref()),
                expected,
                "eligibility={eligibility:?} mode={mode:?} death_time={:?} health={:?}",
                vitals.as_ref().and_then(|v| v.death_time),
                vitals.as_ref().map(|v| v.health),
            );
        }
    }

    #[test]
    fn player_order_swing_wires_into_player_swing_pipeline_and_shares_cooldown() {
        let (mut app, player) = test_app();
        let non_player = app.world_mut().spawn_empty().id();
        assert_eq!(
            player_order::swing(
                app.world_mut(),
                non_player,
                SwingOrder::new(Hand::Main, SwingAnimationKind::Whack, 6),
            ),
            Err(DimensionChangeError::NoPlayer),
            "player_order::swing rejects non-player entities"
        );

        app.world_mut()
            .get_mut::<PlayerGameMode>(player)
            .expect("player mode")
            .set_if_neq(PlayerGameMode(GameMode::Spectator));
        player_order::obey(
            app.world_mut(),
            player,
            PlayerOrder::Swing(SwingOrder::new(Hand::Off, SwingAnimationKind::Stab, 8)),
        )
        .expect("swing order applies");
        assert_eq!(
            run_and_take_swings(&mut app),
            vec![EmittedSwing {
                hand: Hand::Off,
                animation: SwingAnimationKind::Stab,
                duration: 8,
                recipients: SwingRecipients::SelfAndObservers,
            }],
            "CMD-SWG-006: PlayerOrder::Swing publishes self-included swing for spectators"
        );

        app.world_mut().insert_resource(BirthClock(14));
        app.world_mut().write_message(SwingPlayer::command(
            player,
            Hand::Main,
            SwingAnimationKind::Whack,
            6,
        ));
        assert!(
            run_and_take_swings(&mut app).is_empty(),
            "CMD-SWG-006: D=8 blocks T+4 across swing sources"
        );
    }
}
