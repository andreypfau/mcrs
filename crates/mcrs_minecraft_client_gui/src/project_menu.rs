use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{
    GuiExtent, GuiInput, GuiPoint, Widget, WidgetEvent, WidgetId, Widgets,
};
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_client_world::session::LocalServer;
use mcrs_minecraft_client_world::world_list::{WorldList, WorldTypes};

use crate::accessibility_settings_screen::{
    self, accessibility_settings_screen, fov_effects_tooltip,
};
use crate::controls_screen::{self, controls_screen};
use crate::create_world_screen::{self, NewWorldForm, create_world_menu};
use crate::delete_world_screen::delete_world_screen;
use crate::disconnected_screen::disconnected_screen;
use crate::follow_menu_events::DeleteTarget;
use crate::font::Span;
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::level_loading_screen::level_loading_screen;
use crate::menu::{Menu, MenuBackground};
use crate::menu_input::menu_clock;
use crate::mouse_settings_screen::mouse_settings_screen;
use crate::options_screen::{self, options_screen};
use crate::pause_screen::pause_screen;
use crate::scene::{GuiAtlas, GuiScene, GuiSize, window_framebuffer};
use crate::select_world_screen::{self, select_world_screen};
use crate::session_screens::{DisconnectReason, Reason};
use crate::title_screen::{self, accessibility_tooltip, title_screen};
use crate::video_settings_screen::{self, video_settings_screen};
use crate::world_list_error_screen::world_list_error_screen;
#[derive(Resource)]
pub(crate) struct SingleplayerActive(pub(crate) bool);

fn keeps_widgets(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Options
            | Screen::VideoSettings
            | Screen::AccessibilitySettings(_)
            | Screen::Controls(_)
            | Screen::MouseSettings(_)
            | Screen::SelectWorld
            | Screen::CreateWorld(_)
    )
}

fn kept_focus(left: Screen, screen: Screen) -> Option<WidgetId> {
    match (left, screen) {
        (Screen::VideoSettings, Screen::Options) => Some(options_screen::VIDEO),
        (Screen::AccessibilitySettings(_), Screen::Options) => Some(options_screen::ACCESSIBILITY),
        (Screen::Controls(_), Screen::Options) => Some(options_screen::CONTROLS),
        (Screen::Controls(_), Screen::AccessibilitySettings(_)) => {
            Some(accessibility_settings_screen::CONTROLS)
        }
        (Screen::Options, Screen::Title) => Some(title_screen::OPTIONS),
        (Screen::AccessibilitySettings(_), Screen::Title) => Some(title_screen::ACCESSIBILITY),
        (Screen::SelectWorld, Screen::Title) => Some(title_screen::SINGLEPLAYER),
        (_, Screen::SelectWorld) => Some(select_world_screen::SEARCH),
        (_, Screen::CreateWorld(_)) => Some(create_world_screen::NAME),
        (Screen::MouseSettings(_), Screen::Controls(_)) => Some(controls_screen::MOUSE_SETTINGS),
        _ => None,
    }
}

#[derive(SystemParam)]
pub(crate) struct MenuSource<'w, 's> {
    screen: Res<'w, Screen>,
    scene: Res<'w, GuiScene>,
    options: Res<'w, ClientOptions>,
    language: Res<'w, ClientLanguage>,
    atlas: Res<'w, GuiAtlas>,
    local_server: Option<Res<'w, LocalServer>>,
    singleplayer: Res<'w, SingleplayerActive>,
    reason: Res<'w, DisconnectReason>,
    worlds: Res<'w, WorldList>,
    types: Res<'w, WorldTypes>,
    form: Res<'w, NewWorldForm>,
    delete_target: Res<'w, DeleteTarget>,
    time: Res<'w, Time<Real>>,
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
}

pub(crate) fn max_gui_scale(window: &Window) -> u32 {
    GuiSize::auto_scale(window_framebuffer(window))
}

#[expect(
    clippy::too_many_lines,
    reason = "one match lists the builder of every menu screen"
)]
fn screen_menu(
    screen: Screen,
    extent: GuiExtent,
    input: GuiInput,
    source: &MenuSource,
) -> Option<Menu> {
    let language = &*source.language;
    let font = &source.atlas.0.font;
    let options = &*source.options;
    let (widgets, tooltips) = match screen {
        Screen::Title => {
            let mut widgets = title_screen(extent, language);
            for widget in &mut widgets {
                if widget.id() == title_screen::SINGLEPLAYER {
                    widget.active = source.singleplayer.0;
                }
            }
            let mut menu = Menu::new(widgets_of(extent, input, widgets)?, MenuBackground::None);
            menu.tooltips = vec![accessibility_tooltip(language)];
            menu.icons = vec![title_screen::ACCESSIBILITY_ICON];
            return Some(menu);
        }
        Screen::SelectWorld => return select_world_menu(extent, input, source),
        Screen::CreateWorld(_) => return create_world_screen_menu(extent, input, source),
        Screen::DeleteWorldConfirmation => (
            widgets_of(
                extent,
                input,
                delete_world_screen(extent, language, font, delete_name(source))?,
            )?,
            Vec::new(),
        ),
        Screen::Pause => (
            widgets_of(
                extent,
                input,
                pause_screen(extent, language, font, last_button_key(source))?,
            )?,
            Vec::new(),
        ),
        Screen::Message(_) => (widgets_of(extent, input, [])?, Vec::new()),
        Screen::LevelLoading => (
            widgets_of(
                extent,
                input,
                [level_loading_screen(extent, language, font)],
            )?,
            Vec::new(),
        ),
        Screen::Disconnected => (
            widgets_of(
                extent,
                input,
                disconnected_screen(
                    extent,
                    language,
                    font,
                    source
                        .reason
                        .0
                        .as_ref()
                        .map_or("disconnect.lost", Reason::title_key),
                    &reason_spans(source),
                )?,
            )?,
            Vec::new(),
        ),
        Screen::WorldListError => {
            return Some(Menu::new(
                widgets_of(
                    extent,
                    input,
                    world_list_error_screen(extent, language, font),
                )?,
                MenuBackground::ErrorGradient,
            ));
        }
        Screen::Options => (
            widgets_of(
                extent,
                input,
                options_screen(extent, language, font, options.fov)?,
            )?,
            Vec::new(),
        ),
        Screen::VideoSettings => (
            widgets_of(
                extent,
                input,
                video_settings_screen(
                    extent,
                    language,
                    font,
                    options,
                    max_gui_scale(&source.window),
                )?,
            )?,
            vec![video_settings_screen::improved_transparency_tooltip(
                language,
            )],
        ),
        Screen::AccessibilitySettings(_) => (
            widgets_of(
                extent,
                input,
                accessibility_settings_screen(extent, language, font, options)?,
            )?,
            vec![fov_effects_tooltip(language)],
        ),
        Screen::Controls(_) => (
            widgets_of(extent, input, controls_screen(extent, language, font)?)?,
            Vec::new(),
        ),
        Screen::MouseSettings(_) => (
            widgets_of(
                extent,
                input,
                mouse_settings_screen(extent, language, font, options)?,
            )?,
            Vec::new(),
        ),
        Screen::None | Screen::Menu | Screen::GameModeSwitcher(_) | Screen::Chat => {
            return None;
        }
    };
    let mut menu = Menu::new(widgets, MenuBackground::Texture);
    menu.tooltips = tooltips;
    Some(menu)
}

