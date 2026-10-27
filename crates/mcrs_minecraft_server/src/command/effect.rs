use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::{OperatorBroadcast, OutboundPlayerPacket};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;

/// Where a teleport sends one victim (CMD-TP-004, CMD-TP-005).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Teleport {
    /// The label of the destination dimension.
    pub dimension: Entity,
    pub position: DVec3,
    /// `None` keeps the rotation of the victim.
    pub rotation: Option<(f32, f32)>,
    pub relative_position: [bool; 3],
    pub relative_rotation: [bool; 2],
    pub facing: Option<Facing>,
    /// The anchor of the source, from which the victim looks at its facing target.
    pub anchor: LookAnchor,
}

/// What a victim turns to after its move (CMD-TP-011).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Facing {
    Position(DVec3),
    /// The player with this host anchor, at its feet or its eyes, read after each move.
    Player {
        anchor: Entity,
        at: LookAnchor,
    },
}

/// A change a command makes to a player of any dimension. The host applies the effects of one
/// run in the order they were written, each in the world that holds the player, before the
/// run's messages go out and before the next run starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Teleport { player: Entity, to: Teleport },
    GameMode { player: Entity, mode: GameMode },
}

/// What one run of a handler leaves for the host: its effects, then its messages. The host
/// lends it to the world the handler runs in and empties it after the run.
#[derive(Resource, Debug, Default)]
pub(crate) struct CommandOutbox {
    pub(crate) effects: Vec<Effect>,
    pub(crate) packets: Vec<OutboundPlayerPacket>,
    pub(crate) broadcasts: Vec<OperatorBroadcast>,
}
