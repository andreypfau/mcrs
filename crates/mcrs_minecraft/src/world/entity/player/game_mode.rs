use crate::login::GameProfile;
use crate::world::entity::player::ability::{
    Flying, Invulnerable, MayBuild, MayFly, PlayerGameMode, PlayerOpLevel,
    update_abilities_for_game_mode,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{InGameConnectionState, ServerSideConnection};
use mcrs_protocol::packets::game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerInfoUpdate,
};
use mcrs_protocol::packets::game::serverbound::ServerboundChangeGameMode;
use mcrs_protocol::profile::{PlayerListActions, PlayerListEntry};
use mcrs_protocol::{GameEventKind, WritePacket};

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
            &mut ServerSideConnection,
        ),
        With<InGameConnectionState>,
    >,
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
            mut con,
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
        con.write_packet(&ClientboundGameEvent {
            game_event: GameEventKind::ChangeGameMode(pkt.mode),
        });
    }

    let names = players
        .iter()
        .map(|(_, profile, _, _, _, _, _, _)| profile.username.clone())
        .collect::<Vec<_>>();
    let entries: Vec<PlayerListEntry> = players
        .iter()
        .zip(names.iter())
        .map(|((_, profile, mode, _, _, _, _, _), name)| PlayerListEntry {
            username: name.as_str(),
            player_uuid: profile.id,
            properties: profile.properties.iter().cloned().collect(),
            listed: true,
            game_mode: mode.0,
            ..Default::default()
        })
        .collect();

    let info_pkt = ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_game_mode(true),
        entries: entries.into(),
    };

    players
        .iter_mut()
        .for_each(|(_, _, _, _, _, _, _, mut con)| con.write_packet(&info_pkt));
}
