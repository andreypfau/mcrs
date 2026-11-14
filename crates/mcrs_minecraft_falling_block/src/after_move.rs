use crate::falling_block::FallingBlock;
use crate::remains::EntityDrops;
use crate::remains::Remains;
use crate::settings::Settings;
use crate::water_contact::WaterContact;

/// The age after which a falling block outside the world height drops, whatever the maximum age
/// (FBK-TCK-005).
const OUTSIDE_AGE: i32 = 100;

/// The on-ground flag after the move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ground {
    Airborne,
    OnGround,
}

/// The minimum `y` and the height of a dimension (FBK-TCK-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldHeight {
    pub min_y: i32,
    pub height: u32,
}

impl WorldHeight {
    /// True when the cell at `y` is outside: the lowest layer `min_y` counts as outside, the
    /// highest layer `min_y + height - 1` as inside.
    #[must_use]
    pub fn is_outside(self, y: i32) -> bool {
        let top = i64::from(self.min_y) + i64::from(self.height) - 1;
        y <= self.min_y || i64::from(y) > top
    }
}

/// The decision of a tick after the move (steps 5 and 6 of FBK-TCK-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterMove {
    /// The host does the landing.
    Land,
    /// The falling block keeps falling.
    Fall,
    /// A time limit ends the falling block: the host makes the remains and removes it.
    Expire(Remains),
}

impl FallingBlock {
    /// The decision of a tick after the move, with the cell of the falling block at `cell_y`. A
    /// falling block lands on the ground or on water contact. Otherwise it expires when its age is
    /// more than the maximum age (FBK-TCK-004), or more than 100 with the cell outside the world
    /// height (FBK-TCK-005).
    #[must_use]
    pub fn after_move(
        &self,
        settings: Settings,
        ground: Ground,
        water: WaterContact,
        cell_y: i32,
        height: WorldHeight,
        entity_drops: EntityDrops,
    ) -> AfterMove {
        if ground == Ground::OnGround || water == WaterContact::Wet {
            return AfterMove::Land;
        }
        let too_old = settings.max_age > 0 && i64::from(self.age) > i64::from(settings.max_age);
        if too_old || (height.is_outside(cell_y) && self.age > OUTSIDE_AGE) {
            AfterMove::Expire(Remains::of(self.item_drop, entity_drops))
        } else {
            AfterMove::Fall
        }
    }
}
