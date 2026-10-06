#[rustfmt::skip]
pub mod keys;
pub mod damage;
pub mod entity;
pub mod location;
pub mod player;
pub mod slots;

pub use damage::{DamageSourcePredicate, TagPredicate};
pub use entity::EntityPredicate;
pub use location::LocationPredicate;

/// A map whose key selects both the field and the type of its value, the way
/// `Codec.dispatchedMap` does. A key reads with or without its namespace; an
/// unknown key is an error naming it, so a malformed pack fails at load.
#[macro_export]
macro_rules! dispatched_map {
    (
        $(#[$meta:meta])*
        $name:ident { $($key:literal => $field:ident : $ty:ty),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct $name {
            $(pub $field: Option<$ty>,)+
        }

        impl $name {
            pub const KEYS: &'static [&'static str] = &[$($key),+];

            pub fn is_empty(&self) -> bool {
                true $(&& self.$field.is_none())+
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;

                impl<'de> ::serde::de::Visitor<'de> for V {
                    type Value = $name;

                    fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " map"))
                    }

                    fn visit_map<A: ::serde::de::MapAccess<'de>>(
                        self,
                        mut map: A,
                    ) -> Result<$name, A::Error> {
                        use ::serde::de::Error as _;
                        let mut out = <$name>::default();
                        while let Some(key) = map.next_key::<String>()? {
                            let id = ::mcrs_minecraft_core::ResourceLocation::read(&key)
                                .map_err(A::Error::custom)?;
                            match id.as_str() {
                                $($key => {
                                    if out.$field.is_some() {
                                        return Err(A::Error::custom(format_args!(
                                            "Duplicate key '{}'",
                                            $key
                                        )));
                                    }
                                    out.$field = Some(map.next_value().map_err(|e| {
                                        A::Error::custom(format_args!("{}: {e}", $key))
                                    })?);
                                })+
                                unknown => {
                                    return Err(A::Error::custom(format_args!(
                                        concat!("unknown ", stringify!($name), " entry `{}`"),
                                        unknown
                                    )));
                                }
                            }
                        }
                        Ok(out)
                    }
                }

                d.deserialize_map(V)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use ::serde::ser::SerializeMap as _;
                let len = 0usize $(+ self.$field.is_some() as usize)+;
                let mut map = s.serialize_map(Some(len))?;
                $(if let Some(value) = &self.$field {
                    map.serialize_entry($key, value)?;
                })+
                map.end()
            }
        }
    };
}

pub(crate) fn is_any_int(
    bounds: &mcrs_minecraft_item::component::common::MinMaxBounds<i32>,
) -> bool {
    bounds.is_any()
}

pub(crate) fn is_any_double(
    bounds: &mcrs_minecraft_item::component::common::MinMaxBounds<f64>,
) -> bool {
    bounds.is_any()
}
