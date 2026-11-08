//! The values of the game rules. The host owns them; each dimension world reads a copy.

use bevy_ecs::resource::Resource;
use mcrs_minecraft_game_rule::{GameRule, GameRuleValueType};

/// The value of each game rule, by the id of the rule. A bool rule holds 1 for true and 0 for
/// false.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct GameRules(Vec<i32>);

impl Default for GameRules {
    fn default() -> Self {
        Self(
            GameRule::ALL
                .iter()
                .map(|rule| match rule.definition().value {
                    GameRuleValueType::Bool { default } => i32::from(default),
                    GameRuleValueType::Int { default, .. } => default,
                })
                .collect(),
        )
    }
}

impl GameRules {
    /// The value of `rule`.
    #[must_use]
    pub fn get(&self, rule: GameRule) -> i32 {
        self.0
            .get(usize::from(rule.id().number()))
            .copied()
            .unwrap_or_default()
    }

    /// The value of the bool rule `rule`.
    #[must_use]
    pub fn is_on(&self, rule: GameRule) -> bool {
        self.get(rule) != 0
    }

    /// Sets `rule` to `value`.
    pub fn set(&mut self, rule: GameRule, value: i32) {
        if let Some(slot) = self.0.get_mut(usize::from(rule.id().number())) {
            *slot = value;
        }
    }
}
