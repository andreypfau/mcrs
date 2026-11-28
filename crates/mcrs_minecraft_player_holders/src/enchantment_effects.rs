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
pub struct EnchantmentEffects {
    by_enchantment: Vec<Vec<EnchantmentEffectComponentType>>,
}

impl EnchantmentEffects {
    const NONE: Self = Self {
        by_enchantment: Vec::new(),
    };

    fn of(registries: &Registries) -> Self {
        let Some(registry) = registries.registry::<EnchantmentFile>() else {
            return Self::NONE;
        };
        Self {
            by_enchantment: registry.values().iter().map(effects_of).collect(),
        }
    }

    #[must_use]
    pub fn any_has(
        &self,
        enchantments: &Enchantments,
        effect: EnchantmentEffectComponentType,
    ) -> bool {
        enchantments.entries().iter().any(|entry| {
            self.by_enchantment
                .get(usize::from(entry.enchantment.0))
                .is_some_and(|effects| effects.contains(&effect))
        })
    }
}

fn effects_of(file: &EnchantmentFile) -> Vec<EnchantmentEffectComponentType> {
    let Some(effects) = file.effects.as_ref().and_then(NbtTag::extract_compound) else {
        return Vec::new();
    };
    effects
        .child_tags
        .iter()
        .filter_map(|(key, _)| {
            ResourceLocation::read(key)
                .ok()
                .and_then(EnchantmentEffectComponentType::get)
        })
        .collect()
}

impl FromWorld for EnchantmentEffects {
    fn from_world(world: &mut World) -> Self {
        world
            .get_resource::<CurrentRegistries>()
            .map(|registries| Self::of(registries))
            .unwrap_or(Self::NONE)
    }
}

pub(crate) fn derive_enchantment_effects(
    _: On<Insert<CurrentRegistries>>,
    registries: Res<CurrentRegistries>,
    mut effects: ResMut<EnchantmentEffects>,
) {
    effects.set_if_neq(EnchantmentEffects::of(&registries));
}

pub(crate) fn forget_enchantment_effects(
    _: On<Remove<CurrentRegistries>>,
    mut effects: ResMut<EnchantmentEffects>,
) {
    effects.set_if_neq(EnchantmentEffects::NONE);
}
