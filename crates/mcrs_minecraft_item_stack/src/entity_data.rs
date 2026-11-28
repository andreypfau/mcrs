use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::EntityTypeId;
use crate::custom_data::compounds_equal;

pub(crate) const ENTITY_TYPE_KEY: &str = "id";

#[derive(Clone, Debug)]
pub struct EntityData {
    pub entity_type: EntityTypeId,
    data: NbtCompound,
}

impl EntityData {
    #[must_use]
    pub fn new(entity_type: EntityTypeId, mut data: NbtCompound) -> Self {
        data.child_tags.retain(|(key, _)| key != ENTITY_TYPE_KEY);
        Self { entity_type, data }
    }

    #[must_use]
    pub fn data(&self) -> &NbtCompound {
        &self.data
    }
}

impl PartialEq for EntityData {
    fn eq(&self, other: &Self) -> bool {
        self.entity_type == other.entity_type && compounds_equal(&self.data, &other.data)
    }
}

impl Eq for EntityData {}

impl Hash for EntityData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity_type.hash(state);
        self.data.child_tags.len().hash(state);
    }
}
