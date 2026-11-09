use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::VanillaBrain;

/// The brain of each mob type with goals (GOAL-ANI-001).
pub(crate) fn vanilla_brain(kind: &'static EntityTypeData) -> Option<VanillaBrain> {
    match kind.entity_type {
        EntityType::Pig => Some(VanillaBrain::Pig(PigBrain::default())),
        _ => None,
    }
}
