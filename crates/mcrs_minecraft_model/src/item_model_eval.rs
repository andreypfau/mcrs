use bevy::ecs::world::EntityRef;
use bevy::prelude::Entity;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item::{
    Bees, BlockState, CustomModelData, Damage, DyedColor, EnchantmentGlintOverride, Enchantments,
    FireworkExplosion, ItemComponentValue, ItemDataComponent, MaxDamage, MaxStackSize,
    PotionContents, Trim,
};
use mcrs_minecraft_item::{
    ItemStack, Items, children, component_value, has_component, has_non_default,
};
use mcrs_minecraft_registries::{Namer, Ref, Registries, Resolve};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};

use crate::item_model_asset::{
    Case, ChargeType, ConditionProperty, DisplayContext, RangeProperty, SelectSwitch, TintSource,
};
use mcrs_minecraft_item::RgbInt;
use mcrs_minecraft_item::registry_keys::TRIM_MATERIAL;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::ArgbColor;
use mcrs_minecraft_item_stack::BlockState as StackBlockState;
use mcrs_minecraft_item_stack::CustomModelData as StackCustomModelData;
use mcrs_minecraft_item_stack::DyedColor as StackDyedColor;
use mcrs_minecraft_item_stack::FireworkExplosion as StackFireworkExplosion;
use mcrs_minecraft_item_stack::PotionContents as StackPotionContents;
use mcrs_minecraft_item_stack::RegistryValue;
use mcrs_minecraft_item_stack::Trim as StackTrim;
use mcrs_minecraft_item_stack::TrimMaterialId;

use crate::colormap::colormap_rgba;
use crate::stack_view::StackView;

pub struct EntityStack<'w, 'l, L> {
    pub entity: EntityRef<'w>,
    pub items: &'l Items,
    pub lookup: L,
}

impl<'w, L: Copy + Fn(Entity) -> Option<EntityRef<'w>>> EntityStack<'w, '_, L> {
    fn item(&self) -> &ResourceLocationBuf {
        static AIR: std::sync::LazyLock<ResourceLocationBuf> =
            std::sync::LazyLock::new(|| Item::Air.location().to_buf());
        self.entity
            .get::<ItemStack>()
            .and_then(|stack| self.items.get(stack.item))
            .map_or(&AIR, |entry| &entry.identifier)
    }

    fn count(&self) -> u8 {
        self.entity
            .get::<ItemStack>()
            .map_or(0, |stack| stack.count)
    }

    fn value(&self, kind: DataComponentType) -> Option<ItemComponentValue> {
        component_value(self.entity, kind)
    }

    fn has(&self, kind: DataComponentType) -> bool {
        has_component(self.entity, self.items, kind)
    }

    fn has_non_default(&self, kind: DataComponentType) -> bool {
        has_non_default(self.entity, self.items, kind)
    }

    fn children(&self) -> Vec<Self> {
        children(self.entity, &self.lookup)
            .into_iter()
            .map(|entity| EntityStack {
                entity,
                items: self.items,
                lookup: self.lookup,
            })
            .collect()
    }

    fn get<K: ItemDataComponent>(&self) -> Option<K> {
        K::from_value(&self.value(K::KIND)?).cloned()
    }

    fn max_stack_size(&self) -> u8 {
        self.get::<MaxStackSize>().map_or(1, |max| max.0.0 as u8)
    }

    fn is_damageable(&self) -> bool {
        self.has(DataComponentType::MaxDamage)
            && self.has(DataComponentType::Damage)
            && !self.has(DataComponentType::Unbreakable)
    }

    fn max_damage(&self) -> i32 {
        self.get::<MaxDamage>().map_or(0, |max| max.0.0)
    }

    fn damage_value(&self) -> i32 {
        self.get::<Damage>()
            .map_or(0, |damage| damage.0.0)
            .clamp(0, self.max_damage())
    }

    fn is_damaged(&self) -> bool {
        self.is_damageable() && self.damage_value() > 0
    }

    fn next_damage_will_break(&self) -> bool {
        self.is_damageable() && self.damage_value() >= self.max_damage() - 1
    }

    fn is_enchanted(&self) -> bool {
        self.get::<Enchantments>()
            .is_some_and(|enchantments| !enchantments.0.is_empty())
    }

