use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_chat_client::{
    ChatScreen, ClickMode, Closed, Context, History, HistoryChange, InputStyle, Key, Outcome,
    StyleClick, TabDirection, Tag, UnsentDraft,
};
use mcrs_minecraft_client_widget::{Anchor, DOUBLE_CLICK_MS, GuiPoint, KeyModifiers, Step};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_protocol_game::text::Text;

use crate::chat::{self as chat_view, ChatFocus};
use crate::chat_suggestions;
use crate::cursor_grab::grab;
use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::menu_input;
use crate::scene::{GuiAtlas, GuiScene};
use crate::span_events::SpanEvents;
use crate::text::{self, Wrap};
use mcrs_minecraft_client_world::chat::{
    self, Chat, ChatInput, ClientLookup, CommandWorld, SendChat,
};
use mcrs_minecraft_client_world::chat_history::CommandHistoryFile;
use mcrs_minecraft_client_world::screen::Screen;

#[derive(Resource, Default)]
pub struct ChatLines {
    pub lines: mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    pub events: SpanEvents,
}

pub struct ChatScreenPlugin;

impl Plugin for ChatScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatLines>()
            .add_systems(PreUpdate, chat_keys.after(InputSystems))
            .add_systems(
                Update,
                project_lines
                    .after(mcrs_minecraft_client_world::chat::log_messages)
                    .run_if(
                        resource_exists_and_changed::<Chat>
                            .or_else(resource_exists_and_changed::<ClientLanguage>)
                            .or_else(resource_exists_and_changed::<GuiAtlas>),
                    ),
            );
    }
}

fn project_lines(
    chat: Res<Chat>,
    screen: Res<Screen>,
    language: Option<Res<ClientLanguage>>,
    atlas: Option<Res<GuiAtlas>>,
    mut lines: ResMut<ChatLines>,
) {
    let (Some(language), Some(atlas)) = (language, atlas) else {
        return;
    };
    let changed = language.is_changed() || atlas.is_changed();
    if !changed && !chat.is_changed() {
        return;
    }
    let open = (*screen == Screen::Chat).then(|| chat_view::per_page(ChatFocus::Focused));
    project(&chat, open, &language, &atlas.0.font, changed, &mut lines);
}

fn project(
    chat: &Chat,
    open: Option<usize>,
    language: &ClientLanguage,
    font: &Font,
    rebuild: bool,
    lines: &mut ChatLines,
) {
    let ChatLines { lines, events } = lines;
    if rebuild {
        events.clear();
    }
    let wrap_message = |text: &Text, tag: Option<Tag>, rows: &mut Vec<Vec<Span>>| {
        let spans = text::text_spans_with_events(text, language, WHITE, events);
        text::wrap(
            &spans,
            chat_view::wrap_width(tag),
            Wrap::ChatLog,
            font,
            rows,
        );
    };
    if rebuild {
        lines.rebuild(&chat.log, wrap_message);
    } else {
        lines.sync(&chat.log, open, wrap_message);
    }
    events.retain_referenced(lines.lines().flat_map(|line| line.content.iter()));
}

const MODIFIER_KEYS: [KeyCode; 8] = [
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::SuperLeft,
    KeyCode::SuperRight,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
];

fn capture_keys(keys: &mut ButtonInput<KeyCode>) {
    let held = MODIFIER_KEYS.map(|key| keys.pressed(key));
    keys.reset_all();
    for (key, held) in MODIFIER_KEYS.into_iter().zip(held) {
        if held {
            keys.press(key);
        }
    }
    keys.clear();
}

pub(crate) fn screen_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    let step = if modifiers.control {
        Step::Word
    } else {
        Step::Char
    };
    let anchor = if modifiers.shift {
        Anchor::Keep
    } else {
        Anchor::Move
    };
    Some(match code {
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::Tab => Key::Tab {
            direction: if modifiers.shift {
                TabDirection::Backward
            } else {
                TabDirection::Forward
            },
        },
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace { step },
        KeyCode::Delete => Key::Delete { step },
        KeyCode::ArrowLeft => Key::Left { step, anchor },
        KeyCode::ArrowRight => Key::Right { step, anchor },
        KeyCode::Home => Key::Home { anchor },
        KeyCode::End => Key::End { anchor },
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::KeyA if modifiers.is_shortcut() => Key::SelectAll,
        KeyCode::KeyC if modifiers.is_shortcut() => Key::Copy,
        KeyCode::KeyX if modifiers.is_shortcut() => Key::Cut,
        _ => return None,
    })
}

