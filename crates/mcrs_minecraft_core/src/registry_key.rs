use crate::resource_location::ResourceLocation;

pub trait RegistryKey: 'static {
    const KEY: ResourceLocation<&'static str>;
}

pub trait RegistryValue {
    type Registry: RegistryKey;
}
