use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Holder, Registered, TrimMaterialReg, TrimPatternReg};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::text::Text;
use crate::{Decode, Encode};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimMaterial {
    pub palette_id: ResourceLocation,
    pub description: Text,
}

impl Registered for TrimMaterial {
    type Registry = TrimMaterialReg;
}

impl EncodeCtx for TrimMaterial {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.palette_id.encode(&mut w)?;
        self.description.encode(w)
    }
}

impl DecodeCtx<'_> for TrimMaterial {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrimMaterial {
            palette_id: ResourceLocation::decode(r)?,
            description: Text::decode(r)?,
        })
    }
}

/// `decal`: absent reads as false, always written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimPattern {
    pub asset_id: ResourceLocation,
    pub description: Text,
    #[serde(default)]
    pub decal: bool,
}

impl Registered for TrimPattern {
    type Registry = TrimPatternReg;
}

impl EncodeCtx for TrimPattern {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.asset_id.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.decal.encode(w)
    }
}

impl DecodeCtx<'_> for TrimPattern {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrimPattern {
            asset_id: ResourceLocation::decode(r)?,
            description: Text::decode(r)?,
            decal: bool::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trim {
    pub material: Holder<TrimMaterial>,
    pub pattern: Holder<TrimPattern>,
}

impl EncodeCtx for Trim {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.material.encode_ctx(ctx, &mut w)?;
        self.pattern.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Trim {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Trim {
            material: Holder::decode_ctx(ctx, r)?,
            pattern: Holder::decode_ctx(ctx, r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvidesTrimMaterial(pub Holder<TrimMaterial>);

impl EncodeCtx for ProvidesTrimMaterial {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for ProvidesTrimMaterial {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(ProvidesTrimMaterial)
    }
}

fn amethyst() -> Holder<TrimMaterial> {
    Holder::reference(ResourceLocation::minecraft("amethyst"))
}

fn pal() -> Holder<TrimMaterial> {
    Holder::Direct(TrimMaterial {
        palette_id: ResourceLocation::new("mcrs", "pal"),
        description: Text::text("Pal"),
    })
}

impl Sample for Trim {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        tags.push(match self.material {
            Holder::Reference(_) => ("material", STRING_ID),
            Holder::Direct(_) => ("material", COMPOUND_ID),
        });
        match self.pattern {
            Holder::Reference(_) => tags.push(("pattern", STRING_ID)),
            Holder::Direct(_) => {
                tags.extend([("pattern", COMPOUND_ID), ("pattern.decal", BYTE_ID)])
            }
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Trim {
                material: amethyst(),
                pattern: Holder::reference(ResourceLocation::minecraft("coast")),
            },
            Trim {
                material: pal(),
                pattern: Holder::Direct(TrimPattern {
                    asset_id: ResourceLocation::new("mcrs", "pat"),
                    description: Text::translate("trim.mcrs.pat", Vec::new()),
                    decal: true,
                }),
            },
            Trim {
                material: amethyst(),
                pattern: Holder::Direct(TrimPattern {
                    asset_id: ResourceLocation::new("mcrs", "pat"),
                    description: Text::text("Pat"),
                    decal: false,
                }),
            },
        ]
    }
}

impl Sample for ProvidesTrimMaterial {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, STRING_ID};
        match self.0 {
            Holder::Reference(_) => vec![("", STRING_ID)],
            Holder::Direct(_) => vec![("", COMPOUND_ID), ("palette_id", STRING_ID)],
        }
    }

    fn samples() -> Vec<Self> {
        vec![
            ProvidesTrimMaterial(amethyst()),
            ProvidesTrimMaterial(pal()),
        ]
    }
}
