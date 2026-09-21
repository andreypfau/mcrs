use std::io::Write;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::fuel::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::common::resolvable_wire;
use crate::item::wire::record_ctx_wire;
use crate::{Decode, Encode};

resolvable_wire!(ResolvableNumber);

record_ctx_wire! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
