use std::path::PathBuf;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_chat::{ChatScreen, Closed, Context, Key, Outcome, Tag, keeps_draft};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundCommandSuggestion;
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Bounded, VarInt, WritePacket};

use super::chat::{list_rows, per_page, row_at, row_under, wrap_width};
use super::font::{Font, Span};
use super::language::Language;
use super::scene::{GuiAtlas, GuiScene};
use super::text::{Wrap, text_spans, wrap};
use crate::chat::{Chat, ChatInput, ClientLookup, CommandHistoryFile, CommandWorld, context, send};
use crate::screen::{Screen, grab};

/// The lines of the chat log as the GUI wraps them, and the scroll of the open chat.
#[derive(Resource, Default)]
pub struct ChatLines(pub mcrs_minecraft_chat::ChatLines<Vec<Span>>);

/// The chat screen: its keys, its mouse and the lines of the log it shows.
pub struct ChatScreenPlugin;

impl Plugin for ChatScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatLines>()
            .add_systems(PreUpdate, chat_keys.after(InputSystems))
            .add_systems(Update, project_lines.after(crate::chat::log_messages));
    }
}

/// Wraps the messages of the log into lines: the new ones as they arrive, and every kept
/// message again when the font or the language changes (CHAT-LOG-001).
fn project_lines(
    chat: Res<Chat>,
    screen: Res<Screen>,
    language: Option<Res<Language>>,
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
    let font = &atlas.0.font;
    let wrap_message = |text: &Text, tag: Option<Tag>, rows: &mut Vec<Vec<Span>>| {
        let spans = text_spans(text, &language, 0xFFFF_FFFF);
        wrap(&spans, wrap_width(tag), Wrap::ChatLog, font, rows);
    };
    if changed {
        lines.0.rebuild(&chat.log, wrap_message);
    } else {
        lines
            .0
            .sync(&chat.log, *screen == Screen::Chat, wrap_message);
    }
}

fn modifiers(keys: &ButtonInput<KeyCode>) -> (bool, bool) {
    let control = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    (control, shift)
}

/// The key of the chat screen, or `None` for a key that types text or does nothing.
fn screen_key(code: KeyCode, control: bool, shift: bool) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::Tab => Key::Tab { back: shift },
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace { word: control },
        KeyCode::Delete => Key::Delete { word: control },
        KeyCode::ArrowLeft => Key::Left {
            word: control,
            select: shift,
        },
        KeyCode::ArrowRight => Key::Right {
            word: control,
            select: shift,
        },
        KeyCode::Home => Key::Home { select: shift },
        KeyCode::End => Key::End { select: shift },
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::KeyA if control => Key::SelectAll,
        KeyCode::KeyC if control => Key::Copy,
        KeyCode::KeyX if control => Key::Cut,
        _ => return None,
    })
}

#[derive(SystemParam)]
struct ChatKeys<'w, 's> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    scroll: Res<'w, AccumulatedMouseScroll>,
    clipboard: Option<ResMut<'w, bevy::clipboard::Clipboard>>,
    scene: Res<'w, GuiScene>,
    atlas: Option<Res<'w, GuiAtlas>>,
    last_cursor: Local<'s, IVec2>,
}

#[derive(SystemParam)]
struct ChatState<'w> {
    screen: ResMut<'w, Screen>,
    chat: ResMut<'w, Chat>,
    input: ResMut<'w, ChatInput>,
    history_file: Option<Res<'w, CommandHistoryFile>>,
    lines: ResMut<'w, ChatLines>,
}

/// The initial input of a key that opens the chat screen, and whether it is a draft
/// (CHAT-OPEN-001).
fn opening(
    keys: &ButtonInput<KeyCode>,
    screen: Screen,
    joined: bool,
    chat: &mut Chat,
) -> Option<(String, bool)> {
    if screen != Screen::None || !joined {
        return None;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        return Some(
            chat.draft
                .take()
                .map_or((String::new(), false), |draft| (draft, true)),
        );
    }
    if !keys.just_pressed(KeyCode::Slash) {
        return None;
    }
    Some(match chat.draft.take() {
        Some(draft) if draft.starts_with('/') => (draft, true),
        other => {
            chat.draft = other;
            ("/".to_owned(), false)
        }
    })
}

