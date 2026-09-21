use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader};
use bevy_ecs::system::Commands;
use bevy_ecs::world::{EntityWorldMut, World};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::item::{ContainerInput, HashedStack};
use rustc_hash::FxHashMap;

use crate::menu::{CurrentMenu, Menu, MenuLayout, Remote, RemoteSlots};
use crate::plan::{Click, Planner, SLOT_CLICKED_OUTSIDE};
use crate::slot::MenuSnapshot;
use crate::transaction::{Op, Transaction};

#[derive(Message, Debug)]
pub struct ContainerClickRequest {
    pub player: Entity,
    pub game_mode: GameMode,
    pub container_id: i32,
    pub state_id: i32,
    pub slot: i16,
    pub button: u8,
    pub input: ContainerInput,
    pub changed: Vec<(u16, Option<HashedStack>)>,
    pub carried: Option<HashedStack>,
}

#[derive(Default)]
struct MenuFold {
    claims: Vec<(usize, Option<HashedStack>)>,
    carried: Option<Option<HashedStack>>,
    full: bool,
}

/// Every click of a tick folds through one snapshot per player, so a later
/// click sees the earlier one and the player gets one transaction.
pub fn handle_container_clicks(
    world: &World,
    mut requests: MessageReader<ContainerClickRequest>,
    mut commands: Commands,
) {
    let Some(items) = world.get_resource::<Items>() else {
        return;
    };
    let mut plans: FxHashMap<Entity, (MenuSnapshot, Vec<Op>)> = FxHashMap::default();
    let mut menus: FxHashMap<Entity, MenuFold> = FxHashMap::default();
    for req in requests.read() {
        let Some(menu) = world.get::<CurrentMenu>(req.player).map(|current| current.0) else {
            continue;
        };
        let Some((container_id, state_id)) = world
            .get::<Menu>(menu)
            .map(|menu| (menu.container_id, menu.state_id))
        else {
            continue;
        };
        if i32::from(container_id) != req.container_id {
            continue;
        }
        let fold = menus.entry(menu).or_default();
        if req.game_mode == GameMode::Spectator {
            fold.full = true;
            continue;
        }
        let Some(layout) = world.get::<MenuLayout>(menu) else {
            continue;
        };
        let valid_slot = req.slot == -1
            || req.slot == SLOT_CLICKED_OUTSIDE
            || usize::try_from(req.slot).is_ok_and(|slot| slot < layout.0.len());
        if !valid_slot {
            tracing::debug!(player = ?req.player, slot = req.slot, "click on an invalid slot");
            continue;
        }
        fold.full |= req.state_id != i32::from(state_id)
            || matches!(req.input, ContainerInput::QuickCraft | ContainerInput::PickupAll);
        let (snapshot, ops) = plans
            .entry(req.player)
            .or_insert_with(|| (MenuSnapshot::new(world, items, req.player, layout.0.clone()), Vec::new()));
        let mut planner = Planner::new(snapshot);
        planner.click(Click {
            slot: req.slot,
            button: req.button,
            input: req.input,
            creative: req.game_mode == GameMode::Creative,
        });
        ops.extend(planner.ops);
        for (slot, hashed) in &req.changed {
            if usize::from(*slot) < layout.0.len() {
                fold.claims.push((usize::from(*slot), hashed.clone()));
            } else {
                tracing::debug!(player = ?req.player, slot, "changed slot outside the menu");
            }
        }
        fold.carried = Some(req.carried.clone());
    }
    for (_, (_, ops)) in plans {
        if !ops.is_empty() {
            commands.queue(Transaction(ops));
        }
    }
    for (menu, fold) in menus {
        commands.entity(menu).queue(move |mut entity: EntityWorldMut| {
            let Some(mut remote) = entity.get_mut::<RemoteSlots>() else {
                return;
            };
            for (index, hashed) in fold.claims {
                remote.claim(index, hashed);
            }
            if let Some(carried) = fold.carried {
                remote.carried = Remote::Claimed(carried);
            }
            remote.full |= fold.full;
        });
    }
}
