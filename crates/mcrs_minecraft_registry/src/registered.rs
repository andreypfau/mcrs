use mcrs_minecraft_core::RegistryKey;

pub trait Registered: Sized + 'static {
    const REGISTRY: RegistryKey<Self>;
}
