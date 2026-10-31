use mcrs_minecraft_resource::{Id, Registered, ResourceLocation};
use std::fmt;

pub trait StaticEntry: Registered + Copy {
    fn id(self) -> Id<Self>;

    fn location(self) -> ResourceLocation<'static>;
}

#[doc(hidden)]
pub use mcrs_minecraft_resource::ResourceLocation as __ResourceLocation;
#[doc(hidden)]
pub use mcrs_minecraft_resource::rl as __rl;

#[doc(hidden)]
pub fn unknown_entry(registry: impl fmt::Display, location: impl fmt::Display) -> String {
    format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {location}")
}

#[doc(hidden)]
pub fn unknown_id(registry: impl fmt::Display, id: u16) -> String {
    format!("registry {registry} has no id {id}")
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

            pub const ENTRIES: &'static [::mcrs_minecraft_resource::ResourceLocation<'static>] =
                &[$($crate::static_key::__rl!($location),)+];

            pub const fn id(self) -> ::mcrs_minecraft_resource::Id<$name> {
                match ::mcrs_minecraft_resource::Id::checked(self as u16, Self::ALL.len()) {
                    Some(id) => id,
                    None => panic!("an entry's discriminant lies outside its own entry list"),
                }
            }

            pub const fn location(self) -> ::mcrs_minecraft_resource::ResourceLocation<'static> {
                match self {
                    $($name::$variant => $crate::static_key::__rl!($location),)+
                }
            }

            pub const fn as_static_str(self) -> &'static str {
                match self {
                    $($name::$variant => $location,)+
                }
            }

            pub const fn from_id(id: ::mcrs_minecraft_resource::Id<$name>) -> Option<$name> {
                Self::from_protocol_id(id.number())
            }

            pub const fn from_protocol_id(id: u16) -> Option<$name> {
                match Self::ALL.split_at_checked(id as usize) {
                    Some((_, [entry, ..])) => Some(*entry),
                    _ => None,
                }
            }

            /// The entry that `location` names.
            pub fn get(location: ::mcrs_minecraft_resource::ResourceLocation<'_>) -> Option<$name> {
                static INDEX: ::std::sync::LazyLock<
                    ::std::collections::HashMap<::mcrs_minecraft_resource::ResourceLocation<'static>, $name>,
                > = ::std::sync::LazyLock::new(|| {
                    $name::ALL.iter().map(|&entry| (entry.location(), entry)).collect()
                });
                let index: &::std::collections::HashMap<::mcrs_minecraft_resource::ResourceLocation<'_>, $name> = &INDEX;
                index.get(&location).copied()
            }

            /// The entry that `text` names; a bare path is in the `minecraft` namespace.
            pub fn read(text: &str) -> Option<$name> {
                Self::get($crate::static_key::__ResourceLocation::read(text).ok()?)
            }
        }

        impl $crate::StaticEntry for $name {
            fn id(self) -> ::mcrs_minecraft_resource::Id<$name> {
                $name::id(self)
            }

            fn location(self) -> ::mcrs_minecraft_resource::ResourceLocation<'static> {
                $name::location(self)
            }
        }

        impl From<$name> for ::mcrs_minecraft_resource::Id<$name> {
            fn from(entry: $name) -> Self {
                entry.id()
            }
        }

        impl $crate::__wire::Encode for $name {
            fn encode(&self, w: impl ::std::io::Write) -> $crate::__wire::Result<()> {
                $crate::encode_registry_id(*self as u16, w)
            }
        }

        impl $crate::__wire::Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> $crate::__wire::Result<Self> {
                let id = $crate::decode_registry_id(r)?;
                Self::from_protocol_id(id).ok_or_else(|| {
                    $crate::__wire::Error::invalid($crate::static_key::unknown_id(
                        <Self as ::mcrs_minecraft_resource::Registered>::REGISTRY.location(),
                        id,
                    ))
                })
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
                Self::get(location).ok_or_else(|| {
                    D::Error::custom($crate::static_key::unknown_entry(
                        <Self as ::mcrs_minecraft_resource::Registered>::REGISTRY.location(),
                        &location,
                    ))
                })
            }
        }
    };
}
