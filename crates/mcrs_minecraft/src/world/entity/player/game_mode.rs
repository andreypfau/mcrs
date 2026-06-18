use crate::login::GameProfile;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, PlayerInfoEntry};
use crate::world::entity::player::ability::{
    Flying, Invulnerable, MayBuild, MayFly, PlayerGameMode, PlayerOpLevel,
    update_abilities_for_game_mode,
};
use crate::world::entity::player::HostAnchor;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_engine::session::PlayerSession;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::ServerboundChangeGameMode;
use mcrs_protocol::GameEventKind;

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
            &mut Flying,
            &mut MayFly,
            &mut MayBuild,
            &HostAnchor,
        ),
        With<HostAnchor>,
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
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
            mut flying,
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
        update_abilities_for_game_mode(
            pkt.mode,
            &mut invulnerable,
            &mut flying,
            &mut may_fly,
            &mut may_build,
        );
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(anchor.0),
            priority: PacketPriority::Normal,
            data: PacketPayload::GameEvent {
                game_event: GameEventKind::ChangeGameMode(pkt.mode),
            },
            session: PlayerSession(0),
            epoch: 0,
        });
    }

    let entries: Vec<PlayerInfoEntry> = players
        .iter()
        .map(|(_, profile, mode, _, _, _, _, _)| PlayerInfoEntry {
            player_uuid: profile.id,
            username: profile.username.clone(),
            game_mode: mode.0,
            listed: true,
        })
        .collect();

    for (_, _, _, _, _, _, _, anchor) in players.iter() {
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(anchor.0),
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerInfoUpdate {
                entries: entries.clone(),
            },
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}
