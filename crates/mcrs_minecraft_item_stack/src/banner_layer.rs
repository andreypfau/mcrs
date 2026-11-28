use mcrs_minecraft_banner_pattern::BannerPattern;

use crate::BannerPatternId;
use crate::DyeColor;
use crate::RegistryValue;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BannerLayer {
    pub pattern: RegistryValue<BannerPatternId, BannerPattern>,
    pub color: DyeColor,
}
