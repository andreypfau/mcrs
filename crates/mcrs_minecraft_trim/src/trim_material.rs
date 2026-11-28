use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "I: TextSerde", deserialize = "I: TextSerde"))]
pub struct TrimMaterial<I: TextTypes> {
    pub palette_id: ResourceLocationBuf,
    pub description: Text<I>,
}

impl<I: TextTypes> Hash for TrimMaterial<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.palette_id.hash(state);
    }
}
