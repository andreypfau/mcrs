use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_world::screen::{CreateWorldParent, Screen};
use mcrs_minecraft_client_world::world_list::WorldList;

use crate::create_world_screen::NewWorldForm;
pub(crate) fn follow_list_answer(
    worlds: Res<WorldList>,
    mut screen: ResMut<Screen>,
    mut form: ResMut<NewWorldForm>,
    time: Res<Time<Real>>,
) {
    match &*worlds {
        WorldList::Ready(worlds) if worlds.is_empty() => {
            screen.set_if_neq(Screen::CreateWorld(CreateWorldParent::Title));
            form.set_if_neq(NewWorldForm::opened(time.elapsed()));
        }
        WorldList::Unavailable(_) => {
            screen.set_if_neq(Screen::WorldListError);
        }
        WorldList::Ready(_) | WorldList::Reading => {}
    }
}

#[cfg(test)]
mod tests {
    mod jar {
        use mcrs_minecraft_client_widget::WidgetKind;
        use mcrs_minecraft_client_world::world_list::{ReadWorldList, WorldStoreError};

        use super::super::*;
        use crate::menu::{Menu, MenuBackground};
        use crate::menu_test_app::{
            click, read_like_the_store, screen, tap_escape, title_app, world,
        };
        use crate::select_world_screen;

        #[test]
        fn singleplayer_waits_for_the_answer_to_its_read() {
            let mut app = title_app();
            let rows = |app: &App| {
                let menu = app.world().resource::<Menu>();
                match menu
                    .widgets
                    .get(select_world_screen::LIST)
                    .map(|widget| widget.kind())
                {
                    Some(WidgetKind::SelectionList(list)) => (screen(app), list.rows()),
                    _ => (screen(app), 0),
                }
            };
            app.add_systems(
                Update,
                read_like_the_store.run_if(on_message::<ReadWorldList>),
            );
            app.insert_resource(WorldList::Ready(Vec::new()));
            click(&mut app, 213, 118);
            assert_eq!(
                (screen(&app), app.world().resource::<WorldList>()),
                (Screen::SelectWorld, &WorldList::Reading)
            );

            let version = mcrs_minecraft_core::VERSION.world_version;
            app.insert_resource(WorldList::Ready(vec![world(
                "new",
                "New World",
                false,
                version,
            )]));
            app.update();
            assert_eq!(rows(&app), (Screen::SelectWorld, 1));

            app.insert_resource(WorldList::Reading);
            app.update();
            app.insert_resource(WorldList::Ready(Vec::new()));
            app.update();
            assert_eq!(screen(&app), Screen::CreateWorld(CreateWorldParent::Title));

            tap_escape(&mut app);
            click(&mut app, 213, 118);
            app.insert_resource(WorldList::Unavailable(WorldStoreError::Io(
                std::io::ErrorKind::NotADirectory,
            )));
            app.update();
            tap_escape(&mut app);
            let background = app.world().resource::<Menu>().background;
            assert_eq!(
                (screen(&app), background),
                (Screen::WorldListError, MenuBackground::ErrorGradient)
            );
            click(&mut app, 213, 150);
            assert_eq!(screen(&app), Screen::Title);
        }
    }
}
