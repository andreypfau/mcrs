use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::menu::Menu;
use crate::menu_cursor::{reset_pointer_shape, show_pointer_shape};
use crate::menu_event::MenuEvent;
use crate::menu_input::{menu_deferred, menu_keys, menu_pointer};
use crate::menu_input_systems::MenuInputSystems;

/// The input of the open [`Menu`] and its [`MenuEvent`]s, for an app with
/// [`crate::scene::GuiPlugin`], which draws the menu.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MenuEvent>()
            .configure_sets(PreUpdate, MenuInputSystems.after(InputSystems))
            .add_systems(
                PreUpdate,
                (menu_pointer, menu_keys, menu_deferred, show_pointer_shape)
                    .chain()
                    .in_set(MenuInputSystems)
                    .run_if(resource_exists::<Menu>),
            )
            .add_systems(
                PreUpdate,
                reset_pointer_shape.run_if(resource_removed::<Menu>),
            );
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::ScheduleSystem;
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::mouse::AccumulatedMouseScroll;
    use bevy::time::Real;
    use bevy::window::PrimaryWindow;
    use mcrs_minecraft_client_widget::{
        AreaSlot, CycleButton, EditBox, GuiExtent, GuiInput, GuiPoint, GuiRect, KeyModifiers,
        Slider, Tab, TabBar, Widget, WidgetEvent, WidgetId, WidgetKind, WidgetList, Widgets,
    };

    use super::*;
    use crate::menu::MenuBackground;
    use crate::scene::{GuiConfig, GuiScene};

    #[derive(Resource, Default)]
    struct Seen(Vec<MenuEvent>);

    fn open_screen(mut commands: Commands) {
        let mut widgets =
            Widgets::new(GuiExtent::new(320, 240), GuiInput::new(GuiPoint::new(0, 0)));
        for (id, y) in [(1, 10), (2, 40)] {
            let rect = GuiRect::new(10, y, 100, 20);
            widgets
                .push(Widget::new(
                    WidgetId(id),
                    rect,
                    "Done".into(),
                    WidgetKind::Button,
                ))
                .unwrap();
        }
        commands.insert_resource(Menu::new(widgets, MenuBackground::Texture));
    }

    fn read_events(mut events: MessageReader<MenuEvent>, mut seen: ResMut<Seen>) {
        seen.0.extend(events.read().copied());
    }

    const LIST: WidgetId = WidgetId(1);
    const CYCLE: WidgetId = WidgetId(10);
    const SLIDER: WidgetId = WidgetId(11);
    const LAST_ROW: WidgetId = WidgetId(13);

    /// An option list of four rows in an area that shows three: a cycle button and a slider in
    /// row 0, a button in each other row.
    fn open_option_list(mut commands: Commands) {
        let screen = GuiExtent::new(427, 240);
        let mut widgets = Widgets::new(screen, GuiInput::new(GuiPoint::new(0, 0)));
        let area = GuiRect::new(0, 33, 427, 60);
        let mut list = WidgetList::options();
        for _ in 0..4 {
            list.push_row(25);
        }
        let rows: Vec<GuiRect> = (0..4).filter_map(|row| list.row_rect(area, row)).collect();
        let kind = WidgetKind::WidgetList(list);
        widgets
            .push(Widget::new(LIST, area, String::new(), kind))
            .unwrap();
        let cycle = WidgetKind::CycleButton(CycleButton::boolean(false));
        let slider = WidgetKind::Slider(Slider::continuous(0.5));
        let members = [
            (CYCLE, 0, 0, cycle),
            (SLIDER, 0, 1, slider),
            (WidgetId(12), 1, 0, WidgetKind::Button),
            (LAST_ROW, 3, 0, WidgetKind::Button),
        ];
        for (id, row, column, kind) in members {
            let origin = WidgetList::option_origin(screen.width, rows[row], column);
            let rect = GuiRect::new(origin.x, origin.y, 150, 20);
            let mut widget = Widget::new(id, rect, String::new(), kind);
            widget.area = Some(AreaSlot { area: LIST, row });
            widgets.push(widget).unwrap();
        }
        commands.insert_resource(Menu::new(widgets, MenuBackground::Texture));
    }

    fn open_tab_bar(mut commands: Commands) {
        let screen = GuiExtent::new(427, 240);
        let mut widgets = Widgets::new(screen, GuiInput::new(GuiPoint::new(0, 0)));
        let tab = |label: &str| Tab {
            label: label.into(),
            active: true,
        };
        let bar = TabBar::new(vec![tab("Game"), tab("World"), tab("More")]);
        let rect = bar.menu_rect(screen.width);
        let kind = WidgetKind::TabBar(bar);
        widgets
            .push(Widget::new(LIST, rect, String::new(), kind))
            .unwrap();
        commands.insert_resource(Menu::new(widgets, MenuBackground::Texture));
    }

    const EDIT: WidgetId = WidgetId(1);

    fn open_edit_box(mut commands: Commands) {
        let mut widgets =
            Widgets::new(GuiExtent::new(320, 240), GuiInput::new(GuiPoint::new(0, 0)));
        let kind = WidgetKind::EditBox(EditBox::default());
        let rect = GuiRect::new(10, 10, 150, 20);
        widgets
            .push(Widget::new(EDIT, rect, String::new(), kind))
            .unwrap();
        widgets.focus_initial(Some(EDIT), 0, &mut Vec::new());
        commands.insert_resource(Menu::new(widgets, MenuBackground::Texture));
    }

    fn screen_app() -> App {
        app_opening(open_screen)
    }

    fn app_opening<M>(open: impl IntoScheduleConfigs<ScheduleSystem, M>) -> App {
        let mut app = App::new();
        app.add_plugins(MenuPlugin)
            .insert_resource(GuiConfig {
                scale: 1,
                cursor: Some(IVec2::new(20, 45)),
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .init_resource::<GuiScene>()
            .init_resource::<Time<Real>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<Seen>()
            .add_message::<KeyboardInput>()
            .add_systems(Startup, open)
            .add_systems(PreUpdate, read_events.after(MenuInputSystems));
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app
    }

    #[test]
    fn a_click_on_a_button_reaches_the_screen_as_its_activation() {
        let mut app = screen_app();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().0,
            [
                MenuEvent::Widget(WidgetEvent::Activated {
                    widget: WidgetId(2),
                    modifiers: KeyModifiers::NONE,
                }),
                MenuEvent::Widget(WidgetEvent::FocusChanged {
                    focused: Some(WidgetId(2)),
                }),
            ]
        );
    }

    #[test]
    fn escape_is_a_close_request_except_in_the_frame_the_menu_opens() {
        let mut app = screen_app();
        let mut seen_after_escape = || {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Escape,
                logical_key: Key::Escape,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
            std::mem::take(&mut app.world_mut().resource_mut::<Seen>().0)
        };
        assert_eq!(seen_after_escape(), []);
        assert_eq!(seen_after_escape(), [MenuEvent::CloseRequested]);
    }

    #[test]
    fn typed_text_reaches_the_edit_box_and_not_the_key_bindings() {
        let mut app = app_opening(open_edit_box);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyA);
        for (key_code, letter) in [(KeyCode::KeyA, "a"), (KeyCode::KeyB, "b")] {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: Key::Character(letter.into()),
                state: ButtonState::Pressed,
                text: Some(letter.into()),
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        let changed = MenuEvent::Widget(WidgetEvent::ValueChanged { widget: EDIT });
        assert_eq!(app.world().resource::<Seen>().0, [changed, changed]);
        let text = match app
            .world()
            .resource::<Menu>()
            .widgets
            .get(EDIT)
            .map(|w| &w.kind)
        {
            Some(WidgetKind::EditBox(edit)) => edit.text(),
            _ => "",
        };
        assert_eq!(text, "ab");
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.pressed(KeyCode::KeyA) && !keys.just_pressed(KeyCode::KeyA));
    }

    fn point_at(app: &mut App, x: i32, y: i32) {
        app.world_mut().resource_mut::<GuiConfig>().cursor = Some(IVec2::new(x, y));
    }

    fn click(app: &mut App) -> Vec<MenuEvent> {
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.press(MouseButton::Left);
        app.update();
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.release(MouseButton::Left);
        buttons.clear();
        app.update();
        std::mem::take(&mut app.world_mut().resource_mut::<Seen>().0)
    }

    #[test]
    fn an_option_list_scrolls_a_notch_and_its_cycle_button_and_slider_report_their_values() {
        let mut app = app_opening(open_option_list);
        point_at(&mut app, 100, 50);
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = -1.0;
        app.update();
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = 0.0;
        let menu = app.world().resource::<Menu>();
        let offset = menu.widgets.get(LIST).and_then(Widget::scroll_offset);
        assert_eq!(offset, Some(12.0));

        let last_row = menu.widgets.get(LAST_ROW).unwrap().rect();
        point_at(&mut app, last_row.x + 10, last_row.y - 12 + 5);
        assert_eq!(click(&mut app), []);

        point_at(&mut app, 100, 40);
        let cycle = click(&mut app);
        assert!(
            cycle.contains(&MenuEvent::Widget(WidgetEvent::ValueChanged {
                widget: CYCLE
            })),
            "{cycle:?}"
        );
        point_at(&mut app, 300, 40);
        let slider = click(&mut app);
        assert!(
            slider.contains(&MenuEvent::Widget(WidgetEvent::ValueChanged {
                widget: SLIDER
            })),
            "{slider:?}"
        );
    }

    #[test]
    fn ctrl_or_command_with_a_digit_of_either_row_picks_a_tab() {
        let cases = [
            (KeyCode::ControlLeft, KeyCode::Digit2, 1),
            (KeyCode::SuperRight, KeyCode::Numpad3, 2),
        ];
        for (modifier, digit, tab) in cases {
            let mut app = app_opening(open_tab_bar);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(modifier);
            app.world_mut().write_message(KeyboardInput {
                key_code: digit,
                logical_key: Key::Character("2".into()),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
            let seen = &app.world().resource::<Seen>().0;
            let changed = MenuEvent::Widget(WidgetEvent::TabChanged { widget: LIST, tab });
            assert!(seen.contains(&changed), "{digit:?}: {seen:?}");
        }
    }
}
