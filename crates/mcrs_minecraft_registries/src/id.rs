use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// chisle: an id is a u16, so a registry holds at most 65 536 entries; widening `Id` lifts it.
pub(crate) const MAX_ENTRIES: usize = u16::MAX as usize + 1;

pub struct Id<R> {
    number: u16,
    registry: PhantomData<fn() -> R>,
}

impl<R> Id<R> {
    pub(crate) fn new(number: u16) -> Self {
        Self {
            number,
            registry: PhantomData,
        }
    }

    pub fn number(self) -> u16 {
        self.number
    }

    pub fn index(self) -> usize {
        usize::from(self.number)
    }
}

pub struct TagId<R> {
    number: u32,
    registry: PhantomData<fn() -> R>,
}

impl<R> TagId<R> {
    pub(crate) fn new(number: u32) -> Self {
        Self {
            number,
            registry: PhantomData,
        }
    }

    pub fn index(self) -> usize {
        self.number as usize
    }
}

macro_rules! plain_handle {
    ($name:ident) => {
        impl<R> Clone for $name<R> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<R> Copy for $name<R> {}

        impl<R> PartialEq for $name<R> {
            fn eq(&self, other: &Self) -> bool {
                self.number == other.number
            }
        }

        impl<R> Eq for $name<R> {}

        impl<R> PartialOrd for $name<R> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<R> Ord for $name<R> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.number.cmp(&other.number)
            }
        }

        impl<R> Hash for $name<R> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.number.hash(state);
            }
        }

        impl<R> fmt::Debug for $name<R> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.number)
            }
        }
    };
}

plain_handle!(Id);
plain_handle!(TagId);

pub struct Key<R> {
    location: ResourceLocation,
    registry: PhantomData<fn() -> R>,
}

impl<R> Key<R> {
    pub fn new(location: ResourceLocation) -> Self {
        Self {
            location,
            registry: PhantomData,
        }
    }

    pub fn location(&self) -> &ResourceLocation {
        &self.location
    }
}

impl<R> Clone for Key<R> {
    fn clone(&self) -> Self {
        Self::new(self.location.clone())
    }
}

impl<R> PartialEq for Key<R> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<R> Eq for Key<R> {}

impl<R> Hash for Key<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl<R> fmt::Debug for Key<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key({})", self.location)
    }
}

impl<'de, R> Deserialize<'de> for Key<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ResourceLocation::deserialize(deserializer).map(Self::new)
    }
}

impl<R> Serialize for Key<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.location.serialize(serializer)
    }
}