/// Opens the chat screen on its keys (CHAT-OPEN-001), and while it is open gives it every key,
/// so no game key fires under it (CHAT-KEY-001).
fn chat_keys(
    mut keys: ChatKeys,
    mut state: ChatState,
    world: CommandWorld,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut connection: Option<Single<&mut ClientConnection>>,
) {
    let chat = &mut *state.chat;
    let opened = if *state.screen == Screen::Chat {
        None
    } else {
        let opened = opening(&keys.keys, *state.screen, world.has_joined(), chat);
        let Some(opened) = opened else {
            return;
        };
        Some(opened)
    };
    let lookup = world.lookup();
    let source = Source::new(world.level(), &lookup);
    let cx = context(
        &world,
        &source,
        chat.abilities,
        chat.options,
        &chat.completions,
    );
    if let Some((initial, draft)) = opened {
        state.input.0.open(&initial, draft, &cx);
        flush_request(&mut state.input.0, &mut connection);
        *state.screen = Screen::Chat;
        grab(&mut cursor, false);
        keys.keyboard.clear();
        keys.keys.reset_all();
        return;
    }
    let (control, shift) = modifiers(&keys.keys);
    let screen = &mut state.input.0;
    let mut closed = None;
    let mut page = 0;
    for event in keys.keyboard.read() {
        if event.state != ButtonState::Pressed || closed.is_some() {
            continue;
        }
        let outcome = if let Some(key) = screen_key(event.key_code, control, shift) {
            screen.key(key, &mut chat.history, &cx)
        } else if control && event.key_code == KeyCode::KeyV {
            match keys.clipboard.as_deref_mut().map(|c| c.fetch_text()) {
                Some(bevy::clipboard::ClipboardRead::Ready(Ok(text))) => {
                    screen.type_text(&text, &cx)
                }
                _ => Outcome::Ignored,
            }
        } else if let Some(text) = event.text.as_ref().filter(|_| !control) {
            screen.type_text(text, &cx)
        } else {
            Outcome::Ignored
        };
        match outcome {
            Outcome::Closed(how) => closed = Some(how),
            Outcome::Page(direction) => page += direction,
            Outcome::Copy(text) => {
                if let Some(clipboard) = keys.clipboard.as_deref_mut() {
                    let _ = clipboard.set_text(text);
                }
            }
            Outcome::Ignored | Outcome::Handled => {}
        }
        flush_request(screen, &mut connection);
    }
    let moved = *keys.last_cursor != keys.scene.cursor;
    *keys.last_cursor = keys.scene.cursor;
    if let Some(atlas) = keys.atlas.as_deref() {
        mouse(
            &keys,
            moved,
            screen,
            &mut state.lines.0,
            shift,
            &atlas.0.font,
            &cx,
        );
        flush_request(screen, &mut connection);
    }
    if page != 0 {
        let lines = per_page(true).saturating_sub(1) as isize;
        state.lines.0.scroll_by(page * lines, per_page(true));
    }
    keys.keys.reset_all();
    if let Some(closed) = closed {
        close(
            &mut state,
            closed,
            connection.as_deref_mut().map(|c| &mut **c),
        );
        grab(&mut cursor, true);
    }
}

fn flush_request(screen: &mut ChatScreen, connection: &mut Option<Single<&mut ClientConnection>>) {
    if let Some(request) = screen.take_request()
        && let Some(connection) = connection.as_deref_mut()
    {
        connection.write_packet(&ServerboundCommandSuggestion {
            id: VarInt(request.id),
            text: Bounded(screen.input().get(..request.end).unwrap_or_default()),
        });
    }
}

/// Sends what Enter sent, keeps the draft and leaves the screen (CHAT-OPEN-002).
fn close(state: &mut ChatState, closed: Closed, connection: Option<&mut ClientConnection>) {
    let chat = &mut *state.chat;
    let screen = &state.input.0;
    if let Closed::Sent {
        send: Some(sent),
        history_changed,
    } = &closed
    {
        if let Some(connection) = connection {
            send(connection, sent);
        }
        if *history_changed && let Some(file) = &state.history_file {
            write_history(file.0.clone(), chat.history.command_file());
        }
    }
    chat.draft = keeps_draft(screen.input(), Some(&closed), chat.options.save_unsent)
        .then(|| screen.input().to_owned());
    state.lines.0.reset_scroll();
    *state.screen = Screen::None;
}

fn write_history(path: PathBuf, file: String) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            if let Err(error) = std::fs::write(&path, file) {
                warn!("cannot write {}: {error}", path.display());
            }
        })
        .detach();
}

/// The wheel and the left button over the open chat (CHAT-KEY-001, CCMD-LIST-007).
fn mouse(
    keys: &ChatKeys,
    moved: bool,
    screen: &mut ChatScreen,
    log: &mut mcrs_minecraft_chat::ChatLines<Vec<Span>>,
    shift: bool,
    font: &Font,
    cx: &Context<'_, ClientLookup<'_>>,
) {
    let scene = &*keys.scene;
    let rows = list_rows(scene.size, font, screen);
    let over = rows
        .as_ref()
        .and_then(|rows| row_under(rows.rect, scene.cursor).map(|row| rows.first + row));
    let hovered = rows.and_then(|rows| row_at(rows.rect, scene.cursor).map(|row| rows.first + row));
    let step = keys.scroll.delta.y.clamp(-1.0, 1.0) as isize;
    if step != 0 {
        match (over, screen.commands.list_mut()) {
            (Some(_), Some(list)) => list.scroll(step),
            _ => log.scroll_by(if shift { step } else { step * 7 }, per_page(true)),
        }
    }
    let Some(list) = screen.commands.list_mut() else {
        return;
    };
    if let Some(index) = hovered
        && moved
        && list.selected() != index
    {
        list.select(index as isize);
    }
    if let Some(index) = over.filter(|&index| index < list.entries().len())
        && keys.buttons.just_pressed(MouseButton::Left)
    {
        list.select(index as isize);
        screen.insert_suggestion(cx);
    }
}
