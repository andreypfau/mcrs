use bevy_ecs::resource::Resource;

/// What the bridge has shed, kicked and routed since the server started.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeTelemetry {
    pub drop_normal_total: u64,
    pub drop_low_total: u64,
    pub kick_overflow_total: u64,
    pub kick_flood_total: u64,
    pub outbound_messages_consumed_total: u64,
    pub encode_unhandled_total: u64,
    pub outbound_no_queue_total: u64,
}
