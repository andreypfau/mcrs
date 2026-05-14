use crate::world::inventory::PlayerHotbarSlots;
use crate::world::item::ItemStack;
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockSetRequest;
use bevy_app::{App, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::entity::ContainsEntity;
use bevy_ecs::prelude::{On, Query};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::dimension::InDimension;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::ServerboundUseItemOn;
use mcrs_protocol::{BlockStateId, Direction};

const TORCH_ITEM_ID: u16 = 323;
const TORCH_STATE: u16 = 3370;
const WALL_TORCH_STATE_NORTH: u16 = 3371;
const WALL_TORCH_STATE_SOUTH: u16 = 3372;
const WALL_TORCH_STATE_WEST: u16 = 3373;
const WALL_TORCH_STATE_EAST: u16 = 3374;

pub struct PlacingPlugin;

impl Plugin for PlacingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_use_item_on);
    }
}

fn handle_use_item_on(
    event: On<ReceivedPacketEvent>,
    players: Query<(&InDimension, &Reposition, &PlayerHotbarSlots)>,
    items: Query<&ItemStack>,
    mut writer: MessageWriter<BlockSetRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundUseItemOn>() else {
        return;
    };
    let Ok((dim, rep, hotbar)) = players.get(event.entity) else {
        return;
    };
    let Some(slot) = hotbar.get_selected_slot() else {
        return;
    };
    let Ok(stack) = items.get(slot) else {
        return;
    };
    if stack.item_id().0 != TORCH_ITEM_ID {
        return;
    }

    let block_pos = rep.unconvert_block_pos(pkt.block_pos);

    let (place_pos, state_id) = match pkt.face {
        Direction::Up => (
            BlockPos::new(block_pos.x, block_pos.y + 1, block_pos.z),
            TORCH_STATE,
        ),
        Direction::North => (
            BlockPos::new(block_pos.x, block_pos.y, block_pos.z - 1),
            WALL_TORCH_STATE_NORTH,
        ),
        Direction::South => (
            BlockPos::new(block_pos.x, block_pos.y, block_pos.z + 1),
            WALL_TORCH_STATE_SOUTH,
        ),
        Direction::West => (
            BlockPos::new(block_pos.x - 1, block_pos.y, block_pos.z),
            WALL_TORCH_STATE_WEST,
        ),
        Direction::East => (
            BlockPos::new(block_pos.x + 1, block_pos.y, block_pos.z),
            WALL_TORCH_STATE_EAST,
        ),
        Direction::Down => return,
    };

    writer.write(BlockSetRequest {
        dimension: dim.entity(),
        pos: place_pos,
        new_state: BlockStateId(state_id),
        flags: BlockUpdateFlags::all(),
        recursion_left: 512,
    });
}
