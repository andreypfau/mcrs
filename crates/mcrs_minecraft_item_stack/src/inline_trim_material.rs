use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Text;

use crate::identifier::Identifier;

#[derive(Clone, Debug, PartialEq)]
pub struct InlineTrimMaterial {
    palette_id: Identifier,
    pub description: Text,
}

impl InlineTrimMaterial {
    #[must_use]
    pub const fn new(palette_id: ResourceLocationBuf, description: Text) -> Self {
        Self {
            palette_id: Identifier::Owned(palette_id),
            description,
        }
    }

    #[must_use]
    pub fn palette_id(&self) -> ResourceLocation<'_> {
        self.palette_id.as_location()
    }
}

impl Hash for InlineTrimMaterial {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.palette_id.hash(state);
    }
}
