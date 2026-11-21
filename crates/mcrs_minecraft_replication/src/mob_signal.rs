use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MobSignal {
    Hurt {
        damage_type: u32,
    },
    Died,
    Removed,
    Taken {
        collector: EntityId,
        amount: i32,
    },
    Swing {
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
    },
}