    pub fn has_foil(&self) -> bool {
        if let Some(EnchantmentGlintOverride(foil)) = self.get::<EnchantmentGlintOverride>() {
            return foil;
        }
        if self.item().as_str() == Item::Compass.as_static_str()
            && self.has(DataComponentType::LodestoneTracker)
        {
            return true;
        }
        self.is_enchanted()
    }

    fn bundle_weight(&self) -> f32 {
        self.children()
            .iter()
            .map(|child| {
                let weight = if child.has(DataComponentType::BundleContents) {
                    child.bundle_weight() + 1.0 / 16.0
                } else if child.get::<Bees>().is_some_and(|bees| !bees.0.is_empty()) {
                    1.0
                } else {
                    1.0 / child.max_stack_size() as f32
                };
                f32::from(child.count()) * weight
            })
            .sum()
    }
}

fn opaque(color: i32) -> u32 {
    color as u32 | 0xFF00_0000
}

pub struct Evaluator<'a> {
    pub stack: StackView<'a>,
    pub registries: Option<&'a Registries>,
    pub grass_colormap: Option<&'a [u8]>,
}

fn scaled(normalize: bool, value: f32, max: f32) -> f32 {
    if normalize {
        (value / max).clamp(0.0, 1.0)
    } else {
        value.min(max).max(0.0)
    }
}

impl<'w, L: Copy + Fn(Entity) -> Option<EntityRef<'w>>> Evaluator<'_, EntityStack<'w, '_, L>> {
    fn custom_model_data(&self) -> Option<CustomModelData> {
        self.stack.get::<CustomModelData>()
    }

    pub fn condition(&self, property: &ConditionProperty) -> bool {
        match property {
            ConditionProperty::Damaged => self.stack.is_damaged(),
            ConditionProperty::Broken => self.stack.next_damage_will_break(),
            ConditionProperty::HasComponent {
                component,
                ignore_default,
            } => {
                if *ignore_default {
                    self.stack.has_non_default(*component)
                } else {
                    self.stack.has(*component)
                }
            }
            ConditionProperty::CustomModelData { index } => {
                self.custom_model_data()
                    .and_then(|data| data.flags.get(*index as usize).copied())
                    == Some(true)
            }
            ConditionProperty::Component(_)
            | ConditionProperty::UsingItem
            | ConditionProperty::Selected
            | ConditionProperty::Carried
            | ConditionProperty::ExtendedView
            | ConditionProperty::KeybindDown { .. }
            | ConditionProperty::ViewEntity
            | ConditionProperty::FishingRodCast
            | ConditionProperty::BundleHasSelectedItem => false,
        }
    }

    pub fn select(&self, switch: &SelectSwitch) -> Option<usize> {
        fn find<T: PartialEq>(cases: &[Case<T>], value: &T) -> Option<usize> {
            cases.iter().position(|case| case.when.contains(value))
        }
        match switch {
            SelectSwitch::TrimMaterial { cases } => match &self.stack.get::<Trim>()?.material {
                Ref::Id(id) => {
                    let name = Namer::name(self.registries?, *id)?.clone();
                    find(cases, &ResourceKeyBuf::from_location(name))
                }
                Ref::Inline(_) => None,
            },
            SelectSwitch::DisplayContext { cases } => find(cases, &DisplayContext::Gui),
            SelectSwitch::BlockState {
                block_state_property,
                cases,
            } => find(
                cases,
                self.stack
                    .get::<BlockState>()?
                    .0
                    .get(block_state_property)?,
            ),
            SelectSwitch::ChargeType { cases } => find(cases, &self.charge_type()),
            SelectSwitch::CustomModelData { index, cases } => find(
                cases,
                self.custom_model_data()?.strings.get(*index as usize)?,
            ),
            SelectSwitch::Component(switch) => {
                let value = self
                    .stack
                    .value(switch.component)?
                    .name(self.registries?)
                    .ok()?;
                switch
                    .cases
                    .iter()
                    .position(|case| case.when.contains(&value))
            }
            SelectSwitch::MainHand { .. }
            | SelectSwitch::LocalTime { .. }
            | SelectSwitch::ContextEntityType { .. }
            | SelectSwitch::ContextDimension { .. } => None,
        }
    }

    pub fn charge_type(&self) -> ChargeType {
        let projectiles = self.stack.children();
        if projectiles.is_empty() {
            ChargeType::None
        } else if projectiles
            .iter()
            .any(|c| c.item().as_str() == Item::FireworkRocket.as_static_str())
        {
            ChargeType::Rocket
        } else {
            ChargeType::Arrow
        }
    }

    pub fn range(&self, property: &RangeProperty) -> f32 {
        match property {
            RangeProperty::Damage { normalize } => scaled(
                *normalize,
                self.stack.damage_value() as f32,
                self.stack.max_damage() as f32,
            ),
            RangeProperty::Count { normalize } => scaled(
                *normalize,
                f32::from(self.stack.count()),
                self.stack.max_stack_size() as f32,
            ),
            RangeProperty::CustomModelData { index } => self
                .custom_model_data()
                .and_then(|data| data.floats.get(*index as usize).copied())
                .unwrap_or(0.0),
            RangeProperty::BundleFullness => self.stack.bundle_weight(),
            RangeProperty::Cooldown
            | RangeProperty::CrossbowPull
            | RangeProperty::UseDuration { .. }
            | RangeProperty::UseCycle { .. }
            | RangeProperty::Time { .. }
            | RangeProperty::Compass { .. } => 0.0,
        }
    }

    pub fn tint(&self, source: &TintSource) -> u32 {
        match source {
            TintSource::Constant { value } => opaque(value.0),
            TintSource::Dye { default } => self
                .stack
                .get::<DyedColor>()
                .map_or(default.0 as u32, |DyedColor(rgb)| opaque(rgb.0)),
            TintSource::Potion { default } => opaque(
                self.stack
                    .get::<PotionContents>()
                    .and_then(|contents| contents.custom_color)
                    .unwrap_or(default.0),
            ),
            TintSource::Firework { default } => {
                let explosion = self.stack.get::<FireworkExplosion>();
                firework(
                    explosion.as_ref().map_or(&[][..], |e| &e.colors[..]),
                    *default,
                )
            }
            TintSource::Grass {
                temperature,
                downfall,
            } => grass(self.grass_colormap, *temperature, *downfall),
            TintSource::CustomModelData { index, default } => opaque(
                self.custom_model_data()
                    .and_then(|data| data.colors.get(*index as usize).map(|color| color.0))
                    .unwrap_or(default.0),
            ),
            TintSource::Team { default } => opaque(default.0),
        }
    }
}

