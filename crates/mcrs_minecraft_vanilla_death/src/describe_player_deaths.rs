use alloc::format;
use alloc::vec::Vec;

use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_player_vitals::DeathMessage;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_text::Text;

use crate::death_message_text::death_message_text;

pub(crate) fn describe_player_deaths(
    mut deaths: MessageReader<Died>,
    rules: Option<Res<GameRules>>,
    registries: Option<Res<CurrentRegistries>>,
    players: Query<(&EntityUuid, &GameProfile), With<Player>>,
    mob_index: Option<Res<MobIndex>>,
    mobs: Query<&Mob>,
    mut messages: MessageWriter<DeathMessage>,
) {
    if !rules.is_some_and(|rules| rules.is_on(GameRule::ShowDeathMessages)) {
        deaths.clear();
        return;
    }
    let damage_types = registries
        .as_deref()
        .and_then(|registries| registries.registry::<DamageType>());
    for death in deaths.read() {
        let Ok((_, profile)) = players.get(death.target) else {
            continue;
        };
        let victim = Text::text(profile.username.clone());
        let damage_type = damage_types.and_then(|registry| {
            usize::try_from(death.source.damage_type)
                .ok()
                .and_then(|index| registry.values().get(index))
        });
        let cause = death.source.cause;
        let name = |uuid: EntityUuid| {
            player_name(&players, uuid).or_else(|| {
                let mob = mob_index.as_deref()?.get(uuid)?;
                mobs.get(mob).ok().map(mob_name)
            })
        };
        let attacker = cause
            .causing
            .and_then(name)
            .or_else(|| cause.direct.and_then(name));
        messages.write(DeathMessage {
            player: death.target,
            text: death_message_text(victim, damage_type, attacker),
        });
    }
}

fn player_name(
    players: &Query<(&EntityUuid, &GameProfile), With<Player>>,
    uuid: EntityUuid,
) -> Option<Text> {
    players
        .iter()
        .find(|(id, _)| **id == uuid)
        .map(|(_, profile)| Text::text(profile.username.clone()))
}

fn mob_name(mob: &Mob) -> Text {
    match &mob.custom_name {
        Some(name) => Text::text(name.clone()),
        None => {
            let location = mob.kind.entity_type.location();
            let key = format!("entity.{}.{}", location.namespace(), location.path());
            Text::translate(key, Vec::new())
        }
    }
}
