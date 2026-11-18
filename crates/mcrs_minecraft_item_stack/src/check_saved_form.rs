use core::ops::RangeInclusive;

use mcrs_minecraft_nbt::MAX_DEPTH;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::ItemStack;
use crate::NameLookup;
use crate::PatchEntry;
use crate::Profile;
use crate::WireError;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::saved_sink::Field;
use crate::saved_sink::KeyedEntry;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;

pub(crate) const MAX_COUNT: u8 = 99;
pub(crate) const NOT_NEGATIVE: RangeInclusive<i32> = 0..=i32::MAX;
pub(crate) const POSITIVE: RangeInclusive<i32> = 1..=i32::MAX;
pub(crate) const STACK_SIZE: RangeInclusive<i32> = 1..=MAX_COUNT as i32;
pub(crate) const OMINOUS_AMPLIFIER: RangeInclusive<i32> = 0..=4;
pub(crate) const MAX_NESTING: u32 = MAX_DEPTH / NBT_LEVELS_OF_TEMPLATE;

const NBT_LEVELS_OF_TEMPLATE: u32 = 3;

pub(crate) fn check_saved_form<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
) -> Result<(), WireError> {
    if stack.count.get() > MAX_COUNT {
        return Err(WireError::CountOutOfRange);
    }
    for entry in stack.patch() {
        if let PatchEntry::Value(value) = entry {
            saved_form(value, names, &mut Check).map_err(Fault::into_wire)?;
        }
    }
    Ok(())
}

struct Check;

impl SavedSink for Check {
    type Node = ();

    fn leaf(&self, _: Leaf<'_>) {}

    fn text(&mut self, _: &Text) -> Option<()> {
        Some(())
    }

    fn profile(&mut self, profile: &Profile) -> Option<()> {
        profile.check().ok()
    }

    fn compound(&mut self, _: &NbtCompound) -> Option<()> {
        Some(())
    }

    fn tag(&mut self, _: &NbtTag) -> Option<()> {
        Some(())
    }

    fn entity_data(&mut self, _: &NbtCompound, _: ResourceLocation<'_>) -> Option<()> {
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

    fn keyed_map<'a, T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut entry: impl FnMut(&mut Self, T) -> Result<KeyedEntry<'a, ()>, Fault>,
    ) -> Result<(), Fault> {
        for item in items {
            entry(self, item)?;
        }
        Ok(())
    }
}
