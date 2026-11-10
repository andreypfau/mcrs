use core::ops::RangeInclusive;

use mcrs_minecraft_text::Text;

use crate::ItemStack;
use crate::NameLookup;
use crate::PatchEntry;
use crate::WireError;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::saved_sink::Field;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;

pub(crate) const MAX_COUNT: u8 = 99;
pub(crate) const NOT_NEGATIVE: RangeInclusive<i32> = 0..=i32::MAX;
pub(crate) const POSITIVE: RangeInclusive<i32> = 1..=i32::MAX;
pub(crate) const STACK_SIZE: RangeInclusive<i32> = 1..=MAX_COUNT as i32;

/// Checks that each decoded part of the stack has a saved form: the count is at most 99, each
/// number is in the range of its saved form, and each numeric id has a name. A component that this
/// crate does not decode, or that holds a text, is not checked.
pub(crate) fn check_saved_form<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
) -> Result<(), WireError> {
    if stack.count.get() > MAX_COUNT {
        return Err(WireError::CountOutOfRange);
    }
    for entry in stack.patch() {
        if let PatchEntry::Value(value) = entry {
            saved_form(value, names, &mut Check).map_err(wire_error)?;
        }
    }
    Ok(())
}

fn wire_error(fault: Fault) -> WireError {
    match fault {
        Fault::BadValue(component) => WireError::BadValue(component),
        Fault::OutOfRange(component) => WireError::OutOfRange(component),
        Fault::UnnamedId { component, table } => WireError::UnknownEntry { component, table },
    }
}

/// A sink that builds nothing, so the walk only checks the ranges and the names.
struct Check;

impl SavedSink for Check {
    type Node = ();

    fn leaf(&self, _: Leaf<'_>) {}

    fn text(&mut self, _: &Text) -> Option<()> {
        Some(())
    }

    fn fields(&self, _: impl IntoIterator<Item = Field<()>>) {}

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<(), Fault> {
        for entry in entries {
            entry?;
        }
        Ok(())
    }

    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut element: impl FnMut(&mut Self, T) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        for item in items {
            element(self, item)?;
        }
        Ok(())
    }
}
