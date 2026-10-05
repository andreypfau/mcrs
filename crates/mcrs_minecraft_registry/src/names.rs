use crate::id::id_number;
use crate::registry::RegistryError;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct NameTable {
    registry: ResourceLocation<Arc<str>>,
    names: Vec<ResourceLocation<Arc<str>>>,
    numbers: HashMap<ResourceLocation<Arc<str>>, u16>,
}

impl NameTable {
    pub fn new(
        registry: ResourceLocation<Arc<str>>,
        names: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
    ) -> Result<Self, RegistryError> {
        let names: Vec<_> = names.into_iter().collect();
        let mut numbers = HashMap::with_capacity(names.len());
        for (position, name) in names.iter().enumerate() {
            let number = id_number(position).ok_or_else(|| RegistryError::TooManyEntries {
                registry: registry.clone(),
                len: names.len(),
            })?;
            if numbers.insert(name.clone(), number).is_some() {
                return Err(RegistryError::DuplicateEntry {
                    registry,
                    name: name.clone(),
                });
            }
        }
        Ok(NameTable {
            registry,
            names,
            numbers,
        })
    }

    pub fn registry(&self) -> &ResourceLocation<Arc<str>> {
        &self.registry
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn name(&self, index: usize) -> Option<&ResourceLocation<Arc<str>>> {
        self.names.get(index)
    }

    pub fn number(&self, name: &str) -> Option<u16> {
        self.numbers.get(name).copied()
    }

    pub fn names(&self) -> &[ResourceLocation<Arc<str>>] {
        &self.names
    }
}
