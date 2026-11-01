use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::{Deserialize, Serialize};

use crate::{DialogRef, TextSerde, TextTypes};

/// The item a `show_item` hover carries, as the wire holds it: the id and the count are typed,
/// and each data component stays the tag it arrived as, a removal under its `!key`. Code with
/// the item crate maps it to a typed item; code without it keeps it unchanged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemHover {
    pub id: ResourceLocationBuf,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub count: i32,
    #[serde(default, skip_serializing_if = "NbtCompound::is_empty")]
    pub components: NbtCompound,
}

impl TextTypes for ItemHover {
    type EntityKey = ResourceLocationBuf;
    type Dialog = DialogRef;
}

impl TextSerde for ItemHover {}

const fn one() -> i32 {
    1
}

fn is_one(count: &i32) -> bool {
    *count == 1
}
