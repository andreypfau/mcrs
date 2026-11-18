use mcrs_minecraft_item_stack::EnchantmentId;

use crate::AttackBases;
use crate::EnchantmentEntry;
use crate::ItemAvailability;
use crate::MapSummary;
use crate::TooltipDetail;
use crate::TooltipView;

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct TooltipInputs<'a> {
    pub view: TooltipView,
    pub detail: TooltipDetail,
    pub availability: ItemAvailability,
    pub enchantments: &'a [EnchantmentEntry<'a>],
    pub tooltip_order: &'a [EnchantmentId],
    pub maps: &'a [MapSummary],
    pub attack: AttackBases,
    pub prototype_components: usize,
}

impl TooltipInputs<'_> {
    #[must_use]
    pub fn new(attack: AttackBases, prototype_components: usize) -> Self {
        Self {
            view: TooltipView::Standard,
            detail: TooltipDetail::Basic,
            availability: ItemAvailability::Enabled,
            enchantments: &[],
            tooltip_order: &[],
            maps: &[],
            attack,
            prototype_components,
        }
    }
}
