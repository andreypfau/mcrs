use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};
use mcrs_minecraft_client_widget::PointerShape;

use crate::menu::Menu;

type WindowIcon<'w, 's> =
    Single<'w, 's, (Entity, Option<&'static mut CursorIcon>), With<PrimaryWindow>>;

/// The pointer shape of the hovered widget on the primary window (WID-STATE-004).
pub(crate) fn show_pointer_shape(menu: Res<Menu>, window: WindowIcon, commands: Commands) {
    let icon = match menu.widgets.pointer_shape() {
        PointerShape::Hand => SystemCursorIcon::Pointer,
        PointerShape::ResizeHorizontal => SystemCursorIcon::EwResize,
        PointerShape::ResizeVertical => SystemCursorIcon::NsResize,
        PointerShape::NotAllowed => SystemCursorIcon::NotAllowed,
        PointerShape::Text => SystemCursorIcon::Text,
        _ => SystemCursorIcon::Default,
    };
    set_icon(window, icon, commands);
}

/// The normal pointer once the menu closes.
pub(crate) fn reset_pointer_shape(window: WindowIcon, commands: Commands) {
    set_icon(window, SystemCursorIcon::Default, commands);
}

fn set_icon(window: WindowIcon, icon: SystemCursorIcon, mut commands: Commands) {
    let (entity, current) = window.into_inner();
    let icon = CursorIcon::System(icon);
    match current {
        Some(mut current) => {
            current.set_if_neq(icon);
        }
        None => {
            commands.entity(entity).insert(icon);
        }
    }
}
