use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::message::MessageReader;
use bevy_ecs::observer::On;
use bevy_ecs::query::Has;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::HiddenPlayer;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::PlayerBodyItem;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine_damage::RespawnRequest;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::Launch;

type DeadPlayer<'a> = (
    &'a Vitals,
    &'a PlayerHolders,
    &'a Transform,
    PlayerBody,
    Option<&'a EntityUuid>,
    Has<HiddenPlayer>,
);

pub(crate) fn drop_held_at_removal(
    insert: On<Insert<HiddenPlayer>>,
    players: Query<DeadPlayer, With<Player>>,
    holder_slots: Query<&Slots>,
    mut commands: Commands,
) {
    if let Ok((vitals, holders, transform, body, uuid, _)) = players.get(insert.event().entity)
        && !vitals.is_alive()
    {
        drop_held(
            holders,
            transform,
            &body,
            uuid,
            &holder_slots,
            commands.reborrow(),
        );
    }
}

pub(crate) fn drop_held_at_respawn(
    mut requests: MessageReader<RespawnRequest>,
    players: Query<DeadPlayer, With<Player>>,
    holder_slots: Query<&Slots>,
    mut commands: Commands,
) {
    for request in requests.read() {
        drop_held_of_shown_dead(request.player, &players, &holder_slots, commands.reborrow());
    }
}

pub(crate) fn drop_held_at_departure(
    mut departures: MessageReader<InboundPlayerDespawn>,
    index: Option<Res<DimPlayerIndex>>,
    players: Query<DeadPlayer, With<Player>>,
    holder_slots: Query<&Slots>,
    mut commands: Commands,
) {
    let Some(index) = index else {
        departures.clear();
        return;
    };
    for departure in departures.read() {
        if let Some(&player) = index.0.get(&departure.session) {
            drop_held_of_shown_dead(player, &players, &holder_slots, commands.reborrow());
        }
    }
}

fn drop_held_of_shown_dead(
    player: Entity,
    players: &Query<DeadPlayer, With<Player>>,
    holder_slots: &Query<&Slots>,
    commands: Commands<'_, '_>,
) {
    if let Ok((vitals, holders, transform, body, uuid, hidden)) = players.get(player)
        && !vitals.is_alive()
        && !hidden
    {
        drop_held(holders, transform, &body, uuid, holder_slots, commands);
    }
}

fn drop_held(
    holders: &PlayerHolders,
    transform: &Transform,
    body: &PlayerBodyItem,
    uuid: Option<&EntityUuid>,
    holder_slots: &Query<&Slots>,
    mut commands: Commands<'_, '_>,
) {
    let launch = Launch {
        source: BirthSource::Entity(uuid.copied().unwrap_or_default()),
        form: Form::Hand {
            feet: transform.translation,
            eye_height: body.dimensions().eye_height,
            velocity: HandVelocity::Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
        },
        values: ItemEntity::with_pickup_delay(Cause::MenuRest.pickup_delay()),
    };
    for holder in [holders.cursor, holders.craft_grid] {
        let Ok(slots) = holder_slots.get(holder) else {
            continue;
        };
        for stack in slots.occupied() {
            commands.entity(stack).launch(launch);
        }
    }
}
