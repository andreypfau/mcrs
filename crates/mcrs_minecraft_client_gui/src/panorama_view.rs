use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_render::{PanoramaDraw, PanoramaView};

const FRAME_MS: f32 = 50.0;
const LONG_FRAME: f32 = 7.0;
const LONG_FRAME_STEP: f32 = 0.5;
const DEGREES_PER_STEP: f32 = 0.1;

pub(crate) fn project_panorama_view(
    screen: Res<Screen>,
    players: Query<(), With<Player>>,
    time: Res<Time<Real>>,
    mut view: ResMut<PanoramaView>,
) {
    let draw = match *screen {
        Screen::Message(_) | Screen::LevelLoading => PanoramaDraw::Blurred,
        _ if !players.is_empty() => PanoramaDraw::Hidden,
        Screen::Title => PanoramaDraw::Plain,
        Screen::None
        | Screen::Menu
        | Screen::GameModeSwitcher(_)
        | Screen::Chat
        | Screen::WorldListError => PanoramaDraw::Hidden,
        Screen::Pause
        | Screen::Options
        | Screen::VideoSettings
        | Screen::AccessibilitySettings(_)
        | Screen::Controls(_)
        | Screen::MouseSettings(_)
        | Screen::SelectWorld
        | Screen::DeleteWorldConfirmation
        | Screen::CreateWorld(_)
        | Screen::Disconnected => PanoramaDraw::Blurred,
    };
    let spin_degrees = if draw == PanoramaDraw::Hidden {
        view.spin_degrees
    } else {
        spin(view.spin_degrees, time.delta_secs() * 1000.0)
    };
    view.set_if_neq(PanoramaView { spin_degrees, draw });
}

fn spin(degrees: f32, frame_ms: f32) -> f32 {
    let steps = frame_ms / FRAME_MS;
    let steps = if steps > LONG_FRAME {
        LONG_FRAME_STEP
    } else {
        steps
    };
    (degrees + DEGREES_PER_STEP * steps + 180.0).rem_euclid(360.0) - 180.0
}
