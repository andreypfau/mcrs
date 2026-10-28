use crate::command::{SourceState, Worlds};
use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_protocol::text::Text;
use std::collections::VecDeque;

/// A command reported success with a message for the operators (CMDP-EXEC-004). The host,
/// which knows every online player and the operators, delivers it to each operator but `source`.
#[derive(Event, Clone, Debug)]
pub struct OperatorBroadcast {
    /// The host anchor of the player that ran the command.
    pub source: Entity,
    pub message: Text,
}

/// Applies the oldest effect of one type that a handler wrote in the world at the index.
pub(crate) type ApplyNext = fn(&mut Worlds<'_, '_>, usize);

/// The effects of type `E` that a handler of this world wrote and the host did not apply yet,
/// in the order of writing, and the function that applies one. A handler writes one with
/// [`CommandOutput::effect`](crate::command::CommandOutput::effect).
#[derive(Resource)]
pub struct EffectQueue<E: Send + Sync + 'static> {
    pub(crate) values: VecDeque<E>,
    apply: fn(&mut Worlds<'_, '_>, E),
}

pub(crate) fn apply_next<E: Send + Sync + 'static>(worlds: &mut Worlds<'_, '_>, index: usize) {
    let next = worlds
        .world_mut(index)
        .and_then(|world| world.get_resource_mut::<EffectQueue<E>>())
        .and_then(|mut queue| Some((queue.apply, queue.values.pop_front()?)));
    match next {
        Some((apply, effect)) => apply(worlds, effect),
        None => tracing::error!("a command effect is not in the queue of its world"),
    }
}

/// Adds the queue of the effects of type `E` to each dimension world.
pub(crate) struct EffectPlugin<E> {
    pub(crate) apply: fn(&mut Worlds<'_, '_>, E),
}

impl<E> Clone for EffectPlugin<E> {
    fn clone(&self) -> Self {
        Self { apply: self.apply }
    }
}

impl<E: Send + Sync + 'static> Plugin for EffectPlugin<E> {
    fn build(&self, app: &mut App) {
        app.insert_resource(EffectQueue {
            values: VecDeque::new(),
            apply: self.apply,
        });
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// What one run of a handler leaves for the host: its effects in the order of writing, then its
/// messages. The host lends it to the world the handler runs in and empties it after the run.
#[derive(Resource, Debug, Default)]
pub(crate) struct CommandOutbox {
    pub(crate) effects: Vec<ApplyNext>,
    pub(crate) packets: Vec<OutboundPlayerPacket>,
    pub(crate) broadcasts: Vec<OperatorBroadcast>,
    pub(crate) forked: Vec<SourceState>,
}
