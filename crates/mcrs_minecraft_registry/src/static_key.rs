use crate::id::Id;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::fmt;

pub struct StaticKey<R> {
    id: Id<R>,
    location: ResourceLocation<&'static str>,
}

impl<R> StaticKey<R> {
    #[doc(hidden)]
    pub const fn new(number: u16, location: ResourceLocation<&'static str>) -> Self {
        StaticKey {
            id: Id::from_number(number),
            location,
        }
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
#[doc(hidden)]
pub use serde as __serde;

#[macro_export]
macro_rules! static_keys {
    ($registry:ty; $($name:ident = $location:literal,)*) => {
        #[allow(non_camel_case_types, clippy::upper_case_acronyms, dead_code)]
        #[repr(u16)]
        enum Position {
            $($name,)*
        }

        $(
            pub const $name: $crate::StaticKey<$registry> =
                $crate::StaticKey::new(Position::$name as u16, $crate::static_key::__rl!($location));
        )*

        pub const ENTRIES: &[$crate::static_key::StaticLocation] =
            &[$($crate::static_key::__rl!($location)),*];

        pub const fn at(id: $crate::Id<$registry>) -> Option<$crate::StaticKey<$registry>> {
            if id.index() < ENTRIES.len() {
                Some($crate::StaticKey::new(id.number(), ENTRIES[id.index()]))
            } else {
                None
            }
        }

        pub fn find(location: &str) -> Option<$crate::StaticKey<$registry>> {
            ENTRIES
                .iter()
                .position(|entry| entry.as_static_str() == location)
                .map(|position| $crate::StaticKey::new(position as u16, ENTRIES[position]))
        }
    };
}

#[doc(hidden)]
pub type StaticLocation = ResourceLocation<&'static str>;

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

            pub const fn id(self) -> $crate::Id<$name> {
                $crate::Id::from_static_position(self as u16)
            }

            pub const fn location(self) -> $crate::static_key::StaticLocation {
                Self::ENTRIES[self as usize]
            }

            pub const fn as_static_str(self) -> &'static str {
                self.location().as_static_str()
            }

            pub const fn from_id(id: $crate::Id<$name>) -> Option<$name> {
                if id.index() < Self::ALL.len() {
                    Some(Self::ALL[id.index()])
                } else {
                    None
                }
            }

            pub fn find(location: &str) -> Option<$name> {
                Self::ENTRIES
                    .iter()
                    .position(|entry| entry.as_static_str() == location)
                    .map(|position| Self::ALL[position])
            }
        }

        impl From<$name> for $crate::Id<$name> {
            fn from(entry: $name) -> Self {
                entry.id()
            }
        }

        impl $crate::static_key::__serde::Serialize for $name {
            fn serialize<S: $crate::static_key::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_static_str())
            }
        }

        impl<'de> $crate::static_key::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::static_key::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                use $crate::static_key::__serde::de::Error as _;
                let text = <std::borrow::Cow<'de, str> as $crate::static_key::__serde::Deserialize>::deserialize(deserializer)?;
                let location = $crate::static_key::__ResourceLocation::read(&text).map_err(D::Error::custom)?;
                Self::find(location.as_str()).ok_or_else(|| {
                    D::Error::custom(format_args!("{location} is no {}", stringify!($name)))
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::id::Id;

    crate::static_registry! {
        pub enum Fruit;
        Zebra = "minecraft:zebra",
        Apple = "minecraft:apple",
    }

    #[test]
    fn an_entry_is_numbered_by_its_position_and_names_its_location() {
        assert_eq!(Fruit::Zebra.id().number(), 0);
        assert_eq!(Fruit::Apple.id().number(), 1);
        assert_eq!(Fruit::Apple.as_static_str(), "minecraft:apple");
        assert_eq!(
            Fruit::ENTRIES[Fruit::Apple.id().index()],
            Fruit::Apple.location()
        );
    }

    #[test]
    fn an_entry_is_found_by_id_and_by_location() {
        assert_eq!(Fruit::from_id(Fruit::Apple.id()), Some(Fruit::Apple));
        assert_eq!(Fruit::find("minecraft:zebra"), Some(Fruit::Zebra));
        assert_eq!(Fruit::find("minecraft:mango"), None);
        let beyond: Id<Fruit> = crate::bitset::DenseId::from_raw(2);
        assert_eq!(Fruit::from_id(beyond), None);
    }

    #[test]
    fn an_entry_reads_and_writes_as_its_location() {
        assert_eq!(
            serde_json::to_string(&Fruit::Apple).unwrap(),
            r#""minecraft:apple""#
        );
        let cases = [
            (r#""minecraft:apple""#, Ok(Fruit::Apple)),
            (r#""zebra""#, Ok(Fruit::Zebra)),
            (r#""minecraft:mango""#, Err("minecraft:mango is no Fruit")),
        ];
        for (json, expected) in cases {
            let read = serde_json::from_str::<Fruit>(json).map_err(|error| error.to_string());
            match expected {
                Ok(fruit) => assert_eq!(read, Ok(fruit)),
                Err(message) => assert!(read.unwrap_err().contains(message)),
            }
        }
    }

    struct Fixed;

    mod fixed {
        crate::static_keys! {
            super::Fixed;
            AIR = "minecraft:air",
            STONE = "minecraft:stone",
        }
    }

    #[test]
    fn a_key_is_numbered_by_its_position_and_names_its_location() {
        assert_eq!(fixed::AIR.id().number(), 0);
        assert_eq!(fixed::STONE.id().number(), 1);
        assert_eq!(fixed::STONE.as_static_str(), "minecraft:stone");
        assert_eq!(
            fixed::ENTRIES[fixed::STONE.id().index()],
            fixed::STONE.location()
        );
    }

    #[test]
    fn a_key_is_found_by_id_and_by_location() {
        assert_eq!(fixed::at(fixed::STONE.id()), Some(fixed::STONE));
        assert_eq!(fixed::find("minecraft:air"), Some(fixed::AIR));
        assert_eq!(fixed::find("minecraft:dirt"), None);
        let beyond: Id<Fixed> = crate::bitset::DenseId::from_raw(2);
        assert_eq!(fixed::at(beyond), None);
    }
}
