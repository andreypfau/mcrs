use std::cell::Cell;
use std::fmt;

use crate::{BYTE_ID, DOUBLE_ID, Error, FLOAT_ID, INT_ID, LONG_ID, SHORT_ID};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtBudget {
    pub bytes: u64,
    pub tags: u64,
}

impl Default for NbtBudget {
    fn default() -> Self {
        Self {
            bytes: 33_554_432,
            tags: 262_144,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtQuotaExceeded;

impl fmt::Display for NbtQuotaExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the network NBT exceeds its budget")
    }
}

impl core::error::Error for NbtQuotaExceeded {}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Spent {
    budget: NbtBudget,
    bytes: u64,
    tags: u64,
}

impl Spent {
    fn exceeded(self) -> bool {
        self.bytes > self.budget.bytes || self.tags > self.budget.tags
    }
}

thread_local! {
    static SPENT: Cell<Option<Spent>> = const { Cell::new(None) };
}

struct Restore(Option<Spent>);

impl Drop for Restore {
    fn drop(&mut self) {
        SPENT.set(self.0);
    }
}

pub fn with_nbt_budget<T>(
    budget: NbtBudget,
    read: impl FnOnce() -> T,
) -> Result<T, NbtQuotaExceeded> {
    let opened = SPENT.get().is_none().then(|| {
        SPENT.set(Some(Spent {
            budget,
            bytes: 0,
            tags: 0,
        }));
        Restore(None)
    });
    let value = read();
    let exceeded = SPENT.get().is_some_and(Spent::exceeded);
    drop(opened);
    if exceeded {
        Err(NbtQuotaExceeded)
    } else {
        Ok(value)
    }
}

pub fn without_nbt_budget<T>(read: impl FnOnce() -> T) -> T {
    let _restore = Restore(SPENT.take());
    read()
}

pub(crate) fn spent() -> Option<Spent> {
    SPENT.get()
}

pub(crate) fn rewind(to: Option<Spent>) {
    SPENT.set(to);
}

pub(crate) fn charge(bytes: u64, tags: u64) -> Result<(), Error> {
    let Some(mut spent) = SPENT.get() else {
        return Ok(());
    };
    spent.bytes = spent.bytes.saturating_add(bytes);
    spent.tags = spent.tags.saturating_add(tags);
    SPENT.set(Some(spent));
    if spent.exceeded() {
        return Err(Error::QuotaExceeded);
    }
    Ok(())
}

pub(crate) fn value_charge(kind: u8) -> u64 {
    match kind {
        BYTE_ID => 1,
        SHORT_ID => 2,
        INT_ID | FLOAT_ID => 4,
        LONG_ID | DOUBLE_ID => 8,
        _ => 0,
    }
}
