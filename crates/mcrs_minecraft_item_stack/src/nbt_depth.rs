use mcrs_minecraft_nbt::MAX_DEPTH;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::depth_of;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::tag::deepest;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::ItemStack;
use crate::NameLookup;
use crate::Profile;
use crate::saved_form::Fault;
use crate::saved_form::saved_patch;
use crate::saved_sink::Field;
use crate::saved_sink::KeyedEntry;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::WireOnly;

pub(crate) const STACK_LEVEL: u32 = 1;
pub(crate) const LIST_TEMPLATE_LEVELS: u32 = 3;
pub(crate) const SLOT_TEMPLATE_LEVELS: u32 = 4;

pub(crate) fn nbt_depth<L: NameLookup>(stack: &ItemStack, names: &L) -> Result<u32, Fault> {
    let components = saved_patch(stack, names, &mut DepthSink, STACK_LEVEL, Fault::TooDeep)?;
    Ok(DepthSink.fields([Field::optional("components", components)]))
}

pub(crate) fn check_nbt_depth<L: NameLookup>(stack: &ItemStack, names: &L) -> Result<(), Fault> {
    if nbt_depth(stack, names)? > MAX_DEPTH {
        return Err(Fault::TooDeep);
    }
    Ok(())
}

fn deepest_of(levels: impl IntoIterator<Item = Result<u32, Fault>>) -> Result<u32, Fault> {
    let mut deepest = 0;
    for level in levels {
        deepest = level?.max(deepest);
    }
    Ok(deepest.saturating_add(1))
}

struct DepthSink;

impl SavedSink for DepthSink {
    type Node = u32;

    const WIRE_ONLY: WireOnly = WireOnly::Skip;

    fn leaf(&self, _: Leaf<'_>) -> u32 {
        0
    }

    fn text(&mut self, text: &Text) -> Option<u32> {
        depth_of(text, MAX_DEPTH)
    }

    fn profile(&mut self, profile: &Profile) -> Option<u32> {
        depth_of(profile, MAX_DEPTH)
    }

    fn compound(&mut self, compound: &NbtCompound) -> Option<u32> {
        Some(compound.depth(MAX_DEPTH))
    }

    fn tag(&mut self, tag: &NbtTag) -> Option<u32> {
        Some(tag.depth(MAX_DEPTH))
    }

    fn entity_data(&mut self, data: &NbtCompound, _: ResourceLocation<'_>) -> Option<u32> {
        self.compound(data)
    }

    fn fields(&self, fields: impl IntoIterator<Item = Field<u32>>) -> u32 {
        deepest(fields.into_iter().filter_map(|field| field.value))
    }

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<u32, Fault> {
        for entry in entries {
            entry?;
        }
        Ok(1)
    }

    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut element: impl FnMut(&mut Self, T) -> Result<u32, Fault>,
    ) -> Result<u32, Fault> {
        deepest_of(items.into_iter().map(|item| element(self, item)))
    }

    fn keyed_map<'a, T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut entry: impl FnMut(&mut Self, T) -> Result<KeyedEntry<'a, u32>, Fault>,
    ) -> Result<u32, Fault> {
        deepest_of(
            items
                .into_iter()
                .map(|item| entry(self, item).map(|entry| entry.value)),
        )
    }
}
