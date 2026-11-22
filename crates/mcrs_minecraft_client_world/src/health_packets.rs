use bevy::prelude::*;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHealth;

use crate::food_level::FoodLevel;
use crate::player::Player;

pub(crate) fn on_set_health(
    event: On<Received<ClientboundSetHealth>>,
    mut player: Query<(Entity, Option<&mut Vitals>, Option<&mut FoodLevel>), With<Player>>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((player, vitals, food_level)) = player.single_mut() else {
        return;
    };
    let current = vitals.as_deref().copied().unwrap_or_default();
    let death_time = if packet.health > 0.0 {
        None
    } else {
        current.death_time.or(Some(0))
    };
    let received_vitals = Vitals {
        health: packet.health,
        death_time,
        ..current
    };
    let received_food = FoodLevel {
        food: packet.food.0,
        saturation: packet.saturation,
    };
    match vitals {
        Some(mut vitals) => {
            vitals.set_if_neq(received_vitals);
        }
        None => {
            commands.entity(player).insert(received_vitals);
        }
    }
    match food_level {
        Some(mut food_level) => {
            food_level.set_if_neq(received_food);
        }
        None => {
            commands.entity(player).insert(received_food);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;
    use mcrs_minecraft_damage::Vitals;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHealth;

    use crate::food_level::FoodLevel;
    use crate::test_session::TestSession;

    fn set_health(health: f32, food: i32, saturation: f32) -> ClientboundSetHealth {
        ClientboundSetHealth {
            health,
            food: VarInt(food),
            saturation,
        }
    }

    #[test]
    fn set_health_writes_the_vitals_and_the_food_level_of_the_own_player() {
        let mut session = TestSession::new(DVec3::ZERO);
        let vitals =
            |session: &TestSession| *session.app.world().get::<Vitals>(session.player).unwrap();

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

        session
            .app
            .world_mut()
            .get_mut::<Vitals>(session.player)
            .unwrap()
            .death_time = Some(7);
        session.receive(set_health(0.0, 3, 0.5));
        assert_eq!(vitals(&session).death_time, Some(7));

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
}
