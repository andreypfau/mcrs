use crate::fall::Fall;
use crate::ground::Ground;
use crate::remains::EntityDrops;
use crate::remains::Remains;
use crate::settings::Settings;
use crate::water_contact::WaterContact;
use crate::world_height::WorldHeight;

const OUTSIDE_AGE: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterMove {
    Land,
    Fall,
    Expire(Remains),
}

impl Fall {
    #[must_use]
    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::too_many_arguments,
            reason = "each input is one value that step 5 or 6 of FBK-TCK-001 reads"
        )
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
