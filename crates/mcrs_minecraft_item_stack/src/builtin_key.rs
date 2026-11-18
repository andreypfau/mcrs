use mcrs_minecraft_resource::ResourceLocation;

macro_rules! builtin_key {
    ($($name:ident => $path:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum BuiltinKey {
            $($name,)*
        }

        const ALL: &[BuiltinKey] = &[$(BuiltinKey::$name,)*];

        impl BuiltinKey {
            pub(crate) const fn path(self) -> &'static str {
                match self {
                    $(Self::$name => $path,)*
                }
            }
        }
    };
}

crate::for_each_builtin!(builtin_key);

pub(crate) const NAMESPACE: &str = "minecraft";

impl BuiltinKey {
    #[doc(hidden)]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn location(self) -> Option<ResourceLocation<'static>> {
        ResourceLocation::new(NAMESPACE, self.path()).ok()
    }

    #[must_use]
    pub fn from_location(location: ResourceLocation<'_>) -> Option<Self> {
        if location.namespace() != NAMESPACE {
            return None;
        }
        ALL.iter()
            .copied()
            .find(|key| key.path() == location.path())
    }
}
