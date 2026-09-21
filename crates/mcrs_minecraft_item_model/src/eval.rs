use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item_component::{
    Bees, BlockState, BundleContents, ComponentMap, ComponentPatch, CustomModelData, Damage,
    DyedColor, EnchantmentGlintOverride, Enchantments, FireworkExplosion, Holder,
    ItemComponentKind, ItemComponentValue, ItemDataComponent, ItemStackValue, MaxDamage,
    MaxStackSize, PotionContents, Trim,
};

use crate::asset::{
    Case, ChargeType, ConditionProperty, DisplayContext, RangeProperty, SelectSwitch, TintSource,
};

/// A stack as the model selectors see it: its effective components and its
/// child stacks, wherever they are stored.
pub trait StackView: Sized {
    fn item(&self) -> &ResourceLocation;
    fn count(&self) -> u8;
    fn value(&self, kind: ItemComponentKind) -> Option<ItemComponentValue>;
    fn has(&self, kind: ItemComponentKind) -> bool;
    /// Vanilla's `hasNonDefault`: the value is not the prototype's, a
    /// tombstone included.
    fn has_non_default(&self, kind: ItemComponentKind) -> bool;
    fn children(&self) -> Vec<Self>;

    fn get<K: ItemDataComponent>(&self) -> Option<K> {
        K::from_value(&self.value(K::KIND)?).cloned()
    }
}

pub fn max_stack_size(stack: &impl StackView) -> u8 {
    stack.get::<MaxStackSize>().map_or(1, |max| max.0.0 as u8)
}

pub fn is_damageable(stack: &impl StackView) -> bool {
    stack.has(ItemComponentKind::MaxDamage)
        && stack.has(ItemComponentKind::Damage)
        && !stack.has(ItemComponentKind::Unbreakable)
}

pub fn max_damage(stack: &impl StackView) -> i32 {
    stack.get::<MaxDamage>().map_or(0, |max| max.0.0)
}

pub fn damage_value(stack: &impl StackView) -> i32 {
    stack
        .get::<Damage>()
        .map_or(0, |damage| damage.0.0)
        .clamp(0, max_damage(stack))
}

pub fn is_damaged(stack: &impl StackView) -> bool {
    is_damageable(stack) && damage_value(stack) > 0
}

pub fn next_damage_will_break(stack: &impl StackView) -> bool {
    is_damageable(stack) && damage_value(stack) >= max_damage(stack) - 1
}

pub fn is_enchanted(stack: &impl StackView) -> bool {
    stack
        .get::<Enchantments>()
        .is_some_and(|enchantments| !enchantments.0.is_empty())
}

pub fn has_foil(stack: &impl StackView) -> bool {
    if let Some(EnchantmentGlintOverride(foil)) = stack.get::<EnchantmentGlintOverride>() {
        return foil;
    }
    // ponytail: the only vanilla override is the compass with a lodestone
    // tracker; a `foil_when_has` field in the dumped corpus is the upgrade.
    if stack.item().as_str() == "minecraft:compass"
        && stack.has(ItemComponentKind::LodestoneTracker)
    {
        return true;
    }
    is_enchanted(stack)
}

