use crate::item::component::banner::*;
use crate::item::wire::{newtype_wire, record_wire};
use mcrs_minecraft_registries::Wire;

newtype_wire!(BannerPatterns<Wire>);
record_wire!(BannerPattern {
    asset_id,
    translation_key
});
record_wire!(BannerLayer<Wire> { pattern, color });
