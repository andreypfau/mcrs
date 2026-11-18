use mcrs_minecraft_model::stack_view::StackView;

use crate::durability::Durability;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StackOverlay {
    pub count: u8,
    pub durability: Option<Durability>,
}

impl StackOverlay {
    #[must_use]
    pub fn of(view: StackView<'_>) -> Self {
        Self {
            count: view.stack.count.get(),
            durability: view.is_damaged().then(|| Durability {
                damage: view.damage_value(),
                max_damage: view.max_damage(),
            }),
        }
    }
}
