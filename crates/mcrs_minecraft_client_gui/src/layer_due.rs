use crate::gui_layer::GuiLayer;
use crate::menu_input;
use bevy::prelude::*;

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub(crate) fn layer_due<Kind: Component>(
    time: Res<Time<Real>>,
    layers: Query<&GuiLayer, With<Kind>>,
) -> bool {
    let now = menu_input::menu_clock(&time);
    layers.iter().any(|layer| layer.due(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_band::GuiBand;
    use bevy::ecs::system::RunSystemOnce;
    use core::time::Duration;

    #[derive(Component)]
    struct Marker;

    #[test]
    fn due_only_when_the_time_has_come() {
        let mut app = App::new();
        app.insert_resource(Time::<Real>::default());
        let layer = app
            .world_mut()
            .spawn((GuiLayer::new(GuiBand::Hud, 0), Marker))
            .id();
        let due = |app: &mut App| {
            app.world_mut()
                .run_system_once(layer_due::<Marker>)
                .unwrap()
        };

        assert!(!due(&mut app));
        app.world_mut()
            .get_mut::<GuiLayer>(layer)
            .unwrap()
            .redraw_at(100);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(99));
        assert!(!due(&mut app));
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(1));
        assert!(due(&mut app));
    }
}
