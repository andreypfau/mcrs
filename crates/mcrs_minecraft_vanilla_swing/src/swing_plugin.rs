use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::loaded_client::UnloadedClient;
use mcrs_minecraft_engine::swing::{SwingPlayer, SwingSimulateSet};
use mcrs_minecraft_engine_item::{Selected, Slots};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_item_stack::{AttackAnimation, PiercingWeapon, SwingAnimation, SwingKind};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{ServerboundPlayerAction, ServerboundPunch};
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwingPlugin;

impl Plugin for SwingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PendingSwingInput>()
            .add_message::<SwingPlayer>()
            .add_observer(on_punch)
            .add_observer(on_player_action)
            .add_systems(
                FixedUpdate,
                process_swing_inputs
                    .run_if(on_message::<PendingSwingInput>)
                    .in_set(SwingSimulateSet),
            );
    }
}

#[derive(Message, Clone, Copy, Debug)]
struct PendingSwingInput {
    player: Entity,
    kind: SwingInputKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SwingInputKind {
    Punch,
    Stab,
}

fn on_punch(
    received: On<Received<ServerboundPunch>>,
    mut pending: MessageWriter<PendingSwingInput>,
) {
    if received.packet().is_none() {
        return;
    }
    pending.write(PendingSwingInput {
        player: received.entity,
        kind: SwingInputKind::Punch,
    });
}

fn on_player_action(
    received: On<Received<ServerboundPlayerAction>>,
    mut pending: MessageWriter<PendingSwingInput>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    if packet.action == PlayerAction::Stab {
        pending.write(PendingSwingInput {
            player: received.entity,
            kind: SwingInputKind::Stab,
        });
    }
}

fn protocol_kind(kind: SwingKind) -> SwingAnimationKind {
    match kind {
        SwingKind::None => SwingAnimationKind::None,
        SwingKind::Whack => SwingAnimationKind::Whack,
        SwingKind::Stab => SwingAnimationKind::Stab,
    }
}

fn swing_for_animation(player: Entity, animation: Option<SwingAnimation>) -> SwingPlayer {
    match animation {
        Some(animation) => SwingPlayer::new(
            player,
            Hand::Main,
            protocol_kind(animation.kind),
            animation.duration.max(0),
        ),
        None => SwingPlayer::default_main(player),
    }
}

fn process_swing_inputs(
    mut inputs: MessageReader<PendingSwingInput>,
    players: Query<(Option<&UnloadedClient>, Option<&Slots>, Option<&Selected>), With<Owner>>,
    stacks: Query<(Option<&AttackAnimation>, Option<&PiercingWeapon>)>,
    mut swings: MessageWriter<SwingPlayer>,
) {
    for input in inputs.read() {
        let Ok((unloaded, slots, selected)) = players.get(input.player) else {
            continue;
        };
        if matches!(input.kind, SwingInputKind::Punch) && unloaded.is_some() {
            continue;
        }
        let main_stack =
            slots.and_then(|slots| slots.get(usize::from(selected.map_or(0, |slot| slot.0))));
        let animation = match input.kind {
            SwingInputKind::Punch => main_stack
                .and_then(|stack| stacks.get(stack).ok())
                .and_then(|(attack, _)| attack.map(|animation| animation.0)),
            SwingInputKind::Stab => {
                let Some(stack) = main_stack else {
                    continue;
                };
                let Ok((attack, piercing)) = stacks.get(stack) else {
                    continue;
                };
                if piercing.is_none() {
                    continue;
                }
                attack.map(|animation| animation.0)
            }
        };
        swings.write(swing_for_animation(input.player, animation));
    }
}
