#[macro_export]
macro_rules! static_registry {
    ($vis:vis enum $name:ident; $($variant:ident = $location:literal,)+) => {
        $crate::static_registry_core! {
            $vis enum $name;
            $($variant = $location,)+
        }

        impl $name {
            pub fn get(location: ::mcrs_minecraft_resource::ResourceLocation<'_>) -> Option<$name> {
                static INDEX: ::std::sync::LazyLock<
                    ::std::collections::HashMap<::mcrs_minecraft_resource::ResourceLocation<'static>, $name>,
                > = ::std::sync::LazyLock::new(|| {
                    $name::ALL.iter().map(|&entry| (entry.location(), entry)).collect()
                });
                let index: &::std::collections::HashMap<::mcrs_minecraft_resource::ResourceLocation<'_>, $name> = &INDEX;
                index.get(&location).copied()
            }

            pub fn read(text: &str) -> Option<$name> {
                Self::get($crate::__ResourceLocation::read(text).ok()?)
            }
        }

        impl $crate::__wire::Encode for $name {
            fn encode(&self, w: impl ::std::io::Write) -> $crate::__wire::Result<()> {
                $crate::encode_registry_id(*self as u16, w)
            }
        }

        impl $crate::__wire::Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> $crate::__wire::Result<Self> {
                let id = $crate::decode_registry_id(r)?;
                Self::from_protocol_id(id).ok_or_else(|| {
                    $crate::__wire::Error::invalid($crate::unknown_id(
                        <Self as ::mcrs_minecraft_resource::Registered>::REGISTRY.location(),
                        id,
                    ))
                })
            }
        }

        impl $crate::__serde::Serialize for $name {
            fn serialize<S: $crate::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_static_str())
            }
        }

        impl<'de> $crate::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                use $crate::__serde::de::Error as _;
                let text = <std::borrow::Cow<'de, str> as $crate::__serde::Deserialize>::deserialize(deserializer)?;
                let location = $crate::__ResourceLocation::read(&text).map_err(D::Error::custom)?;
                Self::get(location).ok_or_else(|| {
                    D::Error::custom($crate::unknown_entry(
                        <Self as ::mcrs_minecraft_resource::Registered>::REGISTRY.location(),
                        &location,
                    ))
                })
            }
        }
    };
}
