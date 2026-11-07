use std::ops::Range;

use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

/// The entities near a player that can push it on its client, with the positions the server
/// sent. The host implements it over the positions that it keeps per entity.
pub trait PusherHistory {
    /// Appends each living entity and boat that the client has, that the team rules can let push
    /// the player, and whose position hull over `server_ticks` can touch `near`. The hull also
    /// holds the position before the oldest of these ticks, because the client interpolates
    /// from it.
    fn pushers(&self, near: Aabb, server_ticks: Range<u32>, out: &mut Vec<Pusher>);
}

/// One possible pusher: the box of its positions and the size of its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pusher {
    pub low: DVec3,
    pub high: DVec3,
    /// Half the width of the box; a boat adds its push margin.
    pub half_width: f64,
    pub height: f64,
    /// False when the history no longer has a position that the hull needs.
    pub complete: bool,
}

impl Pusher {
    /// The smallest box that holds the box of the entity at each of its positions.
    pub(crate) fn hull(&self) -> Aabb {
        let margin = DVec3::new(self.half_width, 0.0, self.half_width);
        Aabb::new(
            self.low - margin,
            self.high + margin + DVec3::new(0.0, self.height, 0.0),
        )
    }
}

impl PusherHistory for [Pusher] {
    fn pushers(&self, near: Aabb, _server_ticks: Range<u32>, out: &mut Vec<Pusher>) {
        out.extend(self.iter().filter(|pusher| pusher.hull().intersects(&near)));
    }
}