pub(crate) const CHAT: KeyCode = KeyCode::KeyT;

pub(crate) fn pastes(code: KeyCode, modifiers: KeyModifiers) -> bool {
    modifiers.is_shortcut() && code == KeyCode::KeyV
}

pub(crate) fn clipboard_text(clipboard: Option<&mut Clipboard>) -> Option<String> {
    match clipboard?.fetch_text() {
        ClipboardRead::Ready(Ok(text)) => Some(text),
        _ => None,
    }
}

pub(crate) fn copy_text(clipboard: Option<&mut Clipboard>, text: &str) {
    if let Some(clipboard) = clipboard {
        let _ = clipboard.set_text(text);
    }
}

#[derive(SystemParam)]
struct ChatKeys<'w, 's> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    scroll: Res<'w, AccumulatedMouseScroll>,
    clipboard: Option<ResMut<'w, Clipboard>>,
    scene: Res<'w, GuiScene>,
    last_cursor: Local<'s, IVec2>,
    input_dragging: Local<'s, bool>,
    last_input_click: Local<'s, Option<u64>>,
}

#[derive(SystemParam)]
struct ChatState<'w> {
    screen: ResMut<'w, Screen>,
    chat: ResMut<'w, Chat>,
    input: ResMut<'w, ChatInput>,
    history_file: Option<ResMut<'w, CommandHistoryFile>>,
    lines: ResMut<'w, ChatLines>,
}

