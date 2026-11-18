use alloc::string::String;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::identifier::Identifier;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlineBannerPattern {
    asset_id: Identifier,
    pub translation_key: String,
}

impl InlineBannerPattern {
    #[must_use]
    pub const fn new(asset_id: ResourceLocationBuf, translation_key: String) -> Self {
        Self {
            asset_id: Identifier::Owned(asset_id),
            translation_key,
        }
    }

    #[must_use]
    pub fn asset_id(&self) -> ResourceLocation<'_> {
        self.asset_id.as_location()
    }
}
