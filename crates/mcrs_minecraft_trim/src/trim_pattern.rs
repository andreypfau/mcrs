use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "I: TextSerde", deserialize = "I: TextSerde"))]
pub struct TrimPattern<I: TextTypes> {
    pub asset_id: ResourceLocationBuf,
    #[serde(default)]
    pub decal: bool,
    pub description: Text<I>,
}

impl<I: TextTypes> Hash for TrimPattern<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_id.hash(state);
        self.decal.hash(state);
    }
}
