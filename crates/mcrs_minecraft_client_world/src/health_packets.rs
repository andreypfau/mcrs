use bevy::prelude::*;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundRespawn, ClientboundSetEntityData, ClientboundSetHealth,
};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity_data_index::HEALTH;

use crate::food_level::FoodLevel;
use crate::joined_game::JoinedGame;
use crate::player::Player;

pub(crate) const NEW_PLAYER_VITALS: Vitals = Vitals::new(20.0);
pub(crate) const NEW_PLAYER_FOOD: FoodLevel = FoodLevel {
    food: 20,
    saturation: 5.0,
};

fn with_health(current: Vitals, health: f32) -> Vitals {
    Vitals {
        health,
        death_time: (health <= 0.0).then_some(0),
        ..current
    }
}

pub(crate) fn on_set_health(
    event: On<Received<ClientboundSetHealth>>,
    mut player: Query<(&mut Vitals, &mut FoodLevel), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((mut vitals, mut food_level)) = player.single_mut() else {
        return;
    };
    let received = with_health(*vitals, packet.health);
    vitals.set_if_neq(received);
    food_level.set_if_neq(FoodLevel {
        food: packet.food.0,
        saturation: packet.saturation,
    });
}

pub(crate) fn on_set_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    connections: Query<&JoinedGame>,
    mut player: Query<&mut Vitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == packet.entity_id.0)
    {
        return;
    }
    let health =
        packet
            .metadata
            .0
            .iter()
            .rev()
            .find_map(|entry| match (entry.index, &entry.value) {
                (HEALTH, MetaDataValue::Float(health)) => Some(*health),
                _ => None,
            });
    if let Some(health) = health
        && let Ok(mut vitals) = player.single_mut()
    {
        let received = with_health(*vitals, health);
        vitals.set_if_neq(received);
    }
}

pub(crate) fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    mut player: Query<(&mut Vitals, &mut FoodLevel), With<Player>>,
) {
    if event
        .packet()
        .is_some_and(|packet| packet.data_to_keep & ClientboundRespawn::KEEP_ENTITY_DATA == 0)
        && let Ok((mut vitals, mut food_level)) = player.single_mut()
    {
        vitals.set_if_neq(NEW_PLAYER_VITALS);
        food_level.set_if_neq(NEW_PLAYER_FOOD);
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;
    use mcrs_minecraft_damage::Vitals;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundRespawn, ClientboundSetEntityData, ClientboundSetHealth,
    };
    use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};

    use crate::food_level::FoodLevel;
    use crate::test_session::{OWN_PLAYER, TestSession, spawn_info};

    fn set_health(health: f32, food: i32, saturation: f32) -> ClientboundSetHealth {
        ClientboundSetHealth {
            health,
            food: VarInt(food),
            saturation,
        }
    }

    fn vitals(session: &TestSession) -> Vitals {
        *session.app.world().get::<Vitals>(session.player).unwrap()
    }

    #[test]
    fn set_health_writes_the_vitals_and_the_food_level_of_the_own_player() {
        let mut session = TestSession::new(DVec3::ZERO);

        session.receive(set_health(0.0, 3, 0.5));
        assert_eq!(vitals(&session).health, 0.0);
        assert_eq!(vitals(&session).death_time, Some(0));
        assert_eq!(
            session.app.world().get::<FoodLevel>(session.player),
            Some(&FoodLevel {
                food: 3,
                saturation: 0.5,
            })
        );

        session.receive(set_health(20.0, 18, 2.5));
        assert_eq!(vitals(&session).health, 20.0);
        assert!(vitals(&session).is_alive());
        assert_eq!(
            session.app.world().get::<FoodLevel>(session.player),
            Some(&FoodLevel {
                food: 18,
                saturation: 2.5,
            })
        );
    }

    #[test]
    fn the_health_index_of_the_own_player_sets_its_health() {
        let mut session = TestSession::new(DVec3::ZERO);
        let health = |entity_id: i32, health: f32| ClientboundSetEntityData {
            entity_id: VarInt(entity_id),
            metadata: Metadata(vec![MetadataEntry {
                index: 9,
                value: MetaDataValue::Float(health),
            }]),
        };

        session.receive(health(OWN_PLAYER + 1, 4.0));
        assert_eq!(vitals(&session).health, 20.0, "another entity");

        session.receive(health(OWN_PLAYER, 0.0));
        assert_eq!(vitals(&session).health, 0.0);
        assert_eq!(vitals(&session).death_time, Some(0));
    }

    #[test]
    fn a_respawn_resets_the_health_and_the_food_unless_it_keeps_the_entity_data() {
        for (data_to_keep, expected) in [
            (0x00, (20.0, 20)),
            (ClientboundRespawn::KEEP_ATTRIBUTE_MODIFIERS, (20.0, 20)),
            (
                ClientboundRespawn::KEEP_ATTRIBUTE_MODIFIERS | ClientboundRespawn::KEEP_ENTITY_DATA,
                (6.0, 3),
            ),
        ] {
            let mut session = TestSession::new(DVec3::ZERO);
            session.receive(set_health(6.0, 3, 0.5));
            session.receive(ClientboundRespawn {
                player_spawn_info: spawn_info(GameMode::Survival),
                data_to_keep,
            });
            let food = session
                .app
                .world()
                .get::<FoodLevel>(session.player)
                .unwrap();
            assert_eq!(
                (vitals(&session).health, food.food),
                expected,
                "data to keep {data_to_keep:#04x}"
            );
        }
    }
}
