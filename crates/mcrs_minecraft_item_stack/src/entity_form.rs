use alloc::borrow::Cow;

use crate::AttributeModifiers;
use crate::CustomName;
use crate::Damage;
use crate::Food;
use crate::ItemModel;
use crate::ItemName;
use crate::Lore;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::Neutral;
use crate::Rarity;
use crate::RepairCost;
use crate::Tool;
use crate::Unbreakable;

/// What a stack entity needs of a built-in type: the value that it does not store, and a copy of
/// a value of the prototype table that borrows its heap data.
pub(crate) trait EntityForm: Clone + 'static {
    const NEUTRAL: Option<&'static Self> = None;

    fn borrow_static(&'static self) -> Self {
        self.clone()
    }
}

impl EntityForm for MaxStackSize {}

impl EntityForm for MaxDamage {}

impl EntityForm for Damage {}

impl EntityForm for Unbreakable {}

impl EntityForm for Food {}

// chisle: a text is a box, so each stack of a kind with a name owns a copy of it; a text that can
// borrow from the table lifts it.
impl EntityForm for CustomName {}

impl EntityForm for ItemName {}

impl EntityForm for ItemModel {
    fn borrow_static(&'static self) -> Self {
        Self::from_static(self.get())
    }
}

impl EntityForm for Tool {
    fn borrow_static(&'static self) -> Self {
        Self {
            rules: Cow::Borrowed(&self.rules),
            ..*self
        }
    }
}

impl EntityForm for Lore {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);

    fn borrow_static(&'static self) -> Self {
        Self {
            lines: Cow::Borrowed(&self.lines),
        }
    }
}

impl EntityForm for Rarity {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);
}

impl EntityForm for RepairCost {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);
}

impl EntityForm for AttributeModifiers {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);

    fn borrow_static(&'static self) -> Self {
        Self {
            modifiers: Cow::Borrowed(&self.modifiers),
        }
    }
}
