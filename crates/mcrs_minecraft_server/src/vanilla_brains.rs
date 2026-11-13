use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_goals::ChickenBrain;
use mcrs_minecraft_goals::CowBrain;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::SheepBrain;
use mcrs_minecraft_goals::VanillaBrain;

/// The brain of each mob type with goals (GOAL-ANI-001 to GOAL-ANI-004).
pub(crate) fn vanilla_brain(kind: &'static EntityTypeData) -> Option<VanillaBrain> {
    match kind.entity_type {
        EntityType::Pig => Some(VanillaBrain::Pig(PigBrain::default())),
        EntityType::Cow => Some(VanillaBrain::Cow(CowBrain::default())),
        EntityType::Sheep => Some(VanillaBrain::Sheep(SheepBrain::default())),
        EntityType::Chicken => Some(VanillaBrain::Chicken(ChickenBrain::default())),
        _ => None,
    }
}
