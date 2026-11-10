use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Debug)]
pub(crate) enum Identifier {
    Borrowed(ResourceLocation<'static>),
    Owned(ResourceLocationBuf),
}

impl Identifier {
    pub(crate) fn as_location(&self) -> ResourceLocation<'_> {
        match self {
            Self::Borrowed(location) => *location,
            Self::Owned(location) => location.as_location(),
        }
    }
}

impl PartialEq for Identifier {
    fn eq(&self, other: &Self) -> bool {
        self.as_location() == other.as_location()
    }
}

impl Eq for Identifier {}

impl Hash for Identifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_location().hash(state);
    }
}
