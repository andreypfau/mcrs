#[macro_export]
macro_rules! static_registry_core {
    ($vis:vis enum $name:ident; $($variant:ident = $location:literal,)+) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        #[repr(u16)]
        $vis enum $name {
            $($variant,)+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant,)+];

            pub const ENTRIES: &'static [::mcrs_minecraft_resource::ResourceLocation<'static>] =
                &[$($crate::__rl!($location),)+];

            pub const fn id(self) -> ::mcrs_minecraft_resource::Id<$name> {
                match ::mcrs_minecraft_resource::Id::checked(self as u16, Self::ALL.len()) {
                    Some(id) => id,
                    None => panic!("an entry's discriminant lies outside its own entry list"),
                }
            }

            pub const fn location(self) -> ::mcrs_minecraft_resource::ResourceLocation<'static> {
                match self {
                    $($name::$variant => $crate::__rl!($location),)+
                }
            }

            pub const fn as_static_str(self) -> &'static str {
                match self {
                    $($name::$variant => $location,)+
                }
            }

            pub const fn from_id(id: ::mcrs_minecraft_resource::Id<$name>) -> Option<$name> {
                Self::from_protocol_id(id.number())
            }

            pub const fn from_protocol_id(id: u16) -> Option<$name> {
                match Self::ALL.split_at_checked(id as usize) {
                    Some((_, [entry, ..])) => Some(*entry),
                    _ => None,
                }
            }
        }

        impl $crate::StaticEntry for $name {
            fn id(self) -> ::mcrs_minecraft_resource::Id<$name> {
                $name::id(self)
            }

            fn location(self) -> ::mcrs_minecraft_resource::ResourceLocation<'static> {
                $name::location(self)
            }
        }

        impl From<$name> for ::mcrs_minecraft_resource::Id<$name> {
            fn from(entry: $name) -> Self {
                entry.id()
            }
        }
    };
}
