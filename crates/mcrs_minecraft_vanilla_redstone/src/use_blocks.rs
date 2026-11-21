use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res};
use mcrs_minecraft_engine::block_simulation::BlockRules;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine::use_item_on_request::{UseItemOnConsumed, UseItemOnRequest};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine::{PlayerBody, ShiftKey, eye_position, in_reach, reach};
use mcrs_minecraft_engine_item::{Selected, Slots};
use mcrs_minecraft_engine_world::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_inventory_player::OFF_HAND;
use mcrs_minecraft_redstone::{BlockUse, HasRedstone};
use mcrs_minecraft_simulation::BlockBehaviour;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::FxHashMap;

type UserItem<'a> = (
    &'a InDimension,
    &'a Transform,
    &'a PlayerGameMode,
    PlayerBody,
    Option<&'a ShiftKey>,
    Option<&'a TeleportState>,
    Option<&'a Slots>,
    Option<&'a Selected>,
);

fn holds_an_item(slots: Option<&Slots>, selected: Option<&Selected>) -> bool {
    let Some(slots) = slots else {
        return false;
    };
    let main = selected.and_then(|selected| slots.get(usize::from(selected.0)));
    main.is_some() || slots.get(usize::from(OFF_HAND)).is_some()
}

pub(crate) fn use_redstone_blocks<B>(
    mut requests: MessageReader<UseItemOnRequest>,
    users: Query<UserItem, With<Player>>,
    world: WorldView,
    rules: Res<BlockRules<B>>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut consumed: MessageWriter<UseItemOnConsumed>,
    mut pending: Local<FxHashMap<(Entity, BlockPos), BlockStateId>>,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
{
    let redstone = rules.0.redstone();
    pending.clear();
    for request in requests.read() {
        if request.sequence.0 < 0 {
            continue;
        }
        let Ok((dimension, transform, mode, body, shift, teleport, slots, selected)) =
            users.get(request.entity)
        else {
            continue;
        };
        if mode.0 == GameMode::Spectator {
            continue;
        }
        if teleport.is_some_and(|teleport| teleport.pending().is_some()) {
            continue;
        }
        if shift.is_some_and(|shift| shift.0) && holds_an_item(slots, selected) {
            continue;
        }
        let eye = eye_position(transform.translation, body.dimensions());
        if !reach(body.attributes).is_some_and(|limit| in_reach(eye, request.block_pos, limit)) {
            continue;
        }
        let Some(view) = world.get(dimension.0) else {
            continue;
        };
        let key = (dimension.0, request.block_pos);
        let state = match pending.get(&key) {
            Some(&state) => state,
            None => match view.block(request.block_pos) {
                Ok(state) => state,
                Err(_) => continue,
            },
        };
        match redstone.used(state) {
            BlockUse::Toggle(toggle) => {
                pending.insert(key, toggle.state);
                blocks.write(BlockSetRequest {
                    new_state: toggle.state,
                    flags: toggle.flags,
                    ..remove_block(dimension.0, request.block_pos)
                });
            }
            BlockUse::Consumed => {}
            BlockUse::Unhandled => continue,
        }
        consumed.write(UseItemOnConsumed {
            entity: request.entity,
            sequence: request.sequence,
        });
    }
}
