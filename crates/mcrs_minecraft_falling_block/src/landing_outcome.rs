use crate::placement::Placement;
use crate::remains::Remains;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandingOutcome {
    Wait,
    Break,
    Place(Placement),
    Item(Remains),
}