fn select_world_menu(extent: GuiExtent, input: GuiInput, source: &MenuSource) -> Option<Menu> {
    let (widgets, hint) = select_world_screen(extent, &source.language, &source.atlas.0.font)?;
    let mut menu = Menu::new(widgets_of(extent, input, widgets)?, MenuBackground::Texture);
    menu.hints = vec![hint];
    Some(menu)
}

fn create_world_screen_menu(
    extent: GuiExtent,
    input: GuiInput,
    source: &MenuSource,
) -> Option<Menu> {
    match &*source.types {
        WorldTypes::Ready(types) => create_world_menu(
            extent,
            input,
            &source.language,
            &source.atlas.0.font,
            types,
            &source.form,
        ),
        WorldTypes::Loading | WorldTypes::Failed(_) => Some(Menu::new(
            widgets_of(extent, input, [])?,
            MenuBackground::Texture,
        )),
    }
}

fn last_button_key(source: &MenuSource) -> &'static str {
    if source.local_server.is_some() {
        "menu.returnToMenu"
    } else {
        "menu.disconnect"
    }
}

fn delete_name<'a>(source: &'a MenuSource) -> &'a str {
    let Some(folder) = source.delete_target.0.as_deref() else {
        return "";
    };
    let worlds = match &*source.worlds {
        WorldList::Ready(worlds) => worlds.as_slice(),
        WorldList::Reading | WorldList::Unavailable(_) => &[],
    };
    worlds
        .iter()
        .find(|world| world.folder_id == folder)
        .map_or(folder, |world| world.name())
}

fn reason_spans(source: &MenuSource) -> Vec<Span> {
    source
        .reason
        .0
        .as_ref()
        .map(|reason| vec![Span::new(reason.text(&source.language), WHITE)])
        .unwrap_or_default()
}

pub(crate) fn widgets_of(
    extent: GuiExtent,
    input: GuiInput,
    list: impl IntoIterator<Item = Widget>,
) -> Option<Widgets> {
    let mut widgets = Widgets::new(extent, input);
    for widget in list {
        widgets.push(widget).ok()?;
    }
    Some(widgets)
}

pub(crate) fn project_menu(
    source: MenuSource,
    menu: Option<ResMut<Menu>>,
    mut commands: Commands,
    mut shown: Local<Screen>,
    mut events: Local<Vec<WidgetEvent>>,
) {
    let screen = *source.screen;
    let left = std::mem::replace(&mut *shown, screen);
    let extent = GuiExtent::new(source.scene.size.width, source.scene.size.height);
    let Some(mut menu) = menu else {
        let cursor = source.scene.cursor;
        let input = GuiInput::new(GuiPoint::new(cursor.x, cursor.y));
        if let Some(mut built) = screen_menu(screen, extent, input, &source) {
            built
                .widgets
                .focus_initial(None, menu_clock(&source.time), &mut events);
            commands.insert_resource(built);
        }
        events.clear();
        return;
    };
    let reopened =
        left != screen || (matches!(screen, Screen::CreateWorld(_)) && source.types.is_changed());
    let rearranged = menu.widgets.screen() != extent
        || source.options.is_changed()
        || source.language.is_changed()
        || source.form.is_changed();
    if !reopened && !rearranged {
        return;
    }
    let Some(mut built) = screen_menu(screen, extent, menu.widgets.input(), &source) else {
        commands.remove_resource::<Menu>();
        return;
    };
    if reopened || !keeps_widgets(screen) {
        built.widgets.focus_initial(
            kept_focus(left, screen),
            menu_clock(&source.time),
            &mut events,
        );
        events.clear();
        *menu = built;
        return;
    }
    menu.widgets.set_screen(extent);
    for placed in built.widgets.iter() {
        menu.widgets.set_rect(placed.id(), placed.rect());
        if let Some(widget) = menu.widgets.get_mut(placed.id()) {
            widget.caption.clone_from(&placed.caption);
        }
    }
    menu.tooltips = built.tooltips;
    menu.hints = built.hints;
}
