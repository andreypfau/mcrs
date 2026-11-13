use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::observer::On;
use bevy_math::Vec3;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::Hand;
use mcrs_minecraft_protocol_game::serverbound::ServerboundUseItemOn;
use mcrs_voxel::BlockPos;

/// A `use_item_on` packet of the player `entity`. The engine acknowledges its sequence with the
/// block actions of the tick; what the click does is the work of a reader of this message.
#[derive(Message, Clone, Copy, Debug)]
pub struct UseItemOnRequest {
    pub entity: Entity,
    pub hand: Hand,
    pub block_pos: BlockPos,
    pub face: Direction,
    pub cursor: Vec3,
    pub sequence: VarInt,
}

pub(crate) fn handle_use_item_on(
    on: On<Received<ServerboundUseItemOn>>,
    mut writer: MessageWriter<UseItemOnRequest>,
) {
    let Some(p) = on.packet() else {
        return;
    };
    writer.write(UseItemOnRequest {
        entity: on.entity,
        hand: p.hand,
        block_pos: p.block_pos,
        face: p.face,
        cursor: Vec3::new(p.cursor_x, p.cursor_y, p.cursor_z),
        sequence: p.sequence,
    });
}
