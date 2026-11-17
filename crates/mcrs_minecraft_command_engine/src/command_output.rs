use crate::command_outbox::CommandOutbox;
use crate::command_targets::CommandTargets;
use crate::effect_queue::{EffectQueue, apply_next};
use crate::invocation::Invocation;
use crate::operator_broadcast::OperatorBroadcast;
use crate::source_state::SourceState;
use crate::system_message::system_message;
use bevy_ecs::prelude::{Res, ResMut};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_text::{Color, IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    Source,
    SourceAndOperators,
}

#[derive(SystemParam)]
pub struct CommandOutput<'w> {
    outbox: ResMut<'w, CommandOutbox>,
    targets: Res<'w, CommandTargets>,
}

impl CommandOutput<'_> {
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.outbox.packets.push(packet);
    }

    pub fn tell(&mut self, player: PlayerSession, message: Text) {
        self.outbox.packets.extend(system_message(player, &message));
    }

    pub fn success(&mut self, invocation: &Invocation, message: Text, feedback: Feedback) {
        if feedback == Feedback::Source {
            self.outbox
                .packets
                .extend(system_message(invocation.output, &message));
            return;
        }
        self.outbox
            .packets
            .extend(system_message(invocation.output, &message));
        let Some(source) = invocation
            .source
            .player
            .and_then(|session| self.targets.player(session))
        else {
            return;
        };
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

    pub fn fork(&mut self, source: SourceState) {
        self.outbox.forked.push(source);
    }

    pub fn effect<E: Send + Sync + 'static>(&mut self, queue: &mut EffectQueue<E>, effect: E) {
        queue.values.push_back(effect);
        self.outbox.effects.push(apply_next::<E>);
    }
}
