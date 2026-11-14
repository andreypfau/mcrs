use crate::fall::AtLanding;
use crate::fall::Fall;
use crate::falling_block_table::FallingBlockTable;
use crate::landing_event::LandingEvent;
use crate::landing_outcome::LandingOutcome;
use crate::landing_site::LandingSite;
use crate::landing_site::Replaceable;
use crate::landing_site::Support;
use crate::landing_site::WaterBeside;
use crate::placement::Placement;
use crate::remains::EntityDrops;
use crate::remains::Remains;
use crate::state_flags::StateFlags;
use crate::water_contact::WaterContact;

/// Entity data entry 4 of a falling block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Silence {
    Audible,
    Silent,
}

/// A landing: the result and the sound of an anvil that goes with it. The host applies the ground
/// rule to the velocity first, and removes the falling block for each outcome but
/// [`LandingOutcome::Wait`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub outcome: LandingOutcome,
    /// The event of an anvil that is not silent (FBK-LND-007).
    pub anvil_event: Option<LandingEvent>,
}

impl Landing {
    /// The events that the host sends, in this order: [`LandingEvent::BlockBreak`] for
    /// [`LandingOutcome::Break`] (FBK-LND-005), then the event of the anvil.
    #[must_use = "the host sends each event of a landing"]
    pub fn events(&self) -> impl Iterator<Item = LandingEvent> {
        let block_break =
            matches!(self.outcome, LandingOutcome::Break).then_some(LandingEvent::BlockBreak);
        block_break.into_iter().chain(self.anvil_event)
    }
}

impl Fall {
    /// The landing at `site` (FBK-LND-001 to FBK-LND-007). An item of [`Remains::Item`] with
    /// [`LandingEvent::AnvilDestroy`] stands for a drop: a host whose carried block has no item
    /// makes neither.
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
                anvil_event: audible_anvil.then_some(LandingEvent::AnvilDestroy),
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
                anvil_event: audible_anvil.then_some(LandingEvent::AnvilLand),
            };
        }
        let remains = Remains::of(self.item_drop, entity_drops);
        Landing {
            outcome: LandingOutcome::Item(remains),
            anvil_event: (audible_anvil && remains == Remains::Item)
                .then_some(LandingEvent::AnvilDestroy),
        }
    }
}
