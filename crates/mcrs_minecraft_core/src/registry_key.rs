use crate::resource_location::ResourceLocation;
use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;

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

impl<T: 'static> RegistryKey<T> {
    pub fn binding(self) -> TypeBinding {
        TypeBinding {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            registry: self.location,
        }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeBinding {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub registry: ResourceLocation<&'static str>,
}

pub trait RegistryValue {
    type Registry: 'static;
}
