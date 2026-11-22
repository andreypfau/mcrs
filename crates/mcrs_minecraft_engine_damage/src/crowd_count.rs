use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::pushes;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrowdCount {
    pub own: GridRow,
    pub candidates: u32,
    pub free: u32,
    pub limit: i64,
}

impl CrowdCount {
    pub(crate) fn overcrowded(&self, dead: &[GridRow]) -> bool {
        let own = &self.own;
        let bounds = own.bounds();
        let mut count = i64::from(self.candidates);
        let mut free = i64::from(self.free);
        for gone in dead {
            if pushes(own, gone) && gone.bounds().intersects(&bounds) {
                count -= 1;
                free -= i64::from(!gone.flags.contains(GridFlags::IS_PASSENGER));
            }
        }
        count > self.limit && free > self.limit
    }
}
