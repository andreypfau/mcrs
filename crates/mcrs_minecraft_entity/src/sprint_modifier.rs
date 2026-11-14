use mcrs_minecraft_entity_attribute::AttributeModifier;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_resource::rl;

/// The modifier of `movement_speed` that a sprinting player has.
pub const SPRINT_MODIFIER: AttributeModifier<'static> = AttributeModifier {
    id: rl!("minecraft:sprinting"),
    amount: 0.3_f32 as f64,
    operation: ModifierOperation::AddMultipliedTotal,
};
