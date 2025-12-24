use crate::{Decode, Encode, VarInt};
use bevy_ecs::prelude::Component;
use derive_more::{From, Into};
use mcrs_nbt::compound::NbtCompound;
use std::io::Write;
use valence_text::Text;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug, From, Into)]
pub struct ItemId(pub u16);

/// A stack of items in an inventory.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Slot {
    pub id: ItemId,
    pub count: u8,
    pub components: ComponentPatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ContainerInput {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        id: ItemId(0),
        count: 0,
        components: ComponentPatch::EMPTY,
    };

    #[must_use]
    pub const fn new(item: ItemId, count: u8, components: ComponentPatch) -> Self {
        Self {
            id: item,
            count,
            components,
        }
    }

    #[must_use]
    pub const fn with_count(mut self, count: u8) -> Self {
        self.count = count;
        self
    }

    #[must_use]
    pub const fn with_item(mut self, item: ItemId) -> Self {
        self.id = item;
        self
    }

    pub const fn is_empty(&self) -> bool {
        self.id.0 == 0 || self.count == 0
    }
}

impl Encode for Slot {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.count.encode(&mut w)?;
        if self.count == 0 {
            return Ok(());
        }
        VarInt(self.id.0 as i32).encode(&mut w)?;
        self.components.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for Slot {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let count = u8::decode(r)?;
        if count == 0 {
            return Ok(Slot::EMPTY);
        }
        let item = ItemId(u16::decode(r)?);
        let components = ComponentPatch::decode(r)?;
        Ok(Slot {
            id: item,
            count,
            components,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ComponentPatch {
    pub to_add: Vec<ItemComponent>,
    pub to_remove: Vec<ItemComponentKind>,
}

impl ComponentPatch {
    pub const EMPTY: Self = Self {
        to_add: Vec::new(),
        to_remove: Vec::new(),
    };

    pub fn is_empty(&self) -> bool {
        self.to_add.is_empty() && self.to_remove.is_empty()
    }
}

impl Encode for ComponentPatch {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.to_add.len() as i32).encode(&mut w)?;
        VarInt(self.to_remove.len() as i32).encode(&mut w)?;
        for component in &self.to_add {
            component.encode(&mut w)?;
        }
        for component in &self.to_remove {
            component.encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for ComponentPatch {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let to_add_len = VarInt::decode(r)?.0 as usize;
        let to_remove_len = VarInt::decode(r)?.0 as usize;

        let mut to_add = Vec::with_capacity(to_add_len);
        let mut to_remove = Vec::with_capacity(to_remove_len);

        for _ in 0..to_add_len {
            to_add.push(ItemComponent::decode(r)?);
        }
        for _ in 0..to_remove_len {
            to_remove.push(ItemComponentKind::decode(r)?);
        }

        Ok(Self { to_add, to_remove })
    }
}

#[derive(Clone, PartialEq, Debug, Default, Encode, Decode)]
pub struct HashedSlot {
    pub id: ItemId,
    pub count: u8,
    pub components: HashedComponentPatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HashedComponentPatch {
    pub to_add: Vec<(ItemComponentKind, i32)>,
    pub to_remove: Vec<ItemComponentKind>,
}

impl Encode for HashedComponentPatch {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.to_add.len() as i32).encode(&mut w)?;
        VarInt(self.to_remove.len() as i32).encode(&mut w)?;
        for (kind, hash) in &self.to_add {
            kind.encode(&mut w)?;
            hash.encode(&mut w)?;
        }
        for kind in &self.to_remove {
            kind.encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for HashedComponentPatch {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let to_add_len = VarInt::decode(r)?.0 as usize;
        let to_remove_len = VarInt::decode(r)?.0 as usize;

        let mut to_add = Vec::with_capacity(to_add_len);
        let mut to_remove = Vec::with_capacity(to_remove_len);

        for _ in 0..to_add_len {
            let kind = ItemComponentKind::decode(r)?;
            let hash = i32::decode(r)?;
            to_add.push((kind, hash));
        }
        for _ in 0..to_remove_len {
            to_remove.push(ItemComponentKind::decode(r)?);
        }

        Ok(Self { to_add, to_remove })
    }
}

#[derive(Clone, Debug, Copy, Eq, PartialEq, Encode, Decode)]
pub enum ItemComponentKind {
    CustomData,
    MaxStackSize,
    MaxDamage,
    Damage,
    Unbreakable,
    UseEffects,
    CustomName,
    MinimumAttackChange,
    DamageType,
    ItemName,
    ItemModel,
    Lore,
    Rarity,
    Enchantments,
    CanPlaceOn,
    CanBreak,
    AttributeModifiers,
    CustomModelData,
    TooltipDisplay,
    RepairCost,
    CreativeSlotLock,
    EnchantmentGlintOverride,
    IntangibleProjectile,
    Food,
    Consumable,
    UseRemainder,
    UseCooldown,
    DamageResistant,
    Tool,
    Weapon,
    AttackRange,
    Enchantable,
    Equippable,
    Repairable,
    Glider,
    TooltipStyle,
    DeathProtection,
    BlocksAttacks,
    PiercingWeapon,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub enum ItemComponent {
    CustomData(CustomData),
    MaxStackSize(MaxStackSize),
    MaxDamage(VarInt),
    Damage(VarInt),
    Unbreakable,
    UseEffects,
    CustomName(Text),
    MinimumAttackChange,
    DamageType,
    ItemName(Text),
    ItemModel,
    Lore,
    Rarity,
    Enchantments,
    CanPlaceOn,
    CanBreak,
    AttributeModifiers,
    CustomModelData,
    TooltipDisplay,
    RepairCost,
    CreativeSlotLock,
    EnchantmentGlintOverride,
    IntangibleProjectile,
    Food,
    Consumable,
    UseRemainder,
    UseCooldown,
    DamageResistant,
    Tool,
    Weapon,
    AttackRange,
    Enchantable,
    Equippable,
    Repairable,
    Glider,
    TooltipStyle,
    DeathProtection,
    BlocksAttacks,
    PiercingWeapon,
}

impl From<ItemComponent> for ItemComponentKind {
    fn from(value: ItemComponent) -> Self {
        match value {
            ItemComponent::CustomData(_) => ItemComponentKind::CustomData,
            ItemComponent::MaxStackSize(_) => ItemComponentKind::MaxStackSize,
            ItemComponent::MaxDamage(_) => ItemComponentKind::MaxDamage,
            ItemComponent::Damage(_) => ItemComponentKind::Damage,
            ItemComponent::Unbreakable => ItemComponentKind::Unbreakable,
            ItemComponent::UseEffects => ItemComponentKind::UseEffects,
            ItemComponent::CustomName(_) => ItemComponentKind::CustomName,
            ItemComponent::MinimumAttackChange => ItemComponentKind::MinimumAttackChange,
            ItemComponent::DamageType => ItemComponentKind::DamageType,
            ItemComponent::ItemName(_) => ItemComponentKind::ItemName,
            ItemComponent::ItemModel => ItemComponentKind::ItemModel,
            ItemComponent::Lore => ItemComponentKind::Lore,
            ItemComponent::Rarity => ItemComponentKind::Rarity,
            ItemComponent::Enchantments => ItemComponentKind::Enchantments,
            ItemComponent::CanPlaceOn => ItemComponentKind::CanPlaceOn,
            ItemComponent::CanBreak => ItemComponentKind::CanBreak,
            ItemComponent::AttributeModifiers => ItemComponentKind::AttributeModifiers,
            ItemComponent::CustomModelData => ItemComponentKind::CustomModelData,
            ItemComponent::TooltipDisplay => ItemComponentKind::TooltipDisplay,
            ItemComponent::RepairCost => ItemComponentKind::RepairCost,
            ItemComponent::CreativeSlotLock => ItemComponentKind::CreativeSlotLock,
            ItemComponent::EnchantmentGlintOverride => ItemComponentKind::EnchantmentGlintOverride,
            ItemComponent::IntangibleProjectile => ItemComponentKind::IntangibleProjectile,
            ItemComponent::Food => ItemComponentKind::Food,
            ItemComponent::Consumable => ItemComponentKind::Consumable,
            ItemComponent::UseRemainder => ItemComponentKind::UseRemainder,
            ItemComponent::UseCooldown => ItemComponentKind::UseCooldown,
            ItemComponent::DamageResistant => ItemComponentKind::DamageResistant,
            ItemComponent::Tool => ItemComponentKind::Tool,
            ItemComponent::Weapon => ItemComponentKind::Weapon,
            ItemComponent::AttackRange => ItemComponentKind::AttackRange,
            ItemComponent::Enchantable => ItemComponentKind::Enchantable,
            ItemComponent::Equippable => ItemComponentKind::Equippable,
            ItemComponent::Repairable => ItemComponentKind::Repairable,
            ItemComponent::Glider => ItemComponentKind::Glider,
            ItemComponent::TooltipStyle => ItemComponentKind::TooltipStyle,
            ItemComponent::DeathProtection => ItemComponentKind::DeathProtection,
            ItemComponent::BlocksAttacks => ItemComponentKind::BlocksAttacks,
            ItemComponent::PiercingWeapon => ItemComponentKind::PiercingWeapon,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Encode, Decode, Component)]
pub struct CustomData(pub NbtCompound);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode, Component)]
pub struct MaxStackSize(pub u8);

impl Default for MaxStackSize {
    fn default() -> Self {
        Self(64)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode, Component)]
pub struct Lore {
    lines: Vec<Text>,
}

impl Lore {
    pub const fn new(lines: Vec<Text>) -> Self {
        Self { lines }
    }

    pub fn lines(&self) -> &Vec<Text> {
        &self.lines
    }
}
