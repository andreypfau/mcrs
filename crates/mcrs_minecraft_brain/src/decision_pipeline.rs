use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use crate::AiDelay;
use crate::Decision;
use crate::StageConfig;
use mcrs_minecraft_entity_key::TileKey;

/// The decisions of one issue tick, per tile of the mob at the issue, each tile in the order of
/// the order keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuedDecisions {
    pub tick: u64,
    pub tiles: BTreeMap<TileKey, Vec<Decision>>,
}

impl IssuedDecisions {
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.values().map(Vec::len).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.values().all(Vec::is_empty)
    }
}

/// The decisions between their issue and their application. The hot tick of `T'` takes the
/// decisions that are due; no decision applies later than `max_lag` ticks after its issue tick,
/// and the decisions of one mob apply in the order of their issue ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionPipeline {
    delay: AiDelay,
    max_lag: u8,
    slots: VecDeque<Slot>,
}

#[derive(Clone, Debug, PartialEq)]
enum Slot {
    Pending(u64),
    Ready(IssuedDecisions),
}

impl Slot {
    fn tick(&self) -> u64 {
        match self {
            Self::Pending(tick) => *tick,
            Self::Ready(decisions) => decisions.tick,
        }
    }
}

/// The hot tick must wait for the decisions of an issue tick that are not ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Wait {
    pub issue_tick: u64,
}

impl fmt::Display for Wait {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the decisions of tick {} are not ready",
            self.issue_tick
        )
    }
}

impl Error for Wait {}

impl DecisionPipeline {
    #[must_use]
    pub fn new(config: &StageConfig) -> Self {
        Self {
            delay: config.delay(),
            max_lag: config.max_lag(),
            slots: VecDeque::new(),
        }
    }

    /// The hot tick issued the work of `tick` to the stage.
    pub fn issue(&mut self, tick: u64) {
        if self.slots.iter().any(|slot| slot.tick() == tick) {
            return;
        }
        let at = self.slots.partition_point(|slot| slot.tick() < tick);
        self.slots.insert(at, Slot::Pending(tick));
    }

    /// The stage finished the work of `decisions.tick`.
    pub fn ready(&mut self, decisions: IssuedDecisions) {
        let tick = decisions.tick;
        let at = self.slots.partition_point(|slot| slot.tick() < tick);
        match self.slots.get_mut(at) {
            Some(slot) if slot.tick() == tick => *slot = Slot::Ready(decisions),
            _ => self.slots.insert(at, Slot::Ready(decisions)),
        }
    }

    /// The decisions that apply in the hot tick `apply_tick`, oldest issue tick first.
    pub fn take_due(&mut self, apply_tick: u64) -> Result<Vec<IssuedDecisions>, Wait> {
        let lag = u64::from(self.delay.ticks());
        let due = match self.delay {
            AiDelay::Fixed(_) => self.fixed_due(apply_tick, lag)?,
            AiDelay::AtLeast(_) => self.ready_due(apply_tick, lag)?,
        };
        let mut taken = Vec::with_capacity(due);
        for _ in 0..due {
            if let Some(Slot::Ready(decisions)) = self.slots.pop_front() {
                taken.push(decisions);
            }
        }
        Ok(taken)
    }

    fn fixed_due(&self, apply_tick: u64, lag: u64) -> Result<usize, Wait> {
        let Some(last) = apply_tick.checked_sub(lag) else {
            return Ok(0);
        };
        let due = self.slots.partition_point(|slot| slot.tick() <= last);
        match self
            .slots
            .range(..due)
            .find(|slot| matches!(slot, Slot::Pending(_)))
        {
            Some(pending) => Err(Wait {
                issue_tick: pending.tick(),
            }),
            None => Ok(due),
        }
    }

    fn ready_due(&self, apply_tick: u64, lag: u64) -> Result<usize, Wait> {
        let Some(last) = apply_tick.checked_sub(lag) else {
            return Ok(0);
        };
        let max_lag = u64::from(self.max_lag);
        let mut due = 0;
        for slot in &self.slots {
            if slot.tick() > last {
                break;
            }
            match slot {
                Slot::Ready(_) => due += 1,
                Slot::Pending(tick) if tick.saturating_add(max_lag) <= apply_tick => {
                    return Err(Wait { issue_tick: *tick });
                }
                Slot::Pending(_) => break,
            }
        }
        Ok(due)
    }
}
