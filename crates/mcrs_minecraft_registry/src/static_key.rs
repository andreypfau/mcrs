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
pub use mcrs_minecraft_core::rl as __rl;

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

#[cfg(test)]
mod tests {
    use crate::id::Id;

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