/// A bundle's fill fraction: each child weighs `count / max_stack_size`, a
/// nested bundle its own weight plus 1/16, and a hive with bees a full slot.
pub fn bundle_weight<S: StackView>(stack: &S) -> f32 {
    stack
        .children()
        .iter()
        .map(|child| {
            let weight = if child.has(ItemComponentKind::BundleContents) {
                bundle_weight(child) + 1.0 / 16.0
            } else if child.get::<Bees>().is_some_and(|bees| !bees.0.is_empty()) {
                1.0
            } else {
                1.0 / max_stack_size(child) as f32
            };
            f32::from(child.count()) * weight
        })
        .sum()
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

impl<S: StackView> Evaluator<'_, S> {
    fn custom_model_data(&self) -> Option<CustomModelData> {
        self.stack.get::<CustomModelData>()
    }

    pub fn condition(&self, property: &ConditionProperty) -> bool {
        match property {
            ConditionProperty::Damaged => is_damaged(&self.stack),
            ConditionProperty::Broken => next_damage_will_break(&self.stack),
            ConditionProperty::HasComponent {
                component,
                ignore_default,
            } => {
                if *ignore_default {
                    self.stack.has_non_default(component.0)
                } else {
                    self.stack.has(component.0)
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
                    .position(|case| case.when.iter().any(|when| when.0 == value))
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
            return ChargeType::None;
        }
        let rocket = projectiles
            .iter()
            .any(|child| child.item().as_str() == "minecraft:firework_rocket");
        if rocket {
            ChargeType::Rocket
        } else {
            ChargeType::Arrow
        }
    }

    pub fn range(&self, property: &RangeProperty) -> f32 {
        match property {
            RangeProperty::Damage { normalize } => {
                let damage = damage_value(&self.stack) as f32;
                let max = max_damage(&self.stack) as f32;
                if *normalize {
                    (damage / max).clamp(0.0, 1.0)
                } else {
                    damage.clamp(0.0, max)
                }
            }
            RangeProperty::Count { normalize } => {
                let count = f32::from(self.stack.count());
                let max = max_stack_size(&self.stack) as f32;
                if *normalize {
                    (count / max).clamp(0.0, 1.0)
                } else {
                    count.clamp(0.0, max)
                }
            }
            RangeProperty::CustomModelData { index } => self
                .custom_model_data()
                .and_then(|data| data.floats.get(*index as usize).copied())
                .unwrap_or(0.0),
            RangeProperty::BundleFullness => bundle_weight(&self.stack),
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

/// A stack described by its persistent value, resolved against the item's
/// prototype; child stacks come from the value's own child component.
#[derive(Clone)]
pub struct ValueStack<'v, 'p> {
    item: ResourceLocation,
    count: u8,
    patch: &'v ComponentPatch,
    effective: ComponentMap,
    prototypes: &'p dyn Fn(&ResourceLocation) -> Option<&'p ComponentMap>,
}

impl<'v, 'p> ValueStack<'v, 'p> {
    pub fn new(
        value: &'v ItemStackValue,
        prototypes: &'p dyn Fn(&ResourceLocation) -> Option<&'p ComponentMap>,
    ) -> Option<Self> {
        let item = value.item.location().clone();
        let prototype = prototypes(&item)?;
        Some(ValueStack {
            item,
            count: u8::try_from(value.count.0).unwrap_or(u8::MAX),
            patch: &value.components,
            effective: prototype.apply(&value.components),
            prototypes,
        })
    }
}

impl<'v, 'p> StackView for ValueStack<'v, 'p> {
    fn item(&self) -> &ResourceLocation {
        &self.item
    }

    fn count(&self) -> u8 {
        self.count
    }

    fn value(&self, kind: ItemComponentKind) -> Option<ItemComponentValue> {
        self.effective.get_value(kind).cloned()
    }

    fn has(&self, kind: ItemComponentKind) -> bool {
        self.effective.get_value(kind).is_some()
    }

    fn has_non_default(&self, kind: ItemComponentKind) -> bool {
        self.patch.get_value(kind).is_some() || self.patch.is_removed(kind)
    }

    fn children(&self) -> Vec<Self> {
        let values: Vec<&'v ItemStackValue> = match self.patch.get::<BundleContents>() {
            Some(contents) => contents.0.iter().map(|template| &template.0).collect(),
            None => match self.patch.get_value(ItemComponentKind::Container) {
                Some(ItemComponentValue::Container(container)) => container
                    .slots()
                    .iter()
                    .flatten()
                    .map(|template| &template.0)
                    .collect(),
                _ => match self.patch.get_value(ItemComponentKind::ChargedProjectiles) {
                    Some(ItemComponentValue::ChargedProjectiles(list)) => {
                        list.items().iter().map(|template| &template.0).collect()
                    }
                    _ => Vec::new(),
                },
            },
        };
        values
            .into_iter()
            .filter_map(|value| ValueStack::new(value, self.prototypes))
            .collect()
    }
}
