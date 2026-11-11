use crate::BuiltinKey;

macro_rules! builtin {
    ($($name:ident => $path:literal),* $(,)?) => {
        /// One component of a built-in component type.
        #[derive(Clone, Debug, PartialEq, Hash)]
        #[non_exhaustive]
        pub enum Builtin {
            $($name(crate::$name),)*
        }

        impl Builtin {
            #[doc(hidden)]
            #[must_use]
            pub fn key(&self) -> BuiltinKey {
                match self {
                    $(Self::$name(_) => BuiltinKey::$name,)*
                }
            }
        }
    };
}

crate::for_each_builtin!(builtin);