fn opening(
    keys: &ButtonInput<KeyCode>,
    screen: Screen,
    joined: bool,
    chat: &mut Chat,
) -> Option<(String, InputStyle)> {
    if screen != Screen::None || !joined {
        return None;
    }
    if keys.just_pressed(CHAT) {
        return Some(
            chat.draft
                .take()
                .map_or((String::new(), InputStyle::Plain), |draft| {
                    (draft, InputStyle::Draft)
                }),
        );
    }
    if !keys.just_pressed(KeyCode::Slash) {
        return None;
    }
    Some(match chat.draft.take() {
        Some(draft) if draft.starts_with('/') => (draft, InputStyle::Draft),
        other => {
            chat.draft = other;
            ("/".to_owned(), InputStyle::Plain)
        }
    })
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only when the chat opens or closes"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(3))]
fn chat_keys(
    mut keys: ChatKeys,
    mut state: ChatState,
    world: CommandWorld,
    atlas: Option<Res<GuiAtlas>>,
    time: Res<Time<Real>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut sends: MessageWriter<SendChat>,
) {
    let chat = &mut *state.chat;
    let opened = if *state.screen == Screen::Chat {
        None
    } else if let opened @ Some(_) = opening(&keys.keys, *state.screen, world.has_joined(), chat) {
        opened
    } else {
        return;
    };
    let now_ms = menu_input::menu_clock(&time);
    let lookup = world.lookup();
    let source = Source::new(world.level(), &lookup);
    let cx = chat::context(
        &world,
        &source,
        chat.abilities,
        chat.options,
        &chat.completions,
    );
    let width = |text: &str| {
        atlas
            .as_deref()
            .map_or(0, |atlas| atlas.0.font.str_width(text))
    };
    let rect = chat_view::input_bounds(keys.scene.size);
    if let Some((initial, style)) = opened {
        state.input.0.open(&initial, style, now_ms, &cx);
        state.input.0.scroll_input(rect, &width);
        state.screen.set_if_neq(Screen::Chat);
        grab(&mut cursor, false);
        keys.keyboard.clear();
        *keys.input_dragging = false;
        *keys.last_input_click = None;
        capture_keys(&mut keys.keys);
        return;
    }
    let modifiers = menu_input::modifiers(&keys.keys);
    let screen = &mut state.input.0;
    let typed = type_keys(&mut keys, modifiers, screen, &mut chat.history, &cx);
    if typed.scroll {
        screen.scroll_input(rect, &width);
    }
    let moved = *keys.last_cursor != keys.scene.cursor;
    *keys.last_cursor = keys.scene.cursor;
    if let Some(atlas) = atlas.as_deref() {
        let font = &atlas.0.font;
        input_mouse(
            &mut keys,
            screen,
            moved,
            modifiers.shift,
            font,
            menu_input::menu_clock(&time),
        );
        let changed = mouse(
            &keys,
            moved,
            screen,
            &mut state.lines,
            modifiers.shift,
            font,
            &cx,
        );
        if changed {
            screen.scroll_input(rect, &width);
        }
    }
    if typed.page != 0 {
        let page = chat_view::per_page(ChatFocus::Focused);
        let lines = page.saturating_sub(1) as isize;
        state.lines.lines.scroll_by(typed.page * lines, page);
    }
    capture_keys(&mut keys.keys);
    if let Some(closed) = typed.closed {
        close(&mut state, closed, &mut sends);
        grab(&mut cursor, true);
    }
}

struct Typed {
    closed: Option<Closed>,
    page: isize,
    scroll: bool,
}

fn type_keys(
    keys: &mut ChatKeys,
    modifiers: KeyModifiers,
    screen: &mut ChatScreen,
    history: &mut History,
    cx: &Context<'_, ClientLookup<'_>>,
) -> Typed {
    let control = modifiers.control;
    let mut typed = Typed {
        closed: None,
        page: 0,
        scroll: false,
    };
    for event in keys.keyboard.read() {
        if event.state != ButtonState::Pressed || typed.closed.is_some() {
            continue;
        }
        let key = screen_key(event.key_code, modifiers);
        let outcome = if let Some(key) = key {
            screen.key(key, history, cx)
        } else if pastes(event.key_code, modifiers) {
            match clipboard_text(keys.clipboard.as_deref_mut()) {
                Some(text) => screen.type_text(&text, cx),
                None => Outcome::Ignored,
            }
        } else if let Some(text) = event.text.as_ref().filter(|_| !control) {
            screen.type_text(text, cx)
        } else {
            Outcome::Ignored
        };
        if !matches!(
            outcome,
            Outcome::Ignored | Outcome::Copy(_) | Outcome::Page(_)
        ) {
            typed.scroll = key != Some(Key::SelectAll);
        }
        match outcome {
            Outcome::Closed(how) => typed.closed = Some(how),
            Outcome::Page(direction) => typed.page += direction,
            Outcome::Copy(text) => copy_text(keys.clipboard.as_deref_mut(), &text),
            _ => {}
        }
    }
    typed
}

fn close(state: &mut ChatState, closed: Closed, sends: &mut MessageWriter<SendChat>) {
    let chat = &mut *state.chat;
    let screen = &state.input.0;
    if let Closed::Sent {
        send: Some(sent),
        history,
    } = &closed
    {
        sends.write(SendChat(sent.clone()));
        if *history == HistoryChange::Changed
            && let Some(file) = &mut state.history_file
        {
            file.save(chat.history.command_file());
        }
    }
    let unsent = if chat.options.save_unsent {
        UnsentDraft::Save
    } else {
        UnsentDraft::Discard
    };
    chat.draft = mcrs_minecraft_chat_client::keeps_draft(screen.input(), Some(&closed), unsent)
        .then(|| screen.input().to_owned());
    state.lines.lines.reset_scroll();
    state.screen.set_if_neq(Screen::None);
}

fn input_mouse(
    keys: &mut ChatKeys,
    screen: &mut ChatScreen,
    moved: bool,
    shift: bool,
    font: &Font,
    now_ms: u64,
) {
    let cursor = keys.scene.cursor;
    let size = keys.scene.size;
    let rect = chat_view::input_bounds(size);
    let width = |text: &str| font.str_width(text);
    if !keys.buttons.pressed(MouseButton::Left) {
        *keys.input_dragging = false;
    }
    if keys.buttons.just_pressed(MouseButton::Left) {
        if !rect.contains_point(GuiPoint::new(cursor.x, cursor.y)) {
            *keys.input_dragging = false;
            *keys.last_input_click = None;
            return;
        }
        let double = keys
            .last_input_click
            .is_some_and(|previous| now_ms.saturating_sub(previous) <= DOUBLE_CLICK_MS);
        *keys.last_input_click = Some(now_ms);
        *keys.input_dragging = true;
        if double {
            screen.select_input_word(rect, f64::from(cursor.x), &width);
        } else {
            let anchor = if shift { Anchor::Keep } else { Anchor::Move };
            screen.click_input(rect, f64::from(cursor.x), anchor, &width);
        }
    } else if *keys.input_dragging && moved && keys.buttons.pressed(MouseButton::Left) {
        screen.drag_input(rect, f64::from(cursor.x), &width);
    }
}

fn mouse(
    keys: &ChatKeys,
    moved: bool,
    screen: &mut ChatScreen,
    log: &mut ChatLines,
    shift: bool,
    font: &Font,
    cx: &Context<'_, ClientLookup<'_>>,
) -> bool {
    let scene = &*keys.scene;
    let left = keys.buttons.just_pressed(MouseButton::Left) && !*keys.input_dragging;
    let rows = chat_suggestions::list_rows(scene.size, font, screen);
    let over = rows.as_ref().and_then(|rows| {
        chat_suggestions::row_under(rows.rect, scene.cursor).map(|row| rows.first + row)
    });
    let hovered = rows.and_then(|rows| {
        chat_suggestions::row_at(rows.rect, scene.cursor).map(|row| rows.first + row)
    });
    let step = keys.scroll.delta.y.clamp(-1.0, 1.0);
    match (over, screen.commands.list_mut()) {
        (Some(_), Some(list)) => {
            if step as isize != 0 {
                list.scroll(step as isize);
            }
        }
        _ => {
            let lines = if shift { step } else { step * 7.0 } as isize;
            if lines != 0 {
                log.lines
                    .scroll_by(lines, chat_view::per_page(ChatFocus::Focused));
            }
        }
    }
    let mut inserted = false;
    if let Some(list) = screen.commands.list_mut() {
        if let Some(index) = hovered
            && moved
            && list.selected() != index
        {
            list.select(index as isize);
        }
        if let Some(index) = over.filter(|&index| index < list.entries().len())
            && left
        {
            list.select(index as isize);
            screen.insert_suggestion(cx);
            inserted = true;
        }
    }
    if inserted {
        return true;
    }
    if over.is_some() || !left {
        return false;
    }
    let Some(span) = chat_view::open_chat_span_at(scene.size, &log.lines, scene.cursor, font)
    else {
        return false;
    };
    let style = StyleClick {
        insertion: span.insertion.as_deref(),
        event: span.click.and_then(|index| log.events.click(index)),
    };
    let mode = if shift {
        ClickMode::Shift
    } else {
        ClickMode::Plain
    };
    !matches!(screen.click_style(style, mode, cx), Outcome::Ignored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_text::IntoText;

    #[test]
    fn trimmed_events_keep_the_scroll_and_stay_bounded() {
        let mut chat = Chat::default();
        let mut lines = ChatLines::default();
        let language = ClientLanguage::default();
        let font = Font::default();
        let open = Some(chat_view::per_page(ChatFocus::Focused));
        for tick in 0..30 {
            chat.log.push(
                Text::text(format!("m{tick}")).on_hover_show_text("tip"),
                tick,
                None,
            );
        }
        project(&chat, open, &language, &font, true, &mut lines);
        lines
            .lines
            .scroll_by(5, chat_view::per_page(ChatFocus::Focused));
        let scroll = lines.lines.scroll();
        assert_eq!(scroll, 5);
        for tick in 30..330 {
            chat.log.push(
                Text::text(format!("m{tick}")).on_hover_show_text("tip"),
                tick,
                None,
            );
            project(&chat, open, &language, &font, false, &mut lines);
            assert!(
                lines.events.len() <= 100,
                "events follow the 100 line limit"
            );
        }
        let newest = lines
            .lines
            .lines()
            .next()
            .and_then(|line| line.content.first());
        let hover = newest.and_then(|span| span.hover);
        assert!(hover.and_then(|index| lines.events.hover(index)).is_some());
        assert_eq!(
            lines.lines.scroll(),
            100 - chat_view::per_page(ChatFocus::Focused)
        );
    }

    #[test]
    fn held_modifiers_survive_chat_capture_across_frames() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ControlLeft);
        capture_keys(&mut keys);

        keys.press(KeyCode::KeyA);
        let modifiers = menu_input::modifiers(&keys);
        assert_eq!(screen_key(KeyCode::KeyA, modifiers), Some(Key::SelectAll));
        assert_eq!(
            screen_key(KeyCode::Backspace, modifiers),
            Some(Key::Backspace { step: Step::Word })
        );
        capture_keys(&mut keys);
        assert!(!keys.pressed(KeyCode::KeyA), "chat captures the typed key");
        assert!(
            keys.pressed(KeyCode::ControlLeft),
            "the held modifier remains down"
        );
        assert!(
            !keys.just_pressed(KeyCode::ControlLeft),
            "capture does not synthesize a press"
        );

        keys.release(KeyCode::ControlLeft);
        keys.press(KeyCode::ShiftLeft);
        capture_keys(&mut keys);
        assert_eq!(
            screen_key(KeyCode::ArrowLeft, menu_input::modifiers(&keys)),
            Some(Key::Left {
                step: Step::Char,
                anchor: Anchor::Keep
            })
        );
        keys.release(KeyCode::ShiftLeft);
        capture_keys(&mut keys);
        assert!(
            !keys.pressed(KeyCode::ShiftLeft),
            "released modifiers do not remain stuck"
        );
    }
}
