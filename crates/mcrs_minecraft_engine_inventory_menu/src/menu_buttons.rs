use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuButtons {
    bits: u128,
}

impl MenuButtons {
    #[must_use]
    pub fn with(self, entry: u16) -> Self {
        Self {
            bits: self.bits | bit(entry),
        }
    }

    #[must_use]
    pub fn contains(&self, entry: u16) -> bool {
        self.bits & bit(entry) != 0
    }
}

fn bit(entry: u16) -> u128 {
    1u128.checked_shl(u32::from(entry)).unwrap_or(0)
}
