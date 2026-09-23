use crate::item::component::trim::*;
use crate::item::wire::{newtype_ctx_wire, record_ctx_wire};

newtype_ctx_wire!(ProvidesTrimMaterial);
record_ctx_wire! {
    TrimMaterial { palette_id, description },
    TrimPattern { asset_id, description, decal },
    Trim { material, pattern },
}
