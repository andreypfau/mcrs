use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_creative_report::Group;
use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_item::InstrumentValue;
use mcrs_minecraft_item::PaintingVariantValue;
use mcrs_minecraft_item::instrument_tag_keys;
use mcrs_minecraft_item::painting_variant_tag_keys;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_potion::potion_keys::Potion;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::SessionInputs;

const MIN_STORED_LEVEL: u32 = 1;
const MAX_STORED_LEVEL: u32 = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Placement {
    InTag,
    OutOfTag,
}

#[derive(Clone, Copy)]
enum Levels {
    Max,
    Each,
}

struct Painting<'a> {
    area: i32,
    width: i32,
    name: &'a ResourceLocationBuf,
}

struct GroupStacks<'a, L> {
    group: Group,
    names: &'a L,
    items: &'a ItemTable,
    stacks: &'a mut Vec<ItemStack>,
}

pub(crate) fn push_group_stacks<L: NameLookup>(
    group: Group,
    session: &SessionInputs<'_, L>,
    stacks: &mut Vec<ItemStack>,
) {
    let registries = session.registries;
    let mut out = GroupStacks {
        group,
        names: session.names,
        items: session.items,
        stacks,
    };
    match group {
        Group::PlaceablePaintings => out.paintings(registries, Placement::InTag),
        Group::UnplaceablePaintings => out.paintings(registries, Placement::OutOfTag),
        Group::GoatHorns => out.goat_horns(registries),
        Group::TippedArrows | Group::Potions | Group::SplashPotions | Group::LingeringPotions => {
            out.potions();
        }
        Group::MaxLevelEnchantedBooks => out.enchanted_books(registries, Levels::Max),
        Group::EnchantedBooks => out.enchanted_books(registries, Levels::Each),
        _ => {}
    }
}

impl<L: NameLookup> GroupStacks<'_, L> {
    fn paintings(&mut self, registries: &Registries, placement: Placement) {
        let Some(registry) = registries.registry::<PaintingVariantValue>() else {
            return;
        };
        let Some(tags) = registries.tags::<PaintingVariantValue>() else {
            return;
        };
        let Some(placeable) = tags.id(painting_variant_tag_keys::PLACEABLE) else {
            return;
        };
        let mut paintings: Vec<Painting<'_>> = registry
            .iter()
            .filter(|(id, _, _)| {
                let in_tag = tags.contains(placeable, *id);
                in_tag == (placement == Placement::InTag)
            })
            .map(|(_, name, variant)| Painting {
                area: variant.width.0.saturating_mul(variant.height.0),
                width: variant.width.0,
                name,
            })
            .collect();
        paintings.sort_by_key(|painting| (painting.area, painting.width));
        for painting in paintings {
            self.push(NbtTag::String(painting.name.as_str().into()));
        }
    }

    fn goat_horns(&mut self, registries: &Registries) {
        let Some(entries) = registries.entries::<InstrumentValue>() else {
            return;
        };
        let Some(tags) = registries.tags::<InstrumentValue>() else {
            return;
        };
        let Some(goat_horns) = tags.id(instrument_tag_keys::GOAT_HORNS) else {
            return;
        };
        for id in tags.members(goat_horns) {
            if let Some(name) = entries.name(id) {
                self.push(NbtTag::String(name.as_str().into()));
            }
        }
    }

    fn potions(&mut self) {
        for potion in Potion::ALL {
            self.push(compound(
                String::from("potion"),
                NbtTag::String(potion.as_static_str().into()),
            ));
        }
    }

    fn enchanted_books(&mut self, registries: &Registries, levels: Levels) {
        let Some(registry) = registries.registry::<EnchantmentFile>() else {
            return;
        };
        for (_, name, enchantment) in registry.iter() {
            let max_level = enchantment.max_level;
            let first = match levels {
                Levels::Max => max_level,
                Levels::Each => MIN_STORED_LEVEL,
            };
            let last = max_level.min(MAX_STORED_LEVEL);
            for level in first.max(MIN_STORED_LEVEL)..=last {
                let Ok(level) = i32::try_from(level) else {
                    continue;
                };
                self.push(compound(name.to_string(), NbtTag::Int(level)));
            }
        }
    }

    fn push(&mut self, value: NbtTag) {
        let component: ResourceLocation<'_> = self.group.component();
        let saved = NbtCompound {
            child_tags: vec![
                (
                    String::from("id"),
                    NbtTag::String(self.group.item().to_string()),
                ),
                (
                    String::from("components"),
                    compound(component.to_string(), value),
                ),
            ],
        };
        if let Ok(stack) = read_saved(&saved, self.names) {
            self.stacks.push(without_defaults(stack, self.items));
        }
    }
}

fn without_defaults(stack: ItemStack, items: &ItemTable) -> ItemStack {
    let Some(prototype) = items.prototype(stack.kind) else {
        return stack;
    };
    let is_default = |entry: &PatchEntry| matches!(entry, PatchEntry::Value(value) if prototype.components().contains(value));
    if !stack.patch().iter().any(is_default) {
        return stack;
    }
    stack
        .patch()
        .iter()
        .filter(|entry| !is_default(entry))
        .cloned()
        .fold(
            ItemStack::new(stack.kind, stack.count),
            ItemStack::with_entry,
        )
}

fn compound(key: String, value: NbtTag) -> NbtTag {
    NbtTag::Compound(NbtCompound {
        child_tags: vec![(key, value)],
    })
}
