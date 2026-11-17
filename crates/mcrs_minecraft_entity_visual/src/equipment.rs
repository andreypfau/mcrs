use bevy_ecs::component::Component;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item::kind::ItemComponentValue;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_registries::{Wire, WireId};
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EquippableAsset<'a> {
    None,
    Patched(ResourceLocation<'a>),
    ItemDefault(WireId<Item>),
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Equipment {
    slots: [Option<ProtoStack<Wire>>; EquipmentSlot::ALL.len()],
}

impl Equipment {
    pub fn set(&mut self, slot: EquipmentSlot, stack: ProtoStack<Wire>) {
        if let Some(held) = self.slots.get_mut(slot as usize) {
            *held = (!stack.is_empty()).then_some(stack);
        }
    }

    #[must_use]
    pub fn get(&self, slot: EquipmentSlot) -> Option<&ProtoStack<Wire>> {
        self.slots.get(slot as usize).and_then(Option::as_ref)
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.get(EquipmentSlot::Saddle).is_some()
    }

    #[must_use]
    pub fn asset(&self, slot: EquipmentSlot) -> EquippableAsset<'_> {
        let Some(stack) = self.get(slot) else {
            return EquippableAsset::None;
        };
        if stack.components.is_removed(DataComponentType::Equippable) {
            return EquippableAsset::None;
        }
        match stack.components.get_value(DataComponentType::Equippable) {
            Some(ItemComponentValue::Equippable(equippable)) => equippable
                .asset_id
                .as_ref()
                .map_or(EquippableAsset::None, |asset| {
                    EquippableAsset::Patched(asset.as_location())
                }),
            _ => EquippableAsset::ItemDefault(stack.id),
        }
    }
}