fn firework(colors: &[i32], default: RgbInt) -> u32 {
    match colors {
        [] => default.0 as u32,
        [only] => opaque(*only),
        _ => {
            let count = u64::try_from(colors.len()).unwrap_or(u64::MAX);
            let channel = |shift: u32| {
                let sum: u64 = colors
                    .iter()
                    .map(|color| u64::from((color.cast_unsigned() >> shift) & 0xFF))
                    .sum();
                u32::try_from(sum / count).unwrap_or(0xFF)
            };
            0xFF00_0000 | channel(16) << 16 | channel(8) << 8 | channel(0)
        }
    }
}

fn grass(colormap: Option<&[u8]>, temperature: f32, downfall: f32) -> u32 {
    colormap_rgba(colormap, temperature, downfall).map_or(0xFFFF_00FF, |[r, g, b, _]| {
        0xFF00_0000 | ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32
    })
}

fn position<T, V: ?Sized>(cases: &[Case<T>], value: &V) -> Option<usize>
where
    T: PartialEq<V>,
{
    cases
        .iter()
        .position(|case| case.when.iter().any(|when| when == value))
}

impl Evaluator<'_> {
    #[must_use]
    pub fn condition(&self, property: &ConditionProperty) -> bool {
        match property {
            ConditionProperty::Damaged => self.stack.is_damaged(),
            ConditionProperty::Broken => self.stack.next_damage_will_break(),
            ConditionProperty::HasComponent {
                component,
                ignore_default,
            } => {
                if *ignore_default {
                    self.stack.has_non_default(*component)
                } else {
                    self.stack.has(*component)
                }
            }
            ConditionProperty::CustomModelData { index } => self
                .stack
                .get::<StackCustomModelData>()
                .and_then(|data| data.flags.get(*index as usize))
                .is_some_and(|flag| *flag),
            ConditionProperty::Component(_)
            | ConditionProperty::UsingItem
            | ConditionProperty::Selected
            | ConditionProperty::Carried
            | ConditionProperty::ExtendedView
            | ConditionProperty::KeybindDown { .. }
            | ConditionProperty::ViewEntity
            | ConditionProperty::FishingRodCast
            | ConditionProperty::BundleHasSelectedItem => false,
        }
    }

    #[must_use]
    pub fn select(&self, switch: &SelectSwitch) -> Option<usize> {
        match switch {
            SelectSwitch::TrimMaterial { cases } => {
                let StackTrim {
                    material: RegistryValue::Reference(TrimMaterialId(id)),
                    ..
                } = self.stack.get::<StackTrim>()?
                else {
                    return None;
                };
                let names = self.registries?.names_of(TRIM_MATERIAL.location())?;
                let name = names.get(usize::from(*id))?;
                cases
                    .iter()
                    .position(|case| case.when.iter().any(|key| key.location() == name))
            }
            SelectSwitch::DisplayContext { cases } => position(cases, &DisplayContext::Gui),
            SelectSwitch::BlockState {
                block_state_property,
                cases,
            } => position(
                cases,
                self.stack
                    .get::<StackBlockState>()?
                    .get(block_state_property)?,
            ),
            SelectSwitch::ChargeType { cases } => position(cases, &self.stack.charge_type()),
            SelectSwitch::CustomModelData { index, cases } => position(
                cases,
                self.stack
                    .get::<StackCustomModelData>()?
                    .strings
                    .get(*index as usize)?
                    .as_str(),
            ),
            SelectSwitch::Component(switch) => {
                let value = self.stack.typed_value(switch.component)?;
                switch.cases.iter().position(|case| {
                    case.when
                        .iter()
                        .any(|when| when.value.as_ref() == Some(value))
                })
            }
            SelectSwitch::MainHand { .. }
            | SelectSwitch::LocalTime { .. }
            | SelectSwitch::ContextEntityType { .. }
            | SelectSwitch::ContextDimension { .. } => None,
        }
    }

    #[must_use]
    pub fn range(&self, property: &RangeProperty) -> f32 {
        match property {
            RangeProperty::Damage { normalize } => scaled(
                *normalize,
                self.stack.damage_value() as f32,
                self.stack.max_damage() as f32,
            ),
            RangeProperty::Count { normalize } => {
                scaled(*normalize, self.stack.count(), self.stack.max_stack_size())
            }
            RangeProperty::CustomModelData { index } => self
                .stack
                .get::<StackCustomModelData>()
                .and_then(|data| data.floats.get(*index as usize).copied())
                .unwrap_or(0.0),
            RangeProperty::BundleFullness => self.stack.bundle_weight(),
            RangeProperty::Cooldown
            | RangeProperty::CrossbowPull
            | RangeProperty::UseDuration { .. }
            | RangeProperty::UseCycle { .. }
            | RangeProperty::Time { .. }
            | RangeProperty::Compass { .. } => 0.0,
        }
    }

    #[must_use]
    pub fn tint(&self, source: &TintSource) -> u32 {
        match source {
            TintSource::Constant { value } => opaque(value.0),
            TintSource::Dye { default } => self
                .stack
                .get::<StackDyedColor>()
                .map_or(default.0 as u32, |StackDyedColor(ArgbColor(rgb))| {
                    opaque(*rgb)
                }),
            TintSource::Potion { default } => opaque(
                self.stack
                    .get::<StackPotionContents>()
                    .and_then(|contents| contents.custom_color)
                    .unwrap_or(default.0),
            ),
            TintSource::Firework { default } => firework(
                self.stack
                    .get::<StackFireworkExplosion>()
                    .map_or(&[][..], |explosion| &explosion.colors[..]),
                *default,
            ),
            TintSource::Grass {
                temperature,
                downfall,
            } => grass(self.grass_colormap, *temperature, *downfall),
            TintSource::CustomModelData { index, default } => opaque(
                self.stack
                    .get::<StackCustomModelData>()
                    .and_then(|data| data.colors.get(*index as usize))
                    .map_or(default.0, |color| color.0),
            ),
            TintSource::Team { default } => opaque(default.0),
        }
    }
}
