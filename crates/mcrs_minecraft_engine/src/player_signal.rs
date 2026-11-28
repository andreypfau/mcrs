use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader};
use bevy_ecs::system::{Query, ResMut};
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_replication::{MobSignal, PlayerData};

use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlayerSignal {
    pub player: Entity,
    pub signal: MobSignal,
}

impl PlayerSignal {
    #[must_use]
    pub const fn new(player: Entity, signal: MobSignal) -> Self {
        Self { player, signal }
    }
}

pub(crate) fn publish_player_signals(
    mut signals: MessageReader<PlayerSignal>,
    players: Query<&Replicated>,
    mut events: ResMut<TileEvents>,
) {
    for signal in signals.read() {
        if let Ok(replicated) = players.get(signal.player) {
            events.push(
                replicated.tile,
                replicated.id,
                EntityEvent::Data(PlayerData::Signal(signal.signal).into()),
            );
        }
    }
}
