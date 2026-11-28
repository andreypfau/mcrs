use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::packets::table::Row;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerLoaded;
use mcrs_voxel::{BlockPos, SectionPos};

use crate::columns::ColumnStore;
use crate::player::Player;

const TICK_ENDS_BEFORE_TIMEOUT: u8 = 60;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerLoad {
    #[default]
    Loaded,
    Loading(LoadProgress),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadProgress {
    level_chunks_started: bool,
    pub(crate) tick_ends_sent: u8,
}

pub(crate) fn start_load<P: Row>(
    event: On<Received<P>>,
    mut player: Query<&mut PlayerLoad, With<Player>>,
) {
    if event.packet().is_none() {
        return;
    }
    if let Ok(mut load) = player.single_mut() {
        load.set_if_neq(PlayerLoad::Loading(LoadProgress::default()));
    }
}

pub(crate) fn on_game_event(
    event: On<Received<ClientboundGameEvent>>,
    mut player: Query<&mut PlayerLoad, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !matches!(packet.game_event, GameEventKind::LevelChunksLoadStart) {
        return;
    }
    let Ok(mut load) = player.single_mut() else {
        return;
    };
    if let PlayerLoad::Loading(progress) = &mut *load {
        progress.level_chunks_started = true;
    }
}

pub(crate) fn player_loading(player: Query<&PlayerLoad, With<Player>>) -> bool {
    player
        .single()
        .is_ok_and(|load| matches!(load, PlayerLoad::Loading(_)))
}

pub(crate) fn end_player_load(
    player: Single<(&PhysicsTransform, &mut PlayerLoad), With<Player>>,
    columns: Option<Res<ColumnStore>>,
    connection: Option<Single<(&mut ClientConnection, &ConnectionState)>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, state) = connection.into_inner();
    if *state != ConnectionState::Game {
        return;
    }
    let (transform, mut load) = player.into_inner();
    let PlayerLoad::Loading(progress) = *load else {
        return;
    };
    let ready = || {
        columns
            .as_deref()
            .is_some_and(|columns| level_ready(columns, transform.translation))
    };
    if progress.level_chunks_started && ready() {
        connection.write_packet(&ServerboundPlayerLoaded);
        load.set_if_neq(PlayerLoad::Loaded);
    } else if progress.tick_ends_sent >= TICK_ENDS_BEFORE_TIMEOUT {
        load.set_if_neq(PlayerLoad::Loaded);
    }
}

fn level_ready(columns: &ColumnStore, position: DVec3) -> bool {
    columns.holds(ColumnPos::from(position)) || outside_height(columns, position)
}

fn outside_height(columns: &ColumnStore, position: DVec3) -> bool {
    let Some(dimension) = columns.dimension() else {
        return false;
    };
    let extent = dimension.extent;
    let Ok(sections) = i64::try_from(extent.sections) else {
        return false;
    };
    let section_y = i64::from(BlockPos::from(position).y >> SectionPos::BITS);
    let min_section_y = i64::from(extent.min_section_y);
    section_y < min_section_y || section_y >= min_section_y + sections
}
