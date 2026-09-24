use bevy::ecs::world::EntityRef;
use bevy::prelude::Entity;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::{
    ItemStack, Items, children, component_value, has_component, has_non_default,
};
use mcrs_minecraft_item_component::{
    Bees, BlockState, CustomModelData, Damage, DyedColor, EnchantmentGlintOverride, Enchantments,
    FireworkExplosion, Holder, ItemComponentKind, ItemComponentValue, ItemDataComponent, MaxDamage,
    MaxStackSize, PotionContents, Trim,
};

use super::asset::{
    Case, ChargeType, ConditionProperty, DisplayContext, RangeProperty, SelectSwitch, TintSource,
};

/// A stack entity and the corpus that names its item, as the model selectors see it: its
/// effective components and its child stacks.
pub struct EntityStack<'w, 'l, L> {
    pub entity: EntityRef<'w>,
    pub items: &'l Items,
    pub lookup: L,
}

impl<'w, L: Copy + Fn(Entity) -> Option<EntityRef<'w>>> EntityStack<'w, '_, L> {
    fn item(&self) -> &ResourceLocation {
        static AIR: std::sync::LazyLock<ResourceLocation> =
            std::sync::LazyLock::new(|| ResourceLocation::minecraft("air"));
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

    fn value(&self, kind: ItemComponentKind) -> Option<ItemComponentValue> {
        component_value(self.entity, kind)
    }

    fn has(&self, kind: ItemComponentKind) -> bool {
        has_component(self.entity, self.items, kind)
    }

    /// Vanilla's `hasNonDefault`: the value is not the prototype's, a
    /// tombstone included.
    fn has_non_default(&self, kind: ItemComponentKind) -> bool {
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
        self.has(ItemComponentKind::MaxDamage)
            && self.has(ItemComponentKind::Damage)
            && !self.has(ItemComponentKind::Unbreakable)
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
        // ponytail: the only vanilla override is the compass with a lodestone
        // tracker; a `foil_when_has` field in the dumped corpus is the upgrade.
        if self.item().as_str() == "minecraft:compass"
            && self.has(ItemComponentKind::LodestoneTracker)
        {
            return true;
        }
        self.is_enchanted()
    }

    /// A bundle's fill fraction: each child weighs `count / max_stack_size`, a
    /// nested bundle its own weight plus 1/16, and a hive with bees a full slot.
    fn bundle_weight(&self) -> f32 {
        self.children()
            .iter()
            .map(|child| {
                let weight = if child.has(ItemComponentKind::BundleContents) {
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

/// Answers a model tree's selectors for one stack. Properties that depend on
/// the viewer, the world or the swing are the constants the GUI shows.
pub struct Evaluator<'a, S> {
    pub stack: S,
    /// The grass colormap sample at a temperature and downfall.
    pub grass: &'a dyn Fn(f32, f32) -> Option<[f32; 4]>,
}

fn scaled(normalize: bool, value: f32, max: f32) -> f32 {
    if normalize {
        (value / max).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, max)
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
                Holder::Reference(key) => find(cases, key),
                Holder::Direct(_) => None,
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
                let value = self.stack.value(switch.component)?;
                switch
                    .cases
                    .iter()
                    .position(|case| case.when.iter().any(|when| *when == value))
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
            .any(|c| c.item().as_str() == "minecraft:firework_rocket")
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
            // ponytail: without the custom colour a potion shows the base colour;
            // averaging effect colours needs the potion and mob_effect tables in the corpus.
            TintSource::Potion { default } => opaque(
                self.stack
                    .get::<PotionContents>()
                    .and_then(|contents| contents.custom_color)
                    .unwrap_or(default.0),
            ),
            TintSource::Firework { default } => {
                let explosion = self.stack.get::<FireworkExplosion>();
                match explosion.as_ref().map_or(&[][..], |e| &e.colors[..]) {
                    [] => default.0 as u32,
                    [only] => opaque(*only),
                    colors => {
                        let channel = |shift: u32| {
                            let sum: i32 = colors.iter().map(|c| (c >> shift) & 0xFF).sum();
                            (sum / colors.len() as i32) as u32
                        };
                        0xFF00_0000 | channel(16) << 16 | channel(8) << 8 | channel(0)
                    }
                }
            }
            TintSource::Grass {
                temperature,
                downfall,
            } => (self.grass)(*temperature, *downfall).map_or(0xFFFF_00FF, |[r, g, b, _]| {
                0xFF00_0000
                    | ((r * 255.0) as u32) << 16
                    | ((g * 255.0) as u32) << 8
                    | (b * 255.0) as u32
            }),
            TintSource::CustomModelData { index, default } => opaque(
                self.custom_model_data()
                    .and_then(|data| data.colors.get(*index as usize).map(|color| color.0))
                    .unwrap_or(default.0),
            ),
            TintSource::Team { default } => opaque(default.0),
        }
    }
}
