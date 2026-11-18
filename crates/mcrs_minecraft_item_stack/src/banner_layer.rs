use crate::BannerPatternId;
use crate::DyeColor;
use crate::InlineBannerPattern;
use crate::RegistryValue;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BannerLayer {
    pub pattern: RegistryValue<BannerPatternId, InlineBannerPattern>,
    pub color: DyeColor,
}
