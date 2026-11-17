use mcrs_engine_muxer::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MobSignal {
    Hurt { damage_type: u32 },
    Died,
    Removed,
    Taken { collector: EntityId, amount: i32 },
}
