use alloc::vec::Vec;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::FromWorld;
use bevy_ecs::world::World;
use mcrs_minecraft_enchantment::enchantment_effect_component_type_keys::EnchantmentEffectComponentType;
use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct LockingEnchantments {
    by_enchantment: Vec<bool>,
}

impl LockingEnchantments {
    const NONE: Self = Self {
        by_enchantment: Vec::new(),
    };

    fn of(registries: &Registries) -> Self {
        let Some(registry) = registries.registry::<EnchantmentFile>() else {
            return Self::NONE;
        };
        Self {
            by_enchantment: registry
                .values()
                .iter()
                .map(prevents_armour_change)
                .collect(),
        }
    }

    pub(crate) fn lock(&self, enchantments: &Enchantments) -> bool {
        enchantments
            .entries()
            .iter()
            .any(|entry| self.by_enchantment.get(usize::from(entry.enchantment.0)) == Some(&true))
    }
}

fn prevents_armour_change(file: &EnchantmentFile) -> bool {
    let Some(effects) = file.effects.as_ref().and_then(NbtTag::extract_compound) else {
        return false;
    };
    let effect = EnchantmentEffectComponentType::PreventArmorChange.location();
    effects
        .child_tags
        .iter()
        .any(|(key, _)| ResourceLocation::read(key).is_ok_and(|key| key == effect))
}

impl FromWorld for LockingEnchantments {
    fn from_world(world: &mut World) -> Self {
        world
            .get_resource::<CurrentRegistries>()
            .map(|registries| Self::of(registries))
            .unwrap_or(Self::NONE)
    }
}

pub(crate) fn derive_locking_enchantments(
    _: On<Insert<CurrentRegistries>>,
    registries: Res<CurrentRegistries>,
    mut locking: ResMut<LockingEnchantments>,
) {
    locking.set_if_neq(LockingEnchantments::of(&registries));
}

pub(crate) fn forget_locking_enchantments(
    _: On<Remove<CurrentRegistries>>,
    mut locking: ResMut<LockingEnchantments>,
) {
    locking.set_if_neq(LockingEnchantments::NONE);
}
