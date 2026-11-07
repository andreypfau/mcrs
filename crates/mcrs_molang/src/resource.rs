use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::value::{Resource, ResourceKind, ValueType};

/// The elements of an `array.` of a render controller.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceArray {
    Numbers(Vec<f32>),
    Resources(Vec<Resource>),
}

impl ResourceArray {
    pub(crate) fn element_type(&self) -> ValueType {
        match self {
            Self::Numbers(_) => ValueType::Number,
            Self::Resources(_) => ValueType::Resource,
        }
    }

    pub(crate) fn raw(&self) -> Vec<u64> {
        match self {
            Self::Numbers(numbers) => numbers.iter().map(|n| u64::from(n.to_bits())).collect(),
            Self::Resources(resources) => resources.iter().map(|r| r.to_bits()).collect(),
        }
    }
}

/// The resources and arrays that a render controller can name. An expression compiled with no
/// resource set cannot name a resource.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResourceSet {
    names: HashMap<(ResourceKind, Box<str>), u32>,
    arrays: HashMap<Box<str>, ResourceArray>,
}

impl ResourceSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives the short name of a resource an id. Names compare without case.
    pub fn insert(&mut self, kind: ResourceKind, name: &str, id: u32) {
        self.names
            .insert((kind, name.to_ascii_lowercase().into_boxed_str()), id);
    }

    /// Defines `array.<name>`. Names compare without case.
    pub fn insert_array(&mut self, name: &str, array: ResourceArray) {
        self.arrays
            .insert(name.to_ascii_lowercase().into_boxed_str(), array);
    }

    #[must_use]
    pub fn resource(&self, kind: ResourceKind, name: &str) -> Option<Resource> {
        self.names
            .get(&(kind, Box::from(name)))
            .map(|id| Resource { kind, id: *id })
    }

    #[must_use]
    pub fn array(&self, name: &str) -> Option<&ResourceArray> {
        self.arrays.get(name)
    }
}
