use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Bees;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::BundleContents;
use mcrs_minecraft_item_stack::ChargedProjectiles;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentGlintOverride;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_table::ItemTable;

use crate::item_model_asset::ChargeType;

#[derive(Clone, Copy, Debug)]
pub struct StackView<'a> {
    pub stack: &'a ItemStack,
    pub table: &'a ItemTable,
}

impl<'a> StackView<'a> {
    fn nested(&self, stack: &'a ItemStack) -> Self {
        Self {
            stack,
            table: self.table,
        }
    }

    fn is_item(&self, item: Item) -> bool {
        self.table.name(NameTable::Item, self.stack.kind.0) == Some(item.location())
    }

    pub(crate) fn count(&self) -> f32 {
        f32::from(self.stack.count.get())
    }

    pub(crate) fn get<T: ItemComponent + 'static>(&self) -> Option<&'a T> {
        self.table.effective::<T>(self.stack)
    }

    fn patch_entry(&self, kind: DataComponentType) -> Option<&'a PatchEntry> {
        let builtin = BuiltinKey::from_location(kind.location());
        self.stack.patch().iter().find(|entry| {
            let by_name = match entry {
                PatchEntry::Undecoded(value) => value.name.as_location() == kind.location(),
                PatchEntry::UndecodedRemoval(name) => name.as_location() == kind.location(),
                PatchEntry::UndecodedWire(value) => value.component().0 == kind.id().number(),
                PatchEntry::Value(_) | PatchEntry::Removal(_) | PatchEntry::Custom(_) => false,
            };
            by_name || (builtin.is_some() && entry.builtin_key() == builtin)
        })
    }

    fn prototype_value(&self, key: BuiltinKey) -> Option<&'static Builtin> {
        let prototype = self.table.prototype(self.stack.kind)?;
        prototype
            .components()
            .iter()
            .find(|component| component.key() == key)
    }

    pub(crate) fn typed_value(&self, kind: DataComponentType) -> Option<&'a Builtin> {
        match self.patch_entry(kind) {
            Some(PatchEntry::Value(value)) => Some(value),
            Some(
                PatchEntry::Removal(_)
                | PatchEntry::Undecoded(_)
                | PatchEntry::UndecodedRemoval(_)
                | PatchEntry::UndecodedWire(_)
                | PatchEntry::Custom(_),
            ) => None,
            None => self.prototype_value(BuiltinKey::from_location(kind.location())?),
        }
    }

    pub(crate) fn has(&self, kind: DataComponentType) -> bool {
        match self.patch_entry(kind) {
            Some(PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_)) => false,
            Some(
                PatchEntry::Value(_)
                | PatchEntry::Undecoded(_)
                | PatchEntry::UndecodedWire(_)
                | PatchEntry::Custom(_),
            ) => true,
            None => BuiltinKey::from_location(kind.location())
                .is_some_and(|key| self.prototype_value(key).is_some()),
        }
    }

    pub(crate) fn has_non_default(&self, kind: DataComponentType) -> bool {
        match self.patch_entry(kind) {
            Some(PatchEntry::Value(value)) => self.prototype_value(value.key()) != Some(value),
            Some(
                PatchEntry::Undecoded(_) | PatchEntry::UndecodedWire(_) | PatchEntry::Custom(_),
            ) => true,
            Some(PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_)) | None => false,
        }
    }

    pub(crate) fn max_stack_size(&self) -> f32 {
        self.get::<MaxStackSize>().map_or(1.0, |max| max.0 as f32)
    }

    fn is_damageable(&self) -> bool {
        self.has(DataComponentType::MaxDamage)
            && self.has(DataComponentType::Damage)
            && !self.has(DataComponentType::Unbreakable)
    }

    #[must_use]
    pub fn max_damage(&self) -> i32 {
        self.get::<MaxDamage>().map_or(0, |max| max.0)
    }

    #[must_use]
    pub fn damage_value(&self) -> i32 {
        let damage = self.get::<Damage>().map_or(0, |damage| damage.0);
        damage.min(self.max_damage()).max(0)
    }

    #[must_use]
    pub fn is_damaged(&self) -> bool {
        self.is_damageable() && self.damage_value() > 0
    }

    pub(crate) fn next_damage_will_break(&self) -> bool {
        self.is_damageable() && self.damage_value() >= self.max_damage().saturating_sub(1)
    }

    #[must_use]
    pub fn has_foil(&self) -> bool {
        if let Some(EnchantmentGlintOverride(foil)) = self.get::<EnchantmentGlintOverride>() {
            return *foil;
        }
        if self.is_item(Item::Compass) && self.has(DataComponentType::LodestoneTracker) {
            return true;
        }
        self.get::<Enchantments>()
            .is_some_and(|enchantments| !enchantments.entries().is_empty())
    }

    pub(crate) fn bundle_weight(&self) -> f32 {
        let Some(BundleContents(contents)) = self.get::<BundleContents>() else {
            return 0.0;
        };
        contents
            .iter()
            .map(|child| {
                let child = self.nested(child);
                let weight = if child.has(DataComponentType::BundleContents) {
                    child.bundle_weight() + 1.0 / 16.0
                } else if child.get::<Bees>().is_some_and(|bees| !bees.0.is_empty()) {
                    1.0
                } else {
                    1.0 / child.max_stack_size()
                };
                child.count() * weight
            })
            .sum()
    }

    pub(crate) fn charge_type(&self) -> ChargeType {
        let projectiles = self
            .get::<ChargedProjectiles>()
            .map_or(&[][..], |projectiles| &projectiles.0[..]);
        if projectiles.is_empty() {
            ChargeType::None
        } else if projectiles
            .iter()
            .any(|projectile| self.nested(projectile).is_item(Item::FireworkRocket))
        {
            ChargeType::Rocket
        } else {
            ChargeType::Arrow
        }
    }
}
