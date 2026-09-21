use crate::world::inventory::held_stack;
use crate::world::item::chest::OpenContainerRequest;
use bevy_app::{App, Plugin};
use bevy_ecs::entity::{ContainsEntity, Entity};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{On, Query, Res, With};
use mcrs_minecraft_item::{ItemStack, Items, SelectedHotbarSlot, SlotTable};
use mcrs_minecraft_level::block::BlockUpdateFlags;
use mcrs_minecraft_level::block_update::BlockSetRequest;
use mcrs_minecraft_level::entity::player::reposition::Reposition;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::block_entity::BlockEntityPos;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundUseItemOn;

pub struct PlacingPlugin;

impl Plugin for PlacingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_use_item_on);
    }
}

// ponytail: places the item's default block state on the clicked face; no
// facing/waterlogged/replaceable resolution and no survival count decrement
// yet.
fn handle_use_item_on(
    event: On<ReceivedPacketEvent>,
    players: Query<(&InDimension, &Reposition, &SlotTable, &SelectedHotbarSlot)>,
    stacks: Query<&ItemStack>,
    items: Res<Items>,
    containers: Query<(Entity, &BlockEntityPos, &InDimension), With<SlotTable>>,
    mut writer: MessageWriter<BlockSetRequest>,
    mut open: MessageWriter<OpenContainerRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundUseItemOn>() else {
        return;
    };
    let Ok((dim, rep, table, selected)) = players.get(event.entity) else {
        return;
    };
    let clicked = rep.unconvert_block_pos(pkt.block_pos);
    if let Some((container, _, _)) = containers
        .iter()
        .find(|(_, at, in_dim)| at.0 == clicked && in_dim.0 == dim.0)
    {
        open.write(OpenContainerRequest {
            player: event.entity,
            container,
        });
        return;
    }
    let Some(stack) = held_stack(table, selected).and_then(|held| stacks.get(held).ok()) else {
        return;
    };
    let Some(state) = items.get(stack.item()).and_then(|entry| entry.block_placer) else {
        return;
    };
    writer.write(BlockSetRequest {
        dimension: dim.entity(),
        pos: clicked + pkt.face.normal(),
        new_state: state.into(),
        flags: BlockUpdateFlags::all(),
        recursion_left: 512,
    });
}
