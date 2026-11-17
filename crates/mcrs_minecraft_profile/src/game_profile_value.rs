use core::hash::Hash;
use core::hash::Hasher;

use uuid::Uuid;

use crate::profile_identity::same_groups;
use crate::{PlayerName, Property};

#[derive(Clone, Debug)]
pub struct GameProfileValue {
    pub id: Uuid,
    pub name: PlayerName,
    pub properties: Vec<Property>,
}

impl PartialEq for GameProfileValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && same_groups(&self.properties, &other.properties)
    }
}

impl Eq for GameProfileValue {}

impl Hash for GameProfileValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.properties.len().hash(state);
    }
}
