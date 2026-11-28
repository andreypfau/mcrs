use crate::gui_config::GuiConfig;
use crate::gui_size::{GuiSize, write_gui_size};
use crate::menu_input_systems::MenuInputSystems;
use bevy::input::InputSystems;
use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow};
use mcrs_minecraft_client_widget::GuiPoint;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct GuiPointer {
    at: DVec2,
}

impl GuiPointer {
    #[must_use]
    pub fn new(at: DVec2) -> Self {
        Self { at }
    }

    #[must_use]
    pub fn at(self) -> DVec2 {
        self.at
    }

    #[must_use]
    pub fn point(self) -> GuiPoint {
        GuiPoint::new(self.at.x.floor() as i32, self.at.y.floor() as i32)
    }
}

pub struct GuiPointerPlugin;

impl Plugin for GuiPointerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CursorMoved>()
            .init_resource::<GuiSize>()
            .init_resource::<GuiPointer>()
            .add_systems(
                PreUpdate,
                (write_gui_size, write_gui_pointer)
                    .chain()
                    .after(InputSystems)
                    .before(MenuInputSystems),
            );
    }
}

fn write_gui_pointer(
    mut moves: MessageReader<CursorMoved>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    config: Res<GuiConfig>,
    size: Res<GuiSize>,
    mut physical: Local<DVec2>,
    mut pointer: ResMut<GuiPointer>,
) {
    let (primary, window) = *window;
    let mut moved = false;
    for event in moves.read().filter(|event| event.window == primary) {
        *physical = event.position.as_dvec2() * f64::from(window.scale_factor());
        moved = true;
    }
    if !moved && !size.is_changed() && !config.is_changed() {
        return;
    }
    let at = config.cursor.map_or_else(
        || *physical / f64::from(size.scale.max(1)),
        |cursor| cursor.as_dvec2(),
    );
    pointer.set_if_neq(GuiPointer::new(at));
}

#[cfg(test)]
mod tests {
    use bevy::window::WindowResolution;

    use super::*;

    struct Row {
        name: &'static str,
        scale_factor: f32,
        cursor: Option<IVec2>,
        position: Vec2,
        at: DVec2,
        point: GuiPoint,
    }

    #[test]
    fn the_pointer_keeps_the_last_reported_point_in_gui_units() {
        let rows = [
            Row {
                name: "window scale factor 2",
                scale_factor: 2.0,
                cursor: None,
                position: Vec2::new(50.5, 25.5),
                at: DVec2::new(50.5, 25.5),
                point: GuiPoint::new(50, 25),
            },
            Row {
                name: "pointer outside the window",
                scale_factor: 1.0,
                cursor: None,
                position: Vec2::new(-0.5, -3.0),
                at: DVec2::new(-0.25, -1.5),
                point: GuiPoint::new(-1, -2),
            },
            Row {
                name: "cursor override",
                scale_factor: 1.0,
                cursor: Some(IVec2::new(7, 9)),
                position: Vec2::new(101.0, 51.0),
                at: DVec2::new(7.0, 9.0),
                point: GuiPoint::new(7, 9),
            },
        ];
        for row in rows {
            let mut app = App::new();
            app.insert_resource(GuiConfig {
                scale: 2,
                cursor: row.cursor,
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .add_plugins(GuiPointerPlugin);
            let window = app
                .world_mut()
                .spawn((
                    Window {
                        resolution: WindowResolution::default()
                            .with_scale_factor_override(row.scale_factor),
                        ..default()
                    },
                    PrimaryWindow,
                ))
                .id();
            app.world_mut().write_message(CursorMoved {
                window,
                position: row.position,
                delta: None,
            });
            app.update();
            app.update();
            let pointer = *app.world().resource::<GuiPointer>();
            assert_eq!(pointer.at(), row.at, "{}: the point", row.name);
            assert_eq!(pointer.point(), row.point, "{}: the pixel", row.name);
        }
    }
}
