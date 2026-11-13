use mcrs_engine_muxer::EntityId;

/// A packet that a mob sends its viewers once, besides its state (DMG-NET-001, DMG-NET-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MobSignal {
    /// A full hit, with the network id of its damage type.
    Hurt { damage_type: u32 },
    /// The death: the entity event [`DIED`](mcrs_minecraft_protocol_game::clientbound::ClientboundEntityEvent::DIED).
    Died,
    /// The removal after the death: the entity event
    /// [`REMOVED`](mcrs_minecraft_protocol_game::clientbound::ClientboundEntityEvent::REMOVED).
    Removed,
    /// The pickup of the item entity by the collector with this id, of `amount` items
    /// (IEN-NET-004).
    Taken { collector: EntityId, amount: i32 },
}
