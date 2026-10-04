use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};

use crate::component::common::{BannerPatternReg, Holder, Registered};
use crate::component::enums::DyeColor;
use crate::harness::Sample;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerPattern {
    pub asset_id: ResourceLocation,
    pub translation_key: String,
}

impl Registered for BannerPattern {
    type Registry = BannerPatternReg;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerLayer {
    pub pattern: Holder<BannerPattern>,
    pub color: DyeColor,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BannerPatterns(pub Vec<BannerLayer>);

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
