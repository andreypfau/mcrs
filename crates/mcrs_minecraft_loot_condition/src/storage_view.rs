use mcrs_minecraft_predicate::NoWorld;

use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_resource::ResourceLocation;

use crate::nbt_number::NbtNumber;

pub trait StorageView {
    fn storage_number(&self, _storage: ResourceLocation<'_>, _path: &NbtPath) -> Option<NbtNumber> {
        None
    }
}

impl StorageView for NoWorld {}
