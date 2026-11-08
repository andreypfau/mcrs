use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

/// One possible pusher of a player on its client: the box of the positions that the server sent
/// and that the client can show in the tick or the one before, and the size of its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pusher {
    pub low: DVec3,
    pub high: DVec3,
    /// Half the width of the box; a boat adds its push margin.
    pub half_width: f64,
    pub height: f64,
    /// False when a position that the hull needs is missing.
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
