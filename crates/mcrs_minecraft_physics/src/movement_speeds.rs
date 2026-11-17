use core::iter;

use mcrs_minecraft_entity::SPRINT_MODIFIER;
use mcrs_minecraft_entity_attribute::{AttributeModifier, Modifier, ValueRange, fold};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MovementSpeeds {
    pub(crate) walking: f64,
    pub(crate) sprinting: f64,
}

#[must_use]
pub(crate) fn movement_speeds<'a, I>(base: f64, modifiers: I, range: ValueRange) -> MovementSpeeds
where
    I: IntoIterator<Item = AttributeModifier<'a>>,
    I::IntoIter: Clone,
{
    let others = modifiers
        .into_iter()
        .filter(|modifier| modifier.id != SPRINT_MODIFIER.id)
        .map(Modifier::from);
    let sprint = Modifier::from(SPRINT_MODIFIER);
    MovementSpeeds {
        walking: range.clamp(fold(base, others.clone())),
        sprinting: range.clamp(fold(base, others.chain(iter::once(sprint)))),
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_entity_attribute::ModifierOperation;
    use mcrs_minecraft_resource::ResourceLocation;

    use super::*;

    const BASE: f64 = 0.1_f32 as f64;
    const RANGE: ValueRange = ValueRange {
        min: 0.0,
        max: 1024.0,
    };

    fn effect(path: &'static str, amount_bits: u64) -> AttributeModifier<'static> {
        AttributeModifier {
            id: ResourceLocation::new("minecraft", path).expect("a valid path"),
            amount: f64::from_bits(amount_bits),
            operation: ModifierOperation::AddMultipliedTotal,
        }
    }

    struct Case<'a> {
        base: f64,
        modifiers: &'a [AttributeModifier<'static>],
        walking: Option<u64>,
        sprinting: u64,
    }

    #[test]
    fn folds_the_sprint_modifier_in_the_order_of_its_home_slot() {
        let speed = effect("effect.speed", 0x3fc9_9999_a000_0000);
        let slowness = effect("effect.slowness", 0xbfdc_cccc_e000_0000);
        let sprint = effect("sprinting", 0x3fd3_3333_4000_0000);
        let cases = [
            Case {
                base: BASE,
                modifiers: &[speed],
                walking: Some(0x3fbe_b851_f47a_e148),
                sprinting: 0x3fc3_f7ce_e1fb_e76e,
            },
            Case {
                base: BASE,
                modifiers: &[sprint, slowness],
                walking: Some(0x3fac_28f5_ba3d_70a0),
                sprinting: 0x3fb2_4dd2_ef12_6e94,
            },
            Case {
                base: BASE,
                modifiers: &[slowness, speed],
                walking: None,
                sprinting: 0x3fb5_f6fd_1fcd_35a5,
            },
            Case {
                base: 1000.0,
                modifiers: &[],
                walking: Some(1000.0_f64.to_bits()),
                sprinting: 1024.0_f64.to_bits(),
            },
        ];
        for case in cases {
            let modifiers = case.modifiers;
            let speeds = movement_speeds(case.base, modifiers.iter().copied(), RANGE);
            if let Some(walking) = case.walking {
                assert_eq!(speeds.walking.to_bits(), walking, "{modifiers:?}");
            }
            assert_eq!(speeds.sprinting.to_bits(), case.sprinting, "{modifiers:?}");
        }
    }
}
