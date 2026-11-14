use crate::fall::Fall;
use crate::ground::Ground;
use crate::remains::EntityDrops;
use crate::remains::Remains;
use crate::settings::Settings;
use crate::water_contact::WaterContact;
use crate::world_height::WorldHeight;

/// The age after which a falling block outside the world height drops, whatever the maximum age
/// (FBK-TCK-005).
const OUTSIDE_AGE: u32 = 100;

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

impl Fall {
    /// The decision of a tick after the move, with the age after step 2 of FBK-TCK-001 and the
    /// cell of the falling block at `cell_y`. A falling block lands on the ground or on water
    /// contact. Otherwise it expires when its age is more than the maximum age (FBK-TCK-004), or
    /// more than 100 with the cell outside the world height (FBK-TCK-005).
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "each input is one value that step 5 or 6 of FBK-TCK-001 reads"
    )]
    pub fn after_move(
        &self,
        age: u32,
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
        let too_old = settings.max_age.is_some_and(|max_age| age > max_age.get());
        if too_old || (height.is_outside(cell_y) && age > OUTSIDE_AGE) {
            AfterMove::Expire(Remains::of(self.item_drop, entity_drops))
        } else {
            AfterMove::Fall
        }
    }
}
