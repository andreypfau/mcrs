use mcrs_minecraft_registries::Registries;

use crate::item_model_asset::{Case, ConditionProperty, RangeProperty, SelectSwitch, TintSource};
use mcrs_minecraft_item::RgbInt;
use mcrs_minecraft_item::registry_keys::TRIM_MATERIAL;
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
use crate::item_model_context::ItemModelContext;
use crate::stack_view::StackView;

fn opaque(color: i32) -> u32 {
    color as u32 | 0xFF00_0000
}

pub struct Evaluator<'a> {
    pub stack: StackView<'a>,
    pub registries: Option<&'a Registries>,
    pub grass_colormap: Option<&'a [u8]>,
    pub context: ItemModelContext,
}

fn scaled(normalize: bool, value: f32, max: f32) -> f32 {
    if normalize {
        (value / max).clamp(0.0, 1.0)
    } else {
        value.min(max).max(0.0)
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
            SelectSwitch::DisplayContext { cases } => position(cases, &self.context.display),
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

#[must_use]
pub fn condition_reads_context(property: &ConditionProperty) -> bool {
    match property {
        ConditionProperty::Damaged
        | ConditionProperty::Broken
        | ConditionProperty::HasComponent { .. }
        | ConditionProperty::Component(_)
        | ConditionProperty::CustomModelData { .. } => false,
        ConditionProperty::UsingItem
        | ConditionProperty::Selected
        | ConditionProperty::Carried
        | ConditionProperty::ExtendedView
        | ConditionProperty::KeybindDown { .. }
        | ConditionProperty::ViewEntity
        | ConditionProperty::FishingRodCast
        | ConditionProperty::BundleHasSelectedItem => true,
    }
}

#[must_use]
pub fn select_reads_context(switch: &SelectSwitch) -> bool {
    match switch {
        SelectSwitch::TrimMaterial { .. }
        | SelectSwitch::DisplayContext { .. }
        | SelectSwitch::BlockState { .. }
        | SelectSwitch::ChargeType { .. }
        | SelectSwitch::CustomModelData { .. }
        | SelectSwitch::Component(_) => false,
        SelectSwitch::MainHand { .. }
        | SelectSwitch::LocalTime { .. }
        | SelectSwitch::ContextEntityType { .. }
        | SelectSwitch::ContextDimension { .. } => true,
    }
}

#[must_use]
pub fn range_reads_context(property: &RangeProperty) -> bool {
    match property {
        RangeProperty::Damage { .. }
        | RangeProperty::Count { .. }
        | RangeProperty::CustomModelData { .. }
        | RangeProperty::BundleFullness => false,
        RangeProperty::Cooldown
        | RangeProperty::CrossbowPull
        | RangeProperty::UseDuration { .. }
        | RangeProperty::UseCycle { .. }
        | RangeProperty::Time { .. }
        | RangeProperty::Compass { .. } => true,
    }
}

#[must_use]
pub fn tint_reads_context(source: &TintSource) -> bool {
    match source {
        TintSource::Constant { .. }
        | TintSource::Dye { .. }
        | TintSource::Potion { .. }
        | TintSource::Firework { .. }
        | TintSource::Grass { .. }
        | TintSource::CustomModelData { .. } => false,
        TintSource::Team { .. } => true,
    }
}
