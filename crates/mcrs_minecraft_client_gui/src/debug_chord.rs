use bevy::prelude::*;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct DebugOverlay(pub(crate) bool);

#[derive(Resource, Default, PartialEq)]
pub struct DebugModifier {
    pub used: bool,
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DebugOverlayToggle;

pub fn toggle_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut modifier: ResMut<DebugModifier>,
    mut overlay: ResMut<DebugOverlay>,
) {
    if keys.just_released(KeyCode::F3) && !std::mem::take(&mut modifier.used) {
        **overlay = !**overlay;
    }
}
