use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use bytes::Bytes;
use mcrs_minecraft_protocol_game::ColumnPos;
use rustc_hash::FxHashMap;

/// The players of a dimension whose `ColumnView` waits on each column, so that a column that
/// moves on reaches only the players it concerns, and the encoded packets of the cold columns
/// that views wait to send. A projection of the views: only `ColumnView::set` and the view's
/// discard write it.
#[derive(Component, Default)]
pub(crate) struct ColumnWaiters {
    players: FxHashMap<ColumnPos, Vec<Entity>>,
    /// The lists of columns no player waits on any more, kept so the next column reuses one.
    spare: Vec<Vec<Entity>>,
    packets: FxHashMap<ColumnPos, StoredPacket>,
    /// The loading tickets of the views on the columns they sent, to let go of.
    released_tickets: Vec<ColumnPos>,
}

/// A cold column encoded once from the data layer, and the count of views that wait to send it.
struct StoredPacket {
    packet: Bytes,
    views: u32,
}

impl ColumnWaiters {
    pub(crate) fn add(&mut self, column: ColumnPos, player: Entity) {
        let spare = &mut self.spare;
        self.players
            .entry(column)
            .or_insert_with(|| spare.pop().unwrap_or_default())
            .push(player);
    }

    pub(crate) fn remove(&mut self, column: ColumnPos, player: Entity) {
        let Some(players) = self.players.get_mut(&column) else {
            return;
        };
        players.retain(|waiting| *waiting != player);
        if players.is_empty()
            && let Some(empty) = self.players.remove(&column)
        {
            self.spare.push(empty);
        }
    }

    pub(crate) fn of(&self, column: ColumnPos) -> &[Entity] {
        self.players.get(&column).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn columns(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.players.keys().copied()
    }

    /// Keeps `packet` for the views of `column` that are about to wait to send it.
    pub(crate) fn insert_packet(&mut self, column: ColumnPos, packet: Bytes) {
        self.packets
            .entry(column)
            .and_modify(|held| held.packet = packet.clone())
            .or_insert(StoredPacket { packet, views: 0 });
    }

    pub(crate) fn packet(&self, column: ColumnPos) -> Option<&Bytes> {
        self.packets.get(&column).map(|held| &held.packet)
    }

    pub(crate) fn hold_packet(&mut self, column: ColumnPos) {
        if let Some(held) = self.packets.get_mut(&column) {
            held.views = held.views.saturating_add(1);
        }
    }

    pub(crate) fn release_ticket(&mut self, column: ColumnPos) {
        self.released_tickets.push(column);
    }

    pub(crate) fn take_released_tickets(&mut self) -> std::vec::Drain<'_, ColumnPos> {
        self.released_tickets.drain(..)
    }

    pub(crate) fn release_packet(&mut self, column: ColumnPos) {
        let Some(held) = self.packets.get_mut(&column) else {
            return;
        };
        held.views = held.views.saturating_sub(1);
        if held.views == 0 {
            self.packets.remove(&column);
        }
    }
}
