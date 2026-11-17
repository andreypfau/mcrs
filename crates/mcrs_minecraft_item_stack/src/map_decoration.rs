use alloc::boxed::Box;
use core::hash::Hash;
use core::hash::Hasher;

use crate::MapDecorationTypeId;
use crate::float_key;

#[derive(Clone, Debug)]
pub struct MapDecoration {
    pub key: Box<str>,
    pub kind: MapDecorationTypeId,
    pub x: f64,
    pub z: f64,
    pub rotation: f32,
}

impl PartialEq for MapDecoration {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.kind == other.kind
            && float_key::of_f64(self.x) == float_key::of_f64(other.x)
            && float_key::of_f64(self.z) == float_key::of_f64(other.z)
            && float_key::of_f32(self.rotation) == float_key::of_f32(other.rotation)
    }
}

impl Eq for MapDecoration {}

impl Hash for MapDecoration {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.kind.hash(state);
        float_key::of_f64(self.x).hash(state);
        float_key::of_f64(self.z).hash(state);
        float_key::of_f32(self.rotation).hash(state);
    }
}
