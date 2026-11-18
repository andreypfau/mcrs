use bevy::prelude::*;
use bevy::window::CursorOptions;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;

use crate::cursor_grab;

pub(crate) fn close_menu_screen(
    received: On<Received<ClientboundContainerClose>>,
    screen: Option<ResMut<Screen>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Some(mut screen) = screen else {
        return;
    };
    if *screen != Screen::Menu || received.packet().is_none() {
        return;
    }
    *screen = Screen::None;
    if let Ok(mut cursor) = cursor.single_mut() {
        cursor_grab::grab(&mut cursor, true);
    }
}
