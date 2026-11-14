use crate::placement::Placement;
use crate::remains::Remains;

/// The result of a landing (FBK-LND-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandingOutcome {
    /// The target cell holds `moving_piston`: the falling block stays, with no other effect.
    Wait,
    /// The break flag is true: no block and no item (FBK-LND-005).
    Break,
    /// The carried state becomes a block (FBK-LND-004).
    Place(Placement),
    /// The falling block breaks into its item (FBK-LND-006).
    Item(Remains),
}
