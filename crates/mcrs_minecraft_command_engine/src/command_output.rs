use crate::command_outbox::CommandOutbox;
use crate::command_targets::CommandTargets;
use crate::effect_queue::{EffectQueue, apply_next};
use crate::invocation::Invocation;
use crate::operator_broadcast::OperatorBroadcast;
use crate::source_state::SourceState;
use crate::system_message::system_message;
use bevy_ecs::prelude::{Entity, Res, ResMut};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};

/// Who sees the success message of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// Only the player who sent the line.
    Source,
    /// The player who sent the line, and every other operator through an
    /// [`OperatorBroadcast`].
    SourceAndOperators,
}

/// What a handler sends and changes. Its messages go out after its effects apply, so a player
/// that a command moves hears of the move after the move (CMD-TP-014).
#[derive(SystemParam)]
pub struct CommandOutput<'w> {
    outbox: ResMut<'w, CommandOutbox>,
    targets: Res<'w, CommandTargets>,
}

impl CommandOutput<'_> {
    /// A packet the command sends besides its messages.
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.outbox.packets.push(packet);
    }

    /// A system message to the player with the host anchor `player`, in any dimension.
    pub fn tell(&mut self, player: Entity, message: Text) {
        self.send(system_message(player, message));
    }

    /// A success message, to the players that `feedback` names.
    pub fn success(&mut self, invocation: &Invocation, message: Text, feedback: Feedback) {
        // chisle: `send_command_feedback` and `log_admin_commands` have no state in the server
        // yet, so both hold their default true; a game rule resource lifts it.
        if feedback == Feedback::Source {
            self.send(system_message(invocation.output, message));
            return;
        }
        self.send(system_message(invocation.output, message.clone()));
        let Some(source) = invocation
            .source
            .entity
            .and_then(|anchor| self.targets.player(anchor))
        else {
            return;
        };
        // The server has no language table, so the log keeps the translation key and its
        // arguments as JSON.
        tracing::info!("[{}: {message}]", source.name);
        let admin = Text::translate(
            "chat.type.admin",
            vec![source.name.clone().into_text(), message],
        )
        .color(Color::GRAY)
        .italic();
        self.outbox.broadcasts.push(OperatorBroadcast {
            source: invocation.output,
            message: admin,
        });
    }

    /// A source a modifier gives, after the ones it gave before in this run. The modifier then
    /// returns [`Modified::Forked`](crate::Modified::Forked).
    pub fn fork(&mut self, source: SourceState) {
        self.outbox.forked.push(source);
    }

    /// An effect for the host to apply after this run, in the order of writing across all
    /// effect types, before the messages of the run go out.
    pub fn effect<E: Send + Sync + 'static>(&mut self, queue: &mut EffectQueue<E>, effect: E) {
        queue.values.push_back(effect);
        self.outbox.effects.push(apply_next::<E>);
    }
}
