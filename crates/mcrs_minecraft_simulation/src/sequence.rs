use crate::scalar::{GameTick, TickPriority};

const COUNTER_BITS: u32 = 23;
const COUNTER_MAX: u32 = 0x7F_FFFF;
const TICK_MAX: i64 = 0xFF_FFFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubTick(i64);

impl SubTick {
    pub const fn runtime(tick: GameTick, counter: u32) -> Self {
        let tick = clamp_tick(tick);
        let counter = if counter > COUNTER_MAX {
            COUNTER_MAX
        } else {
            counter
        };
        Self(tick.wrapping_shl(COUNTER_BITS) | counter as i64)
    }

    pub const fn restored(index: u32, count: u32) -> Option<Self> {
        if index < count {
            Some(Self((index as i64).wrapping_sub(count as i64)))
        } else {
            None
        }
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    const fn runtime_parts(self) -> Option<(i64, u32)> {
        if self.0 < 0 {
            None
        } else {
            Some((
                self.0.wrapping_shr(COUNTER_BITS),
                (self.0 & COUNTER_MAX as i64) as u32,
            ))
        }
    }
}

const fn clamp_tick(tick: GameTick) -> i64 {
    if tick.0 < 0 {
        0
    } else if tick.0 > TICK_MAX {
        TICK_MAX
    } else {
        tick.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TickKey {
    pub due: GameTick,
    pub priority: TickPriority,
    pub sequence: SubTick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SequenceClock {
    tick: i64,
    counter: u32,
}

impl SequenceClock {
    pub const fn new() -> Self {
        Self {
            tick: 0,
            counter: 0,
        }
    }

    pub fn stamp(&mut self, now: GameTick) -> SubTick {
        let now = clamp_tick(now);
        if now > self.tick {
            self.tick = now;
            self.counter = 0;
        }
        let stamp = SubTick::runtime(GameTick(self.tick), self.counter);
        // chisle: past 2^23 stamps in one tick the counter saturates and later stamps tie,
        // ordered by position only; a 16-byte (tick, counter) key lifts it.
        self.counter = self.counter.saturating_add(1).min(COUNTER_MAX);
        stamp
    }

    pub fn observe(&mut self, stamp: SubTick) {
        let Some((tick, counter)) = stamp.runtime_parts() else {
            return;
        };
        let next = counter.saturating_add(1).min(COUNTER_MAX);
        if tick > self.tick {
            self.tick = tick;
            self.counter = next;
        } else if tick == self.tick {
            self.counter = self.counter.max(next);
        }
    }

    pub fn join(&mut self, other: &Self) {
        if other.tick > self.tick {
            *self = *other;
        } else if other.tick == self.tick {
            self.counter = self.counter.max(other.counter);
        }
    }
}
