use mcrs_voxel::BlockStateId;

use crate::falling_block::AtLanding;
use crate::falling_block::FallingBlock;
use crate::falling_block_table::FallingBlockTable;
use crate::remains::EntityDrops;
use crate::remains::Remains;
use crate::state_flags::StateFlags;
use crate::water_contact::WaterContact;

/// Whether the state at the target cell is replaceable at a landing (FBK-LND-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Replaceable {
    Replaceable,
    Occupied,
}

/// Whether the carried state can stay at the target cell by its support predicate (FBK-LND-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Support {
    Stays,
    Fails,
}

/// Whether a neighbour above the target cell or on one of its four horizontal sides holds water of
/// the tag `minecraft:water`, with a face toward the target cell that is not sturdy (FBK-LND-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterBeside {
    Absent,
    Present,
}

/// Entity data entry 4 of a falling block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Silence {
    Audible,
    Silent,
}

/// What the host reads from the world at the target cell for a landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingSite {
    /// The state at the target cell.
    pub target: BlockStateId,
    /// The state below the target cell.
    pub below: BlockStateId,
    pub replaceable: Replaceable,
    pub support: Support,
    pub water_contact: WaterContact,
    pub water_beside: WaterBeside,
}

/// The block that a landing places (FBK-LND-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The carried state, waterlogged on a water source, set at step 2.
    pub placed: BlockStateId,
    /// The default state of the concrete, set at step 5, when the concrete condition is true.
    pub concrete: Option<BlockStateId>,
}

impl Placement {
    /// The state that the landing leaves at the target cell, which the packets of FBK-NET-002
    /// carry.
    #[must_use]
    pub fn final_state(&self) -> BlockStateId {
        self.concrete.unwrap_or(self.placed)
    }
}

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

/// A `level_event` that a landing sends to the players of BRK-BRK-006.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LevelEvent {
    /// The type 2001 with the network id of the carried state, at the cell of the centre of the box
    /// (FBK-LND-005).
    BlockBreak,
    /// The type 1031 with the data 0 at the target cell (FBK-LND-007).
    AnvilLand,
    /// The type 1029 with the data 0 at the target cell (FBK-LND-007).
    AnvilDestroy,
}

impl LevelEvent {
    /// The type of the event on the wire.
    #[must_use]
    pub fn id(self) -> i32 {
        match self {
            Self::BlockBreak => 2001,
            Self::AnvilLand => 1031,
            Self::AnvilDestroy => 1029,
        }
    }
}

/// A landing: the result and the sound of an anvil that goes with it. The host applies the ground
/// rule to the velocity first, and removes the falling block for each outcome but
/// [`LandingOutcome::Wait`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub outcome: LandingOutcome,
    /// The event of an anvil that is not silent (FBK-LND-007).
    pub anvil_event: Option<LevelEvent>,
}

impl Landing {
    /// The events that the host sends, in this order: [`LevelEvent::BlockBreak`] for
    /// [`LandingOutcome::Break`] (FBK-LND-005), then the event of the anvil.
    pub fn events(&self) -> impl Iterator<Item = LevelEvent> {
        let block_break =
            matches!(self.outcome, LandingOutcome::Break).then_some(LevelEvent::BlockBreak);
        block_break.into_iter().chain(self.anvil_event)
    }
}

impl FallingBlock {
    /// The landing at `site` (FBK-LND-001 to FBK-LND-007). An item of [`Remains::Item`] with
    /// [`LevelEvent::AnvilDestroy`] stands for a drop: a host whose carried block has no item makes
    /// neither.
    #[must_use]
    pub fn land(
        &self,
        table: &FallingBlockTable,
        site: &LandingSite,
        entity_drops: EntityDrops,
        silence: Silence,
    ) -> Landing {
        if table.flags(site.target).contains(StateFlags::MOVING_PISTON) {
            return Landing {
                outcome: LandingOutcome::Wait,
                anvil_event: None,
            };
        }
        let audible_anvil =
            table.flags(self.carried).contains(StateFlags::ANVIL) && silence == Silence::Audible;
        if self.at_landing == AtLanding::Break {
            return Landing {
                outcome: LandingOutcome::Break,
                anvil_event: audible_anvil.then_some(LevelEvent::AnvilDestroy),
            };
        }
        let rests = !table.is_free(site.below) || site.water_contact == WaterContact::Wet;
        if site.replaceable == Replaceable::Replaceable && site.support == Support::Stays && rests {
            let target = table.flags(site.target);
            let placed = if target.contains(StateFlags::WATER_SOURCE) {
                table.wet(self.carried)
            } else {
                self.carried
            };
            let hardens = target.contains(StateFlags::HOLDS_WATER)
                || site.water_beside == WaterBeside::Present;
            return Landing {
                outcome: LandingOutcome::Place(Placement {
                    placed,
                    concrete: table.concrete(self.carried).filter(|_| hardens),
                }),
                anvil_event: audible_anvil.then_some(LevelEvent::AnvilLand),
            };
        }
        let remains = Remains::of(self.item_drop, entity_drops);
        Landing {
            outcome: LandingOutcome::Item(remains),
            anvil_event: (audible_anvil && remains == Remains::Item)
                .then_some(LevelEvent::AnvilDestroy),
        }
    }
}
