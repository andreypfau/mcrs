use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pusher {
    pub low: DVec3,
    pub high: DVec3,
    pub half_width: f64,
    pub height: f64,
    pub complete: bool,
}

impl Pusher {
    pub(crate) fn hull(&self) -> Aabb {
        let margin = DVec3::new(self.half_width, 0.0, self.half_width);
        Aabb::new(
            self.low - margin,
            self.high + margin + DVec3::new(0.0, self.height, 0.0),
        )
    }
}
