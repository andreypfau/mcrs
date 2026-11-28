use mcrs_minecraft_physics::PathType;

pub(crate) const TYPES: [PathType; 27] = [
    PathType::Blocked,
    PathType::Open,
    PathType::Walkable,
    PathType::WalkableDoor,
    PathType::DropDown,
    PathType::PowderSnow,
    PathType::OnTopOfPowderSnow,
    PathType::Fence,
    PathType::Lava,
    PathType::Water,
    PathType::WaterBorder,
    PathType::Rail,
    PathType::UnpassableRail,
    PathType::FireInNeighbor,
    PathType::Fire,
    PathType::DamagingInNeighbor,
    PathType::Damaging,
    PathType::DoorOpen,
    PathType::DoorWoodClosed,
    PathType::DoorIronClosed,
    PathType::Breach,
    PathType::Leaves,
    PathType::Sticky,
    PathType::AvoidInAir,
    PathType::DamageCautious,
    PathType::OnTopOfDropDown,
    PathType::BigMobsCloseToDanger,
];

#[derive(Clone, Debug, PartialEq)]
pub struct MalusTable([f32; 27]);
impl Default for MalusTable {
    fn default() -> Self {
        Self(TYPES.map(PathType::default_malus))
    }
}
impl MalusTable {
    #[must_use]
    pub fn new(overrides: impl IntoIterator<Item = (PathType, f32)>) -> Self {
        let mut table = Self::default();
        for (kind, malus) in overrides {
            if let Some(slot) = table.0.get_mut(kind as usize) {
                *slot = malus;
            }
        }
        table
    }
    #[must_use]
    pub fn get(&self, kind: PathType) -> f32 {
        self.0.get(kind as usize).copied().unwrap_or(-1.0)
    }
}
