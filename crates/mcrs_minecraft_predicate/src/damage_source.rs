use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_resource::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageSource<E> {
    pub damage_type: Id<DamageType>,
    pub direct_entity: Option<E>,
    pub causing_entity: Option<E>,
}

impl<E: PartialEq> DamageSource<E> {
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.direct_entity == self.causing_entity
    }
}
