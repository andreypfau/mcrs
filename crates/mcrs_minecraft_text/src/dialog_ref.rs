use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::ResourceLocation;
use serde::{Deserialize, Serialize};

/// A registry id, or the dialog written inline.
// chisle: an inline dialog is carried as its compound and is not validated; text types that know
// the dialog value name it as their `Dialog` instead.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DialogRef {
    Reference(ResourceLocation),
    Inline(NbtCompound),
}
