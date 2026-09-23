use crate::item::component::banner::*;
use crate::item::wire::{newtype_ctx_wire, record_ctx_wire};

newtype_ctx_wire!(BannerPatterns);
record_ctx_wire! {
    BannerPattern { asset_id, translation_key },
    BannerLayer { pattern, color },
}
