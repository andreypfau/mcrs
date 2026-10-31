use crate::ResourceLocation;
use core::fmt;
use core::marker::PhantomData;

pub struct RegistryKey<T> {
    location: ResourceLocation<&'static str>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> RegistryKey<T> {
    pub const fn new(location: ResourceLocation<&'static str>) -> Self {
        RegistryKey {
            location,
            _marker: PhantomData,
        }
    }

    pub const fn location(self) -> ResourceLocation<&'static str> {
        self.location
    }

    pub fn path(self) -> &'static str {
        self.location
            .as_static_str()
            .split_once(':')
            .map_or("", |(_, path)| path)
    }
}

impl<T> Clone for RegistryKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for RegistryKey<T> {}

impl<T> PartialEq for RegistryKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<T> Eq for RegistryKey<T> {}

impl<T> fmt::Debug for RegistryKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryKey({})", self.location)
    }
}

impl<T> fmt::Display for RegistryKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.location, f)
    }
}

pub trait Registered: Sized + 'static {
    const REGISTRY: RegistryKey<Self>;
}

/// The type a registry holds as its values. A registered type holds itself; a value type that a
/// crate above the registered marker defines names that marker.
pub trait RegistryValue {
    type Registry: Registered;
}

impl<T: Registered> RegistryValue for T {
    type Registry = T;
}
