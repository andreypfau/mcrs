use crate::login::GameProfile;
use crate::world::bus::to;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PlayerInfoEntry};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{
    Flying, Invulnerable, MayBuild, MayFly, PlayerGameMode, PlayerOpLevel,
    update_abilities_for_game_mode,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::GameEventKind;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChangeGameMode;

const REQUIRED_OP_LEVEL: u8 = 2;

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_change_game_mode);
    }
}

fn handle_change_game_mode(
    event: On<ReceivedPacketEvent>,
    mut players: Query<
        (
            &PlayerOpLevel,
            &GameProfile,
            &mut PlayerGameMode,
            &mut Invulnerable,
            &mut MayFly,
            &mut MayBuild,
            &HostAnchor,
        ),
        With<HostAnchor>,
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    let Some(pkt) = event.decode::<ServerboundChangeGameMode>() else {
        return;
    };

    {
        let Ok((
            op_level,
            _profile,
            mut current_mode,
            mut invulnerable,
            mut may_fly,
            mut may_build,
            anchor,
        )) = players.get_mut(event.entity)
        else {
            return;
        };

        if op_level.clamped() < REQUIRED_OP_LEVEL {
            tracing::warn!(
                "player {:?} tried to change game mode to {:?} without permission",
                event.entity,
                pkt.mode
            );
            return;
        }

        if current_mode.0 == pkt.mode {
            return;
        }

        current_mode.0 = pkt.mode;
        let flying = update_abilities_for_game_mode(
            pkt.mode,
            &mut invulnerable,
            &mut may_fly,
            &mut may_build,
        );
        if flying {
            commands.entity(event.entity).insert(Flying);
        } else {
            commands.entity(event.entity).remove::<Flying>();
        }
        packet_writer.write(to(
            anchor.0,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::ChangeGameMode(pkt.mode),
            }),
        ));
    }

    let entries: Vec<PlayerInfoEntry> = players
        .iter()
        .map(|(_, profile, mode, _, _, _, _)| PlayerInfoEntry {
            player_uuid: profile.id,
            username: profile.username.clone(),
            game_mode: mode.0,
            listed: true,
        })
        .collect();

    for (_, _, _, _, _, _, anchor) in players.iter() {
        packet_writer.write(to(
            anchor.0,
            PacketPayload::PlayerInfoUpdate {
                entries: entries.clone(),
            },
        ));
    }
}
