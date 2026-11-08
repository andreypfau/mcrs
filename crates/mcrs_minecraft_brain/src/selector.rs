use crate::ControlFlags;
use crate::GoalSpec;
use crate::Interrupt;

/// The goals of one mob for one full evaluation. Each method names a goal by its index in
/// [`Goals::specs`]; the list holds at most [`Selector::CAPACITY`] goals.
pub trait Goals {
    fn specs(&self) -> &[GoalSpec];
    fn can_start(&mut self, goal: usize) -> bool;
    fn can_continue(&mut self, goal: usize) -> bool;
    fn start(&mut self, goal: usize);
    fn stop(&mut self, goal: usize);
    fn tick(&mut self, goal: usize);
}

/// The goal selection of one goal list: the goals that run and the holder of each control flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selector {
    running: u32,
    holders: [Option<u8>; 4],
}

impl Selector {
    pub const CAPACITY: usize = 32;

    /// One full evaluation: stop the goals that hold a disabled flag or may not continue, start
    /// each examined goal whose start condition holds, then tick each running goal, all in the
    /// order of the list.
    pub fn evaluate<G: Goals>(&mut self, goals: &mut G, disabled: ControlFlags) {
        let count = goals.specs().len().min(Self::CAPACITY);
        for goal in 0..count {
            if !self.is_running(goal) {
                continue;
            }
            let Some(spec) = goals.specs().get(goal).copied() else {
                continue;
            };
            if spec.flags.intersects(disabled) || !goals.can_continue(goal) {
                self.stop(goals, goal, spec);
            }
        }
        for goal in 0..count {
            if self.is_running(goal) || !self.is_examined(goals.specs(), goal, disabled) {
                continue;
            }
            let Some(spec) = goals.specs().get(goal).copied() else {
                continue;
            };
            if !goals.can_start(goal) {
                continue;
            }
            for slot in spec.flags.slots() {
                let holder = self.holders.get(slot).copied().flatten();
                if let Some(holder) = holder.map(usize::from)
                    && let Some(holder_spec) = goals.specs().get(holder).copied()
                {
                    self.stop(goals, holder, holder_spec);
                }
            }
            self.take(goal, spec.flags);
            goals.start(goal);
        }
        for goal in 0..count {
            if self.is_running(goal) {
                goals.tick(goal);
            }
        }
    }

    #[must_use]
    pub fn is_running(&self, goal: usize) -> bool {
        goal < Self::CAPACITY && self.running & (1 << goal) != 0
    }

    /// The goal that holds the flag in `slot`, from 0 for `move` to 3 for `target`.
    #[must_use]
    pub fn holder(&self, slot: usize) -> Option<usize> {
        self.holders.get(slot).copied().flatten().map(usize::from)
    }

    /// True when a full evaluation would evaluate the start condition of `goal`: it does not
    /// run, holds no disabled flag, and each of its flags is free or held by a goal with a larger
    /// priority number that permits an interrupt.
    #[must_use]
    pub fn is_examined(&self, specs: &[GoalSpec], goal: usize, disabled: ControlFlags) -> bool {
        let Some(spec) = specs.get(goal) else {
            return false;
        };
        if self.is_running(goal) || spec.flags.intersects(disabled) {
            return false;
        }
        spec.flags.slots().all(|slot| match self.holder(slot) {
            None => true,
            Some(holder) => specs.get(holder).is_some_and(|holder_spec| {
                holder_spec.interrupt == Interrupt::Permitted
                    && holder_spec.priority > spec.priority
            }),
        })
    }

    fn take(&mut self, goal: usize, flags: ControlFlags) {
        self.running |= 1 << goal;
        let Ok(index) = u8::try_from(goal) else {
            return;
        };
        for slot in flags.slots() {
            if let Some(holder) = self.holders.get_mut(slot) {
                *holder = Some(index);
            }
        }
    }

    fn stop<G: Goals>(&mut self, goals: &mut G, goal: usize, spec: GoalSpec) {
        self.running &= !(1 << goal);
        for slot in spec.flags.slots() {
            if let Some(holder) = self.holders.get_mut(slot)
                && holder.map(usize::from) == Some(goal)
            {
                *holder = None;
            }
        }
        goals.stop(goal);
    }
}
