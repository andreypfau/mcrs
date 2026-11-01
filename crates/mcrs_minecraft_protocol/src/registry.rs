use crate::{Decode, Encode};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use std::borrow::Cow;

#[derive(Clone, Debug, Encode, Decode)]
pub struct Entry<'a> {
    pub id: ResourceLocation<'a>,
    pub data: Option<Cow<'a, NbtTag>>,
}
