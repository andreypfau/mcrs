use mcrs_minecraft_core::{Id, Registered, StaticResourceLocation};
use std::fmt;

pub trait StaticEntry: Registered + Copy {
    fn id(self) -> Id<Self>;

    fn location(self) -> StaticResourceLocation;
}

#[doc(hidden)]
pub use mcrs_minecraft_core::resource_location::ResourceLocation as __ResourceLocation;
#[doc(hidden)]
pub use mcrs_minecraft_core::rl as __rl;

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

            pub const ENTRIES: &'static [::mcrs_minecraft_core::StaticResourceLocation] =
                &[$($crate::static_key::__rl!($location),)+];

            pub const fn id(self) -> ::mcrs_minecraft_core::Id<$name> {
                match ::mcrs_minecraft_core::Id::checked(self as u16, Self::ALL.len()) {
                    Some(id) => id,
                    None => panic!("an entry's discriminant lies outside its own entry list"),
                }
            }

            pub const fn location(self) -> ::mcrs_minecraft_core::StaticResourceLocation {
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
                static INDEX: ::std::sync::LazyLock<
                    ::std::collections::HashMap<&'static str, $name>,
                > = ::std::sync::LazyLock::new(|| {
                    $name::ALL
                        .iter()
                        .map(|&entry| (entry.as_static_str(), entry))
                        .collect()
                });
                INDEX.get(location).copied()
            }

            pub fn read(text: &str) -> Option<$name> {
                let location = $crate::static_key::__ResourceLocation::read(text).ok()?;
                Self::find(location.as_str())
            }
        }

        impl $crate::StaticEntry for $name {
            fn id(self) -> ::mcrs_minecraft_core::Id<$name> {
                $name::id(self)
            }

            fn location(self) -> ::mcrs_minecraft_core::StaticResourceLocation {
                $name::location(self)
            }
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
