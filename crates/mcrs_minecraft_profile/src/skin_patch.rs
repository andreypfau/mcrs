use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::PlayerModelType;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SkinPatch {
    pub texture: Option<ResourceLocationBuf>,
    pub cape: Option<ResourceLocationBuf>,
    pub elytra: Option<ResourceLocationBuf>,
    pub model: Option<PlayerModelType>,
}
