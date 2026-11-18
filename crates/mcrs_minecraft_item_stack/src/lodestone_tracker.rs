use crate::DimensionPos;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct LodestoneTracker {
    pub target: Option<DimensionPos>,
    pub tracked: bool,
}
