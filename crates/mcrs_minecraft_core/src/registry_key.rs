use crate::resource_location::ResourceLocation;

pub trait RegistryKey: 'static {
    const KEY: ResourceLocation<&'static str>;
}

/// Declares uninhabited registry key types, one `Name = "minecraft:path";` per
/// line, with `, tags "path"` after the key of a registry that has tags.
#[macro_export]
macro_rules! registry_keys {
    ($($(#[$meta:meta])* $name:ident = $key:literal $(, tags $path:literal)?;)*) => {$(
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {}

        impl $crate::registry_key::RegistryKey for $name {
            const KEY: $crate::ResourceLocation<&'static str> = $crate::rl!($key);
        }

        $(impl $crate::tag_key::TaggedRegistry for $name {
            const REGISTRY_PATH: &'static str = $path;
        })?
    )*};
}
