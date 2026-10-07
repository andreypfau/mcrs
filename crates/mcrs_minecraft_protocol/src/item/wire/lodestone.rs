use crate::item::component::lodestone::*;
use crate::item::wire::record_wire;

record_wire!(
    GlobalPos { dimension, pos },
    LodestoneTracker { target, tracked }
);
