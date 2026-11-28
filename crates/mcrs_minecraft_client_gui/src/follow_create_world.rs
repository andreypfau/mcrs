use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::WidgetEvent;
use mcrs_minecraft_client_world::screen::{Screen, ScreenMessage};
use mcrs_minecraft_client_world::world_list::{CreateWorld, WorldTypes};

use crate::create_world_screen::{self, NewWorldForm, edit_text, show_tab};
use crate::menu::Menu;
use crate::menu_event::MenuEvent;
use crate::session_screens::{DisconnectReason, Reason};
pub(crate) fn on_create_world(screen: Res<Screen>) -> bool {
    matches!(*screen, Screen::CreateWorld(_))
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates the world name only when the player confirms the create world screen"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
pub(crate) fn follow_create_world(
    mut events: MessageReader<MenuEvent>,
    (mut screen, mut reason): (ResMut<Screen>, ResMut<DisconnectReason>),
    mut menu: ResMut<Menu>,
    mut form: ResMut<NewWorldForm>,
    (types, time): (Res<WorldTypes>, Res<Time<Real>>),
    mut creates: MessageWriter<CreateWorld>,
    mut changes: Local<Vec<WidgetEvent>>,
) {
    let types = match &*types {
        WorldTypes::Ready(types) => types.as_slice(),
        WorldTypes::Loading => &[],
        WorldTypes::Failed(error) => {
            reason.0 = Some(Reason::WorldStore(*error));
            screen.set_if_neq(Screen::Disconnected);
            return;
        }
    };
    let mut next = *form;
    for event in events.read() {
        match *event {
            MenuEvent::Widget(WidgetEvent::ValueChanged {
                widget: create_world_screen::SEED,
            }) => {
                if edit_text(&menu.widgets, create_world_screen::SEED).is_some_and(str::is_empty) {
                    next.seed_emptied(time.elapsed());
                }
            }
            MenuEvent::Widget(WidgetEvent::ValueChanged { widget }) => {
                if let Some(widget) = menu.widgets.get(widget) {
                    next.write(widget);
                }
            }
            MenuEvent::Widget(WidgetEvent::TabChanged { tab, .. }) => {
                show_tab(&mut menu.widgets, tab, &mut changes);
            }
            MenuEvent::Widget(WidgetEvent::Activated {
                widget: create_world_screen::CREATE,
                ..
            })
            | MenuEvent::Confirmed => {
                if matches!(*screen, Screen::CreateWorld(_))
                    && let Some(world) = next.new_world(&menu.widgets, types)
                {
                    creates.write(CreateWorld(world));
                    screen.set_if_neq(Screen::Message(ScreenMessage::PreparingWorldCreation));
                }
            }
            _ => {}
        }
    }
    changes.clear();
    form.set_if_neq(next);
}

#[cfg(test)]
mod tests {
    mod jar {
        use mcrs_minecraft_client_world::screen::CreateWorldParent;
        use mcrs_minecraft_client_world::world_list::{
            ReadWorldList, WorldList, WorldStoreError, WorldStoreFailed,
        };
        use mcrs_minecraft_resource::ResourceLocationBuf;
        use mcrs_minecraft_save::{NewWorld, NewWorldMode};

        use super::super::*;
        use crate::disconnected_screen;
        use crate::menu_test_app::{
            caption, click, drained, read_like_the_store, screen, tap_escape, title_app, type_text,
            world, world_types,
        };

        #[test]
        fn create_new_world_writes_the_values_of_both_tabs() {
            let mut app = title_app();
            let version = mcrs_minecraft_core::VERSION.world_version;
            app.insert_resource(WorldList::Ready(vec![world(
                "alpha", "Alpha", false, version,
            )]))
            .insert_resource(WorldTypes::Ready(world_types()));
            click(&mut app, 213, 118);
            click(&mut app, 292, 198);
            drained::<ReadWorldList>(&mut app);
            click(&mut app, 292, 224);
            assert_eq!(screen(&app), Screen::SelectWorld);
            assert_eq!(drained::<ReadWorldList>(&mut app), [ReadWorldList]);

            tap_escape(&mut app);
            app.add_systems(
                Update,
                read_like_the_store.run_if(on_message::<ReadWorldList>),
            );
            click(&mut app, 213, 118);
            app.insert_resource(WorldList::Ready(Vec::new()));
            app.update();
            assert_eq!(screen(&app), Screen::CreateWorld(CreateWorldParent::Title));
            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::Title);

            click(&mut app, 213, 118);
            app.insert_resource(WorldList::Ready(Vec::new()));
            app.update();
            type_text(&mut app, " 2");
            click(&mut app, 213, 81);
            assert_eq!(
                caption(&app, create_world_screen::GAME_MODE),
                "Game Mode: Hardcore"
            );
            click(&mut app, 307, 12);
            click(&mut app, 133, 45);
            assert_eq!(
                caption(&app, create_world_screen::WORLD_TYPE),
                "World Type: Superflat"
            );
            click(&mut app, 213, 86);
            type_text(&mut app, "123");
            drained::<CreateWorld>(&mut app);
            let create = |app: &mut App| {
                click(app, 134, 224);
                (screen(app), drained::<CreateWorld>(app))
            };
            let first = create(&mut app);
            let random_seed = app.world().resource::<NewWorldForm>().random_seed;
            let preparing = Screen::Message(ScreenMessage::PreparingWorldCreation);
            assert_eq!(
                [first, create(&mut app)],
                [
                    (
                        preparing,
                        vec![CreateWorld(NewWorld {
                            name: "New World 2".to_owned(),
                            game_mode: NewWorldMode::Hardcore,
                            world_type: ResourceLocationBuf::read("minecraft:flat").unwrap(),
                            seed_text: "123".to_owned(),
                            random_seed,
                        })]
                    ),
                    (preparing, Vec::new())
                ]
            );

            app.world_mut()
                .write_message(WorldStoreFailed::Create(WorldStoreError::Encode));
            app.update();
            app.update();
            assert_eq!(
                (screen(&app), caption(&app, disconnected_screen::TITLE)),
                (Screen::Disconnected, "Failed to access world".to_owned())
            );
        }
    }
}
