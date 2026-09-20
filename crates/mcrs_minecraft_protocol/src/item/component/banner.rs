use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::entity::DyeColor;
use crate::item::component::common::{BannerPatternReg, Holder, Registered};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerPattern {
    pub asset_id: ResourceLocation,
    pub translation_key: String,
}

impl Registered for BannerPattern {
    type Registry = BannerPatternReg;
}

impl EncodeCtx for BannerPattern {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.asset_id.encode(&mut w)?;
        self.translation_key.encode(w)
    }
}

impl DecodeCtx<'_> for BannerPattern {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BannerPattern {
            asset_id: ResourceLocation::decode(r)?,
            translation_key: String::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerLayer {
    pub pattern: Holder<BannerPattern>,
    pub color: DyeColor,
}

impl EncodeCtx for BannerLayer {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.pattern.encode_ctx(ctx, &mut w)?;
        self.color.encode(w)
    }
}

impl DecodeCtx<'_> for BannerLayer {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BannerLayer {
            pattern: Holder::decode_ctx(ctx, r)?,
            color: DyeColor::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BannerPatterns(pub Vec<BannerLayer>);

impl EncodeCtx for BannerPatterns {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BannerPatterns {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(BannerPatterns)
    }
}

impl Sample for BannerPatterns {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            BannerPatterns::default(),
            BannerPatterns(vec![
                BannerLayer {
                    pattern: Holder::reference(ResourceLocation::minecraft("creeper")),
                    color: DyeColor::Red,
                },
                BannerLayer {
                    pattern: Holder::Direct(BannerPattern {
                        asset_id: ResourceLocation::new("mcrs", "x"),
                        translation_key: "block.mcrs.banner.x".into(),
                    }),
                    color: DyeColor::LightBlue,
                },
            ]),
        ]
    }
}
