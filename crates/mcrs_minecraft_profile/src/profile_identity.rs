use core::hash::Hash;
use core::hash::Hasher;

use uuid::Uuid;

use crate::{GameProfileValue, PlayerName, Property};

#[derive(Clone, Debug)]
pub enum ProfileIdentity {
    Full(GameProfileValue),
    Partial {
        name: Option<PlayerName>,
        id: Option<Uuid>,
        properties: Vec<Property>,
    },
}

impl PartialEq for ProfileIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Full(left), Self::Full(right)) => left == right,
            (
                Self::Partial {
                    name,
                    id,
                    properties,
                },
                Self::Partial {
                    name: other_name,
                    id: other_id,
                    properties: other_properties,
                },
            ) => name == other_name && id == other_id && same_groups(properties, other_properties),
            _ => false,
        }
    }
}

impl Eq for ProfileIdentity {}

impl Hash for ProfileIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Full(profile) => profile.hash(state),
            Self::Partial {
                name,
                id,
                properties,
            } => {
                name.hash(state);
                id.hash(state);
                properties.len().hash(state);
            }
        }
    }
}

pub(crate) fn same_groups(left: &[Property], right: &[Property]) -> bool {
    left.len() == right.len()
        && left.iter().all(|property| {
            let named = |other: &&Property| other.name == property.name;
            left.iter().filter(named).eq(right.iter().filter(named))
        })
}
