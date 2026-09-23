use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;

use super::font::{Font, Span, draw_text};
use super::language::Language;
use super::scene::{GuiAtlas, GuiQuad, GuiScene, GuiSize};

const HISTORY: usize = 100;
const LINES_SHOWN: usize = 10;
const LINE_HEIGHT: i32 = 9;
const BOTTOM_MARGIN: i32 = 40;
const WIDTH: i32 = 320;
const LIFETIME_TICKS: f64 = 200.0;
const BACKGROUND_OPACITY: f32 = 0.5;
const YELLOW: u32 = 0xFFFF_FF55;
const WHITE: u32 = 0xFFFF_FFFF;

struct Message {
    spans: Vec<Span>,
    added: Duration,
}

/// The client-side chat lines the debug keys report into. This client has no
/// chat of its own yet, so only these lines are drawn, where vanilla's chat
/// draws them, fading out the same way.
#[derive(Resource, Default)]
pub struct DebugChat {
    messages: VecDeque<Message>,
}

impl DebugChat {
    pub fn feedback(&mut self, language: &Language, key: &str, now: Duration) {
        let mut spans = vec![
            Span::new(language.get("debug.prefix"), YELLOW).bold(),
            Span::new(" ", WHITE),
        ];
        spans.extend(language.translate(key, WHITE, &[]));
        info!(
            "{}",
            spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
        );
        self.messages.push_front(Message { spans, added: now });
        self.messages.truncate(HISTORY);
    }
}

#[cfg(test)]
impl DebugChat {
    pub(crate) fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }
}

pub fn fade(age: Duration) -> f32 {
    let ticks = age.as_secs_f64() * 20.0;
    let t = ((1.0 - ticks / LIFETIME_TICKS) * 10.0).clamp(0.0, 1.0);
    (t * t) as f32
}

fn with_alpha(color: u32, alpha: f32) -> u32 {
    let base = f32::from((color >> 24) as u8);
    ((base * alpha) as u32) << 24 | (color & 0x00FF_FFFF)
}

fn chat_lines(size: GuiSize, font: &Font, chat: &DebugChat, now: Duration, out: &mut Vec<GuiQuad>) {
    let chat_bottom = size.height - BOTTOM_MARGIN;
    let right = (WIDTH + 12).min(size.width);
    for (line, message) in chat.messages.iter().take(LINES_SHOWN).enumerate() {
        let alpha = fade(now.saturating_sub(message.added));
        if alpha <= 1e-5 {
            continue;
        }
        let bottom = chat_bottom - line as i32 * LINE_HEIGHT;
        out.push(GuiQuad::Fill {
            rect: IRect::new(0, bottom - LINE_HEIGHT, right, bottom),
            color: with_alpha(0xFF00_0000, alpha * BACKGROUND_OPACITY),
        });
        let spans: Vec<_> = message
            .spans
            .iter()
            .map(|span| Span {
                color: with_alpha(span.color, alpha),
                ..span.clone()
            })
            .collect();
        draw_text(font, IVec2::new(4, bottom - 8), &spans, out);
    }
}

pub fn draw(
    chat: Res<DebugChat>,
    atlas: Option<Res<GuiAtlas>>,
    time: Res<Time<Real>>,
    mut scene: ResMut<GuiScene>,
) {
    let Some(atlas) = atlas else { return };
    let scene = &mut *scene;
    chat_lines(
        scene.size,
        &atlas.0.font,
        &chat,
        time.elapsed(),
        &mut scene.quads,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::font::corpus_font;

    #[test]
    fn a_line_holds_for_nine_seconds_and_fades_over_the_tenth() {
        assert_eq!(fade(Duration::ZERO), 1.0);
        assert_eq!(fade(Duration::from_secs(9)), 1.0);
        assert!((fade(Duration::from_millis(9500)) - 0.25).abs() < 1e-6);
        assert_eq!(fade(Duration::from_secs(10)), 0.0);
    }

    #[test]
    fn the_newest_message_sits_forty_pixels_above_the_bottom() {
        let mut chat = DebugChat::default();
        chat.feedback(Language::corpus(), "debug.gamemodes.error", Duration::ZERO);
        let size = GuiSize {
            width: 400,
            height: 240,
            scale: 2,
        };
        let mut out = Vec::new();
        chat_lines(size, corpus_font(), &chat, Duration::from_secs(1), &mut out);
        assert_eq!(
            out[0],
            GuiQuad::Fill {
                rect: IRect::new(0, 191, 332, 200),
                color: 0x7F00_0000
            }
        );
        let text: String = chat.messages[0]
            .spans
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(
            text,
            "[Debug]: Unable to open game mode switcher; no permission"
        );
        assert!(matches!(
            out[1],
            GuiQuad::Glyph { origin, glyph: '[', color: 0xFF3F_3F15 } if origin == IVec2::new(5, 193)
        ));

        out.clear();
        chat_lines(
            size,
            corpus_font(),
            &chat,
            Duration::from_secs(11),
            &mut out,
        );
        assert!(out.is_empty());
    }
}
