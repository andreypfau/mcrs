use crate::item::component::fuel::*;
use crate::item::wire::record_wire;

record_wire! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
