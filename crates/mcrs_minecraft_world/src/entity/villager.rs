use mcrs_minecraft_entity::keys::VillagerProfession;
use mcrs_minecraft_registry::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VillagerData {
    pub kind: Id<mcrs_minecraft_entity::keys::VillagerType>,
    pub profession: VillagerProfession,
    pub level: i32,
}

impl Default for VillagerData {
    fn default() -> Self {
        Self {
            kind: mcrs_minecraft_entity::keys::VillagerType::Plains.id(),
            profession: VillagerProfession::None,
            level: 1,
        }
    }
}
