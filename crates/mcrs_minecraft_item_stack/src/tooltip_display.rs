use alloc::borrow::Cow;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::hash::Hash;
use core::hash::Hasher;

use crate::ComponentType;
use crate::Neutral;
use crate::entity_form::EntityForm;

/// What the tooltip of the stack does not show.
// The hidden types hold no type two times. Equality and the hash read them as a set.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct TooltipDisplay {
    pub hide_tooltip: bool,
    hidden_components: Cow<'static, [ComponentType]>,
}

impl TooltipDisplay {
    /// A tooltip that hides the components of these types, in the order of the input. A type that
    /// occurs again is dropped.
    #[must_use]
    pub fn hiding(hidden_components: impl IntoIterator<Item = ComponentType>) -> Self {
        let mut seen: BTreeSet<ComponentType> = BTreeSet::new();
        let hidden_components: Vec<ComponentType> = hidden_components
            .into_iter()
            .filter(|component_type| seen.insert(*component_type))
            .collect();
        Self {
            hide_tooltip: false,
            hidden_components: Cow::Owned(hidden_components),
        }
    }

    /// The types whose components the tooltip does not show, in the order of the input.
    #[must_use]
    pub fn hidden_components(&self) -> &[ComponentType] {
        &self.hidden_components
    }
}

impl PartialEq for TooltipDisplay {
    fn eq(&self, other: &Self) -> bool {
        let hidden: &[ComponentType] = &self.hidden_components;
        let other_hidden: &[ComponentType] = &other.hidden_components;
        self.hide_tooltip == other.hide_tooltip
            && hidden.len() == other_hidden.len()
            && (hidden == other_hidden
                || hidden
                    .iter()
                    .all(|component_type| other_hidden.contains(component_type)))
    }
}

impl Eq for TooltipDisplay {}

impl Hash for TooltipDisplay {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hide_tooltip.hash(state);
        self.hidden_components.len().hash(state);
        let mut combined: u64 = 0;
        for component_type in self.hidden_components.iter() {
            combined = combined.wrapping_add(mixed(*component_type));
        }
        combined.hash(state);
    }
}

// A sum of the bare ids gives one hash to each set with the same sum, so each id is spread over
// 64 bits first.
fn mixed(component_type: ComponentType) -> u64 {
    let mut bits = u64::from(component_type.0).wrapping_add(0x9e37_79b9_7f4a_7c15);
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    bits ^ (bits >> 31)
}

impl Default for TooltipDisplay {
    fn default() -> Self {
        Self {
            hide_tooltip: false,
            hidden_components: Cow::Borrowed(&[]),
        }
    }
}

impl Neutral for TooltipDisplay {
    const NEUTRAL: &'static Self = &Self {
        hide_tooltip: false,
        hidden_components: Cow::Borrowed(&[]),
    };
}

impl EntityForm for TooltipDisplay {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);

    fn borrow_static(&'static self) -> Self {
        Self {
            hide_tooltip: self.hide_tooltip,
            hidden_components: Cow::Borrowed(&self.hidden_components),
        }
    }
}
