extern crate alloc;

use alloc::collections::{BTreeSet, VecDeque};

use crate::wire::Uint24;

pub const RECEIVE_WINDOW: u32 = 2048;

#[derive(Debug, Default)]
pub(crate) struct ReceiveWindow {
    highest: Option<Uint24>,
    seen: BTreeSet<Uint24>,
    arrival_order: VecDeque<Uint24>,
}

impl ReceiveWindow {
    pub(crate) const fn new() -> Self {
        Self {
            highest: None,
            seen: BTreeSet::new(),
            arrival_order: VecDeque::new(),
        }
    }

    pub(crate) const fn highest(&self) -> Option<Uint24> {
        self.highest
    }

    pub(crate) fn contains(&self, index: Uint24) -> bool {
        self.seen.contains(&index)
    }

    pub(crate) fn accept(&mut self, index: Uint24) -> bool {
        match self.highest {
            Some(highest) if index.is_newer_than(highest) => self.highest = Some(index),
            Some(highest) if highest.wrapping_sub(index) >= RECEIVE_WINDOW => return false,
            Some(_) => {}
            None => self.highest = Some(index),
        }
        if !self.seen.insert(index) {
            return false;
        }
        self.arrival_order.push_back(index);
        self.evict();
        true
    }

    fn evict(&mut self) {
        let Some(highest) = self.highest else {
            return;
        };
        while let Some(&oldest) = self.arrival_order.front() {
            let expired = highest.wrapping_sub(oldest) >= RECEIVE_WINDOW;
            if !expired && self.arrival_order.len() <= RECEIVE_WINDOW as usize {
                break;
            }
            self.arrival_order.pop_front();
            self.seen.remove(&oldest);
        }
    }
}
