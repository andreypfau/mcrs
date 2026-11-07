use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::MIN_PUSH_DISTANCE;
use mcrs_minecraft_physics::push_axis;

use crate::Limits;
use crate::Pusher;

/// The sum of the possible pushes on the player, per horizontal axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PushBound {
    pub(crate) min_x: f64,
    pub(crate) max_x: f64,
    pub(crate) min_z: f64,
    pub(crate) max_z: f64,
}

impl PushBound {
    /// The bound of the pushers on a player with the box `player` at `position`; `None` when it
    /// cannot hold them: too many, too wide, or a hull without the positions it needs.
    pub(crate) fn of(
        pushers: &[Pusher],
        player: &Aabb,
        position: DVec3,
        limits: &Limits,
    ) -> Option<Self> {
        let mut bound = Self::default();
        let mut count = 0_usize;
        for pusher in pushers {
            if !pusher.hull().intersects(player) {
                continue;
            }
            if !pusher.complete {
                return None;
            }
            let (min_dx, max_dx) = (position.x - pusher.high.x, position.x - pusher.low.x);
            let (min_dz, max_dz) = (position.z - pusher.high.z, position.z - pusher.low.z);
            let mut x = interval(min_dx, max_dx, min_dz, max_dz);
            let mut z = interval(min_dz, max_dz, min_dx, max_dx);
            if !certain(pusher, player, [min_dx, max_dx, min_dz, max_dz]) {
                x = Interval {
                    min: x.min.min(0.0),
                    max: x.max.max(0.0),
                };
                z = Interval {
                    min: z.min.min(0.0),
                    max: z.max.max(0.0),
                };
            }
            if x == Interval::ZERO && z == Interval::ZERO {
                continue;
            }
            count = count.saturating_add(1);
            bound.min_x += x.min;
            bound.max_x += x.max;
            bound.min_z += z.min;
            bound.max_z += z.max;
        }
        let too_wide = bound.max_x - bound.min_x > limits.max_push_width
            || bound.max_z - bound.min_z > limits.max_push_width;
        if count > limits.max_pushers || too_wide {
            return None;
        }
        Some(bound)
    }

    /// The push that comes nearest to `error` on each axis.
    pub(crate) fn clamp(&self, error: DVec3) -> DVec3 {
        DVec3::new(
            error.x.clamp(self.min_x, self.max_x),
            0.0,
            error.z.clamp(self.min_z, self.max_z),
        )
    }

    /// The largest length of a push in the bound.
    pub(crate) fn reach(&self) -> f64 {
        let x = self.min_x.abs().max(self.max_x.abs());
        let z = self.min_z.abs().max(self.max_z.abs());
        (x * x + z * z).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Interval {
    min: f64,
    max: f64,
}

impl Interval {
    const ZERO: Self = Self { min: 0.0, max: 0.0 };
}

/// The push on one axis over the range of `Δ`. The push grows with the along component, so the
/// ends of its range are at the ends of that range. Across, it changes only at the nearest
/// value, at the distance cutoff and at the farthest value.
fn interval(min_along: f64, max_along: f64, min_across: f64, max_across: f64) -> Interval {
    let (near, far) = if min_across <= 0.0 && max_across >= 0.0 {
        (0.0, min_across.abs().max(max_across.abs()))
    } else {
        let (a, b) = (min_across.abs(), max_across.abs());
        (a.min(b), a.max(b))
    };
    let across = [near, f64::from(MIN_PUSH_DISTANCE).clamp(near, far), far];
    let pushes = |along: f64| across.map(|value| push_axis(along, value).unwrap_or(0.0));
    Interval {
        min: pushes(min_along)
            .into_iter()
            .fold(f64::INFINITY, f64::min)
            .next_down(),
        max: pushes(max_along)
            .into_iter()
            .fold(f64::NEG_INFINITY, f64::max)
            .next_up(),
    }
}

/// True when the push of the pusher happens at each of its positions: each box meets the player
/// with a volume, and no position is within the cutoff on both axes.
fn certain(pusher: &Pusher, player: &Aabb, [min_dx, max_dx, min_dz, max_dz]: [f64; 4]) -> bool {
    let margin = DVec3::new(pusher.half_width, 0.0, pusher.half_width);
    let covered = Aabb::new(
        pusher.high - margin,
        pusher.low + margin + DVec3::new(0.0, pusher.height, 0.0),
    );
    let always_meets = covered.intersects(player);
    let nearest = |min: f64, max: f64| {
        if min <= 0.0 && max >= 0.0 {
            0.0
        } else {
            min.abs().min(max.abs())
        }
    };
    always_meets
        && nearest(min_dx, max_dx).max(nearest(min_dz, max_dz)) >= f64::from(MIN_PUSH_DISTANCE)
}
