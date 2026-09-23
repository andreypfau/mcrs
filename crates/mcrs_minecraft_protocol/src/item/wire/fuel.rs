use crate::item::component::fuel::*;
use crate::item::wire::record_ctx_wire;

record_ctx_wire! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
