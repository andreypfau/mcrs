use mcrs_minecraft_resource::ResourceLocation;

macro_rules! builtin_key {
    ($($name:ident => $path:literal),* $(,)?) => {
        /// The component type of a built-in component.
        // The derived order is the order of the patch.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum BuiltinKey {
            $($name,)*
        }

        const ALL: &[BuiltinKey] = &[$(BuiltinKey::$name,)*];

        impl BuiltinKey {
            /// The path of the identifier of the component type. Its namespace is [`NAMESPACE`].
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
    /// The place of the type in the order of the type ids, from 0.
    #[doc(hidden)]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The identifier of the component type.
    pub(crate) fn location(self) -> Option<ResourceLocation<'static>> {
        ResourceLocation::new(NAMESPACE, self.path()).ok()
    }

    pub(crate) fn from_location(location: ResourceLocation<'_>) -> Option<Self> {
        if location.namespace() != NAMESPACE {
            return None;
        }
        ALL.iter()
            .copied()
            .find(|key| key.path() == location.path())
    }
}
