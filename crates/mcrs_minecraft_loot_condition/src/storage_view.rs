use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_resource::ResourceLocation;

use crate::nbt_number::NbtNumber;

pub trait StorageView {
    fn storage_number(&self, storage: ResourceLocation<'_>, path: &NbtPath) -> Option<NbtNumber>;
}

impl StorageView for NoWorld {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}
