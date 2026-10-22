use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::fmt;

pub struct StaticKey<R> {
    id: Id<R>,
    location: ResourceLocation<&'static str>,
}

impl<R> StaticKey<R> {
    #[doc(hidden)]
    pub const fn at(
        entries: &[ResourceLocation<&'static str>],
        position: u16,
        location: ResourceLocation<&'static str>,
    ) -> Option<Self> {
        let Some((_, [entry, ..])) = entries.split_at_checked(position as usize) else {
            return None;
        };
        if same_bytes(
            entry.as_static_str().as_bytes(),
            location.as_static_str().as_bytes(),
        ) && let Some(id) = Id::checked(position, entries.len())
        {
            return Some(StaticKey { id, location });
        }
        None
    }

    pub const fn id(self) -> Id<R> {
        self.id
    }

    pub const fn location(self) -> ResourceLocation<&'static str> {
        self.location
    }

    pub const fn as_static_str(self) -> &'static str {
        self.location.as_static_str()
    }
}

const fn same_bytes(mut left: &[u8], mut right: &[u8]) -> bool {
    while let ([a, left_rest @ ..], [b, right_rest @ ..]) = (left, right) {
        if *a != *b {
            return false;
        }
        left = left_rest;
        right = right_rest;
    }
    left.is_empty() && right.is_empty()
}

impl<R> Clone for StaticKey<R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> Copy for StaticKey<R> {}

impl<R> PartialEq for StaticKey<R> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<R> Eq for StaticKey<R> {}

impl<R> fmt::Debug for StaticKey<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StaticKey({}, {})", self.id.number(), self.location)
    }
}

impl<R> From<StaticKey<R>> for Id<R> {
    fn from(key: StaticKey<R>) -> Self {
        key.id
    }
}

#[doc(hidden)]
pub use mcrs_minecraft_core::resource_location::ResourceLocation as __ResourceLocation;
#[doc(hidden)]
pub use mcrs_minecraft_core::rl as __rl;
#[macro_export]
macro_rules! static_keys {
    ($registry:ty; $($position:literal: $name:ident = $location:literal,)*) => {
        pub const ENTRIES: &[$crate::static_key::StaticLocation] =
            &[$($crate::static_key::__rl!($location)),*];

        // The position is written out rather than searched for: a search per
        // key in const eval is quadratic, a minute of compile time at 2000 keys.
        $(
            pub const $name: $crate::StaticKey<$registry> = match $crate::StaticKey::at(
                ENTRIES,
                $position,
                $crate::static_key::__rl!($location),
            ) {
                Some(key) => key,
                None => panic!("a static key's location is not the entry at its position"),
            };
        )*
    };
}

#[doc(hidden)]
pub type StaticLocation = ResourceLocation<&'static str>;

#[doc(hidden)]
pub fn unknown_entry(registry: impl fmt::Display, location: impl fmt::Display) -> String {
    format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {location}")
}

/// A static registry as an enum of its entries, numbered by protocol id.
#[macro_export]
macro_rules! static_registry {
    ($vis:vis enum $name:ident; $($variant:ident = $location:literal,)+) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        #[repr(u16)]
        $vis enum $name {
            $($variant,)+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant,)+];

            pub const ENTRIES: &'static [$crate::static_key::StaticLocation] =
                &[$($crate::static_key::__rl!($location),)+];

            pub const fn id(self) -> ::mcrs_minecraft_core::Id<$name> {
                match ::mcrs_minecraft_core::Id::checked(self as u16, Self::ALL.len()) {
                    Some(id) => id,
                    None => panic!("an entry's discriminant lies outside its own entry list"),
                }
            }

            pub const fn location(self) -> $crate::static_key::StaticLocation {
                match self {
                    $($name::$variant => $crate::static_key::__rl!($location),)+
                }
            }

            pub const fn as_static_str(self) -> &'static str {
                self.location().as_static_str()
            }

            pub const fn from_id(id: ::mcrs_minecraft_core::Id<$name>) -> Option<$name> {
                Self::from_protocol_id(id.number())
            }

            pub const fn from_protocol_id(id: u16) -> Option<$name> {
                match Self::ALL.split_at_checked(id as usize) {
                    Some((_, [entry, ..])) => Some(*entry),
                    _ => None,
                }
            }

            pub fn find(location: &str) -> Option<$name> {
                Self::ALL
                    .iter()
                    .find(|entry| entry.as_static_str() == location)
                    .copied()
            }

            pub fn read(text: &str) -> Option<$name> {
                let location = $crate::static_key::__ResourceLocation::read(text).ok()?;
                Self::find(location.as_str())
            }
        }

        impl $crate::RegistryType for $name {
            type Value = ();
        }

        impl From<$name> for ::mcrs_minecraft_core::Id<$name> {
            fn from(entry: $name) -> Self {
                entry.id()
            }
        }

        impl $crate::__serde::Serialize for $name {
            fn serialize<S: $crate::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_static_str())
            }
        }

        impl<'de> $crate::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                use $crate::__serde::de::Error as _;
                let text = <std::borrow::Cow<'de, str> as $crate::__serde::Deserialize>::deserialize(deserializer)?;
                let location = $crate::static_key::__ResourceLocation::read(&text).map_err(D::Error::custom)?;
                Self::find(location.as_str()).ok_or_else(|| {
                    D::Error::custom($crate::static_key::unknown_entry(
                        <Self as ::mcrs_minecraft_core::Registered>::REGISTRY.location(),
                        &location,
                    ))
                })
            }
        }
    };
}
