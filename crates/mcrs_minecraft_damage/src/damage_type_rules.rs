use alloc::vec::Vec;

use crate::DamageTags;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct DamageTypeRules {
    rows: Vec<DamageTags>,
}

impl DamageTypeRules {
    #[must_use]
    pub fn new(rows: impl IntoIterator<Item = DamageTags>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn tags(&self, damage_type: u32) -> DamageTags {
        usize::try_from(damage_type)
            .ok()
            .and_then(|index| self.rows.get(index))
            .copied()
            .unwrap_or_default()
    }
}
