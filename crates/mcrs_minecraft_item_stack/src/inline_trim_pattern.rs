use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Text;

use crate::identifier::Identifier;

#[derive(Clone, Debug, PartialEq)]
pub struct InlineTrimPattern {
    asset_id: Identifier,
    pub description: Text,
    pub decal: bool,
}

impl InlineTrimPattern {
    #[must_use]
    pub const fn new(asset_id: ResourceLocationBuf, description: Text) -> Self {
        Self {
            asset_id: Identifier::Owned(asset_id),
            description,
            decal: false,
        }
    }

    #[must_use]
    pub fn asset_id(&self) -> ResourceLocation<'_> {
        self.asset_id.as_location()
    }
}

impl Hash for InlineTrimPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_id.hash(state);
        self.decal.hash(state);
    }
}
