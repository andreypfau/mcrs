use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::ComponentType;
use crate::Neutral;

/// What the tooltip of the stack does not show.
// The hidden types are in the order of the type ids with no type two times, so the derived
// equality compares them as sets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct TooltipDisplay {
    pub hide_tooltip: bool,
    hidden_components: Cow<'static, [ComponentType]>,
}

impl TooltipDisplay {
    /// A tooltip that hides the components of these types. A type that occurs again is dropped.
    #[must_use]
    pub fn hiding(hidden_components: impl IntoIterator<Item = ComponentType>) -> Self {
        let mut hidden_components: Vec<ComponentType> = hidden_components.into_iter().collect();
        hidden_components.sort_unstable();
        hidden_components.dedup();
        Self {
            hide_tooltip: false,
            hidden_components: Cow::Owned(hidden_components),
        }
    }

    /// The types whose components the tooltip does not show, in the order of the type ids.
    #[must_use]
    pub fn hidden_components(&self) -> &[ComponentType] {
        &self.hidden_components
    }
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
