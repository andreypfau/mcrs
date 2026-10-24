use crate::item::component::trim::*;
use crate::item::wire::{newtype_wire, record_wire};
use mcrs_minecraft_registries::Wire;

newtype_wire!(ProvidesTrimMaterial<Wire>);
record_wire! {
    TrimMaterial<Wire> { palette_id, description },
    TrimPattern<Wire> { asset_id, description, decal },
    Trim<Wire> { material, pattern },
}
