use bevy_ecs::change_detection::Mut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::In;
use bevy_ecs::system::InMut;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_engine_inventory_click::MenuClicker;
use mcrs_minecraft_engine_inventory_click::MenuClicks;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_registries::write_packet_hashed;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::item::HashedStack;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;

use crate::RemoteMenu;
use crate::click_purpose::ClickPurpose;
use crate::click_source::ClickSource;
use crate::local_inventory::StackNames;
use crate::outbox::Outbox;
use crate::outgoing::Outgoing;
use crate::predict_creative::CreativeScratch;

#[derive(Resource, Default)]
pub(crate) struct ClickScratch {
    touched: Vec<SlotRef>,
    before: Vec<EntryBefore>,
    hash: HashScratch,
    bytes: Vec<u8>,
    pub(crate) creative: CreativeScratch,
}

struct EntryBefore {
    entry: u16,
    slot: SlotRef,
    stack: Option<ItemStack>,
}

struct PlannedClick {
    container_id: i32,
    state_id: i32,
    request: Option<Request>,
}

#[derive(Default)]
struct ClickClaims {
    changed: Vec<(u16, Option<HashedStack>)>,
    carried: Option<HashedStack>,
}

pub(crate) fn predict_click(world: &mut World, viewer: Entity, click: Click) {
    world.try_resource_scope(|world, mut scratch: Mut<ClickScratch>| {
        let Some(packet) = predict(world, viewer, click, &mut scratch) else {
            return;
        };
        if let Some(mut outbox) = world.get_resource_mut::<Outbox>() {
            outbox.0.push(Outgoing::Click(packet));
        }
    });
}

fn predict(
    world: &mut World,
    viewer: Entity,
    click: Click,
    scratch: &mut ClickScratch,
) -> Option<ServerboundContainerClick> {
    let planned = world
        .run_system_cached_with(plan_click, (viewer, click, &mut *scratch))
        .ok()??;
    if let Some(request) = planned.request {
        world.commands().entity(viewer).request(request);
        world.flush();
    }
    let claims = world
        .run_system_cached_with(claim_click, (viewer, &mut *scratch))
        .ok()
        .flatten()
        .unwrap_or_default();
    Some(ServerboundContainerClick {
        container_id: VarInt(planned.container_id),
        state_seqno: VarInt(planned.state_id),
        click,
        changed_slots: Bounded(claims.changed),
        carried_item: claims.carried,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query and resource is its own parameter"
)]
fn plan_click(
    (In(viewer), In(click), InMut(scratch)): (In<Entity>, In<Click>, InMut<ClickScratch>),
    players: Query<
        (
            &PlayerHolders,
            Option<&Selected>,
            Option<&PlayerGameMode>,
            Option<&Abilities>,
            &Menus,
        ),
        With<Slots>,
    >,
    menus: Query<(&MenuLayout, &MenuParts, Option<&RemoteMenu>)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    source: Option<Res<ClickSource>>,
    mut clicks: MenuClicks,
    names: StackNames,
) -> Option<PlannedClick> {
    scratch.touched.clear();
    scratch.before.clear();
    let (player_holders, selected, game_mode, abilities, player_menus) =
        players.get(viewer).ok()?;
    let menu = player_menus.open()?;
    let (layout, parts, remote) = menus.get(menu).ok()?;
    let mut planned = PlannedClick {
        container_id: remote.map_or(RemoteMenu::INVENTORY, RemoteMenu::container_id),
        state_id: remote.map_or(0, RemoteMenu::state_id),
        request: None,
    };
    if game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Spectator) {
        return Some(planned);
    }
    let mode = if abilities.is_some_and(|abilities| abilities.instant_build) {
        PlayerMode::Creative
    } else {
        PlayerMode::NotCreative
    };
    let Some(source) = source else {
        return Some(planned);
    };
    let clicker = MenuClicker {
        player: ClickPlayer {
            mode,
            selected: selected.map_or(0, |selected| selected.0),
        },
        cursor: SlotRef {
            holder: player_holders.cursor,
            index: 0,
        },
        request: RequestId {
            source: source.0,
            serial: ClickPurpose::Click.serial(),
        },
    };
    let Ok(request) = clicks.plan(viewer, click, clicker) else {
        return Some(planned);
    };
    for op in request.ops() {
        let (first, second) = match op {
            Op::Split { from, to, .. } | Op::Merge { from, to } => (from, Some(to)),
            Op::Swap { left, right } => (left, Some(right)),
            Op::Set { slot, .. } => (slot, None),
            Op::Drop { from, .. } => (from, None),
        };
        scratch.touched.push(first);
        scratch.touched.extend(second);
    }
    if scratch.touched.is_empty() {
        return Some(planned);
    }
    for entry in 0..layout.len() {
        let Some(slot) = layout.entry(entry).and_then(|entry| parts.slot(entry)) else {
            continue;
        };
        if !scratch.touched.contains(&slot) {
            continue;
        }
        let stack = (holders.get(slot.holder).ok())
            .and_then(|slots| slots.get(usize::from(slot.index)))
            .and_then(|stack| stacks.to_value(stack));
        scratch.before.push(EntryBefore { entry, slot, stack });
    }
    planned.request = names.names().is_some().then_some(request);
    Some(planned)
}

fn claim_click(
    (In(viewer), InMut(scratch)): (In<Entity>, InMut<ClickScratch>),
    players: Query<&PlayerHolders>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    names: StackNames,
) -> Option<ClickClaims> {
    let names = names.names()?;
    let ClickScratch {
        before,
        hash,
        bytes,
        ..
    } = scratch;
    let occupant = |slot: SlotRef| {
        (holders.get(slot.holder).ok()).and_then(|slots| slots.get(usize::from(slot.index)))
    };
    let mut changed = Vec::new();
    for before in before.iter() {
        let after = occupant(before.slot);
        let unchanged = match (before.stack.as_ref(), after) {
            (None, None) => true,
            (Some(value), Some(stack)) => stacks.equals_value(stack, value),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if unchanged {
            continue;
        }
        let value = after.and_then(|stack| stacks.to_value(stack));
        changed.push((
            before.entry,
            write_packet_hashed(value.as_ref(), &names, hash, bytes),
        ));
    }
    let cursor = players.get(viewer).ok().and_then(|player_holders| {
        occupant(SlotRef {
            holder: player_holders.cursor,
            index: 0,
        })
    });
    let cursor = cursor.and_then(|stack| stacks.to_value(stack));
    Some(ClickClaims {
        changed,
        carried: write_packet_hashed(cursor.as_ref(), &names, hash, bytes),
    })
}
