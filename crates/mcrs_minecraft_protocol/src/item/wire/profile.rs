use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_registry::RegistryLookup;
use uuid::Uuid;

use crate::item::ctx::{DecodeCtx, EncodeCtx};
use mcrs_minecraft_profile::*;

use crate::{Bounded, Decode, Encode};

struct WireProperty<'a> {
    name: Bounded<&'a str, 64>,
    value: Bounded<&'a str, 32767>,
    signature: Option<Bounded<&'a str, 1024>>,
}

impl Encode for WireProperty<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.name.encode(&mut w)?;
        self.value.encode(&mut w)?;
        self.signature.encode(w)
    }
}

impl<'a> Decode<'a> for WireProperty<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(WireProperty {
            name: Bounded::decode(r)?,
            value: Bounded::decode(r)?,
            signature: Option::decode(r)?,
        })
    }
}

fn encode_properties(properties: &[Property], w: impl Write) -> anyhow::Result<()> {
    ensure!(
        properties.len() <= MAX_PROPERTIES,
        "{} properties exceed the maximum of {MAX_PROPERTIES}",
        properties.len()
    );
    let wire: Vec<WireProperty> = properties
        .iter()
        .map(|p| WireProperty {
            name: Bounded(&p.name),
            value: Bounded(&p.value),
            signature: p.signature.as_deref().map(Bounded),
        })
        .collect();
    wire.encode(w)
}

fn decode_properties(r: &mut &[u8]) -> anyhow::Result<Vec<Property>> {
    let wire = Bounded::<Vec<WireProperty>, MAX_PROPERTIES>::decode(r)?.0;
    Ok(grouped_by_name(
        wire.into_iter()
            .map(|p| Property {
                name: p.name.0.into(),
                value: p.value.0.into(),
                signature: p.signature.map(|s| s.0.into()),
            })
            .collect(),
    ))
}

impl EncodeCtx for Profile {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match &self.profile {
            ProfileIdentity::Full(profile) => {
                true.encode(&mut w)?;
                profile.id.encode(&mut w)?;
                profile.name.encode(&mut w)?;
                encode_properties(&profile.properties, &mut w)?;
            }
            ProfileIdentity::Partial {
                name,
                id,
                properties,
            } => {
                false.encode(&mut w)?;
                name.encode(&mut w)?;
                id.encode(&mut w)?;
                encode_properties(properties, &mut w)?;
            }
        }
        self.skin.texture.encode(&mut w)?;
        self.skin.cape.encode(&mut w)?;
        self.skin.elytra.encode(&mut w)?;
        self.skin
            .model
            .map(|model| model == PlayerModelType::Slim)
            .encode(w)
    }
}

impl DecodeCtx<'_> for Profile {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let profile = if bool::decode(r)? {
            ProfileIdentity::Full(GameProfileValue {
                id: Uuid::decode(r)?,
                name: PlayerName::decode(r)?,
                properties: decode_properties(r)?,
            })
        } else {
            ProfileIdentity::Partial {
                name: Option::decode(r)?,
                id: Option::decode(r)?,
                properties: decode_properties(r)?,
            }
        };
        Ok(Profile {
            profile,
            skin: SkinPatch {
                texture: Option::decode(r)?,
                cape: Option::decode(r)?,
                elytra: Option::decode(r)?,
                model: Option::<bool>::decode(r)?.map(|slim| {
                    if slim {
                        PlayerModelType::Slim
                    } else {
                        PlayerModelType::Wide
                    }
                }),
            },
        })
    }
}

impl<S: Encode> Encode for Property<S> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.name.encode(&mut w)?;
        self.value.encode(&mut w)?;
        self.signature.encode(w)
    }
}

impl<'a, S: Decode<'a>> Decode<'a> for Property<S> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Property {
            name: S::decode(r)?,
            value: S::decode(r)?,
            signature: Option::decode(r)?,
        })
    }
}
