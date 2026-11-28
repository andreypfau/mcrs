use bevy::math::IVec2;
use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::WidgetId;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_save::{ListedLevel, WorldSummary};
use time::{OffsetDateTime, UtcOffset};

use crate::gui_quad::WHITE;
use crate::gui_texture_id::GuiTextureId;
use crate::menu::{RowContent, RowLine};
use mcrs_minecraft_font::font::{Font, Span};
use mcrs_minecraft_font::language::ClientLanguage;

const TEXT_X: i32 = 35;
const NAME_Y: i32 = 1;
const FOLDER_Y: i32 = 12;
const INFO_Y: i32 = 21;
const MAX_TEXT_WIDTH: i32 = 231;
const ELLIPSIS: &str = "...";
const GREY: u32 = 0xFF80_8080;
const WARNING: u32 = 0xFFFF_5555;
const FAILURE: u32 = 0xFFFF_0000;

const LOADING_Y: i32 = 11;
const DOTS_GAP: i32 = 9;
pub(crate) const DOTS_FRAME_MS: u64 = 300;
const DOTS: [&str; 4] = ["O o o", "o O o", "o o O", "o O o"];

#[derive(Resource, Clone, Copy)]
pub(crate) struct LocalOffset(pub(crate) UtcOffset);

pub(crate) fn world_entry(
    list: WidgetId,
    row: usize,
    world: &WorldSummary,
    language: &ClientLanguage,
    font: &Font,
    offset: UtcOffset,
) -> RowContent {
    let folder = match world
        .last_played
        .and_then(|played| entry_date(played, offset))
    {
        Some(date) => format!("{} ({date})", world.folder_id),
        None => world.folder_id.clone(),
    };
    let line = |y, spans| RowLine {
        offset: IVec2::new(TEXT_X, y),
        spans: cut(font, spans),
    };
    RowContent {
        list,
        row,
        icon: Some(GuiTextureId::UnknownServer),
        lines: vec![
            line(NAME_Y, vec![Span::new(world.name(), WHITE)]),
            line(FOLDER_Y, vec![Span::new(folder, GREY)]),
            line(INFO_Y, info_line(world, language)),
        ],
    }
}

pub(crate) fn loading_entry(
    list: WidgetId,
    row: usize,
    inner: IVec2,
    width: i32,
    language: &ClientLanguage,
    font: &Font,
    now_ms: u64,
) -> RowContent {
    let frame = usize::try_from(now_ms / DOTS_FRAME_MS % 4).unwrap_or(0);
    let dots = DOTS.get(frame).copied().unwrap_or_default();
    let centred = |text: &str, color, y| {
        let x = (width - font.str_width(text)) / 2;
        RowLine {
            offset: IVec2::new(x, y) - inner.with_y(0),
            spans: vec![Span::new(text, color)],
        }
    };
    let text = language.get("selectWorld.loading_list");
    RowContent {
        list,
        row,
        icon: None,
        lines: vec![
            centred(text, WHITE, LOADING_Y),
            centred(dots, GREY, LOADING_Y + DOTS_GAP),
        ],
    }
}

fn info_line(world: &WorldSummary, language: &ClientLanguage) -> Vec<Span> {
    let Some(level) = &world.level else {
        return vec![Span::new(language.get("recover_world.warning"), FAILURE)];
    };
    let version = if level.version_name.is_empty() {
        language.get("selectWorld.versionUnknown")
    } else {
        level.version_name.as_str()
    };
    if world.locked {
        return vec![Span::new(language.get("selectWorld.locked"), WARNING)];
    }
    if world.is_incompatible() {
        let version = vec![Span::new(version, WARNING)];
        return language.translate("selectWorld.incompatible.info", WARNING, &[version]);
    }
    let mut parts = vec![game_mode(level, language)];
    if level.allow_commands {
        parts.push(Span::new(
            format!(", {}", language.get("selectWorld.commands")),
            GREY,
        ));
    }
    parts.push(Span::new(
        format!(", {} {version}", language.get("selectWorld.version")),
        GREY,
    ));
    parts
}

fn game_mode(level: &ListedLevel, language: &ClientLanguage) -> Span {
    if level.hardcore {
        return Span::new(language.get("gameMode.hardcore"), FAILURE);
    }
    let mode = GameMode::from_id(level.game_type).unwrap_or_default();
    Span::new(language.get(mode.translation_key()), GREY)
}

fn entry_date(millis: i64, offset: UtcOffset) -> Option<String> {
    let utc = OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).ok()?;
    let at = utc.checked_to_offset(offset)?;
    let hour = match at.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    let half = if at.hour() < 12 { "AM" } else { "PM" };
    Some(format!(
        "{}/{}/{:02}, {hour}:{:02} {half}",
        u8::from(at.month()),
        at.day(),
        at.year().rem_euclid(100),
        at.minute(),
    ))
}

fn cut(font: &Font, spans: Vec<Span>) -> Vec<Span> {
    if font.width(&spans) <= MAX_TEXT_WIDTH {
        return spans;
    }
    let room = (MAX_TEXT_WIDTH - font.str_width(ELLIPSIS)) as f32;
    let mut width = 0.0;
    let mut kept = Vec::new();
    for mut span in spans {
        let end = span.text.char_indices().find_map(|(index, ch)| {
            width += font.advance(ch, span.bold);
            (width > room).then_some(index)
        });
        let color = span.color;
        if let Some(end) = end {
            span.text.truncate(end);
        }
        if !span.text.is_empty() {
            kept.push(span);
        }
        if end.is_some() {
            kept.push(Span::new(ELLIPSIS, color));
            break;
        }
    }
    kept
}
