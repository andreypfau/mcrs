use crate::resource_location::ResourceLocation;

pub trait RegistryKey: 'static {
    const KEY: ResourceLocation<&'static str>;
}

pub trait RegistryValue {
    type Registry: RegistryKey;
}

/// Declares uninhabited registry key types, one `Name = "minecraft:path";` per line.
#[macro_export]
macro_rules! registry_keys {
    ($($(#[$meta:meta])* $name:ident = $key:literal;)*) => {$(
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {}

        impl $crate::registry_key::RegistryKey for $name {
            const KEY: $crate::ResourceLocation<&'static str> = $crate::rl!($key);
        }
    )*};
}
