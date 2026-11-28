use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::{Changed, Or, With};
use bevy_ecs::system::{Commands, Populated};
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::player_state::PlayerFileState;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHealth;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct SentHealth {
    health: f32,
    food: i32,
    saturation: Saturation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Saturation {
    Empty,
    Positive,
}

pub(crate) fn send_health(
    mut players: Populated<
        (
            Entity,
            &Vitals,
            &PlayerFileState,
            &Owner,
            Option<&mut SentHealth>,
        ),
        (
            With<Player>,
            Or<(Changed<Vitals>, Changed<PlayerFileState>)>,
        ),
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    for (player, vitals, file_state, &Owner(session), sent) in &mut players {
        let current = SentHealth {
            health: vitals.health,
            food: file_state.food_level,
            saturation: if file_state.food_saturation_level == 0.0 {
                Saturation::Empty
            } else {
                Saturation::Positive
            },
        };
        match sent {
            Some(mut sent) => {
                if !sent.set_if_neq(current) {
                    continue;
                }
            }
            None => {
                commands.entity(player).insert(current);
            }
        }
        writer.write(bus::to(
            session,
            PacketPayload::SetHealth(ClientboundSetHealth {
                health: vitals.health,
                food: VarInt(file_state.food_level),
                saturation: file_state.food_saturation_level,
            }),
        ));
    }
}
