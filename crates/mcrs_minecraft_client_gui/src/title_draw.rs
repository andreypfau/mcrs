use std::hash::{BuildHasher, RandomState};

use bevy::math::{IRect, Rect, Vec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::GuiExtent;

use crate::font::draw_str;
use crate::gui_texture_id::GuiTextureId;
use crate::item_decorations::WHITE;
use crate::menu_input::menu_clock;
use crate::scene::{GuiAtlas, GuiAtlasData, GuiQuad, GuiScene};
use crate::title_screen::{TitleText, notice_lines, version_text};

const FADE_DELAY_MS: u64 = 1000;
const FADE_MS: u64 = 1000;
const MINCERAFT_ONE_IN: u64 = 10_000;

#[derive(Resource, Default)]
pub(crate) struct TitleScreenState {
    logo: Option<GuiTextureId>,
    fade: Fade,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Fade {
    #[default]
    Waiting,
    Since(u64),
    Over,
}

impl TitleScreenState {
    pub(crate) fn end_fade(&mut self) {
        self.fade = Fade::Over;
    }

    fn alpha(&mut self, now_ms: u64) -> u32 {
        let start = match self.fade {
            Fade::Waiting => {
                self.fade = Fade::Since(now_ms);
                now_ms
            }
            Fade::Since(start) => start,
            Fade::Over => return 255,
        };
        let elapsed = now_ms.saturating_sub(start);
        if elapsed > FADE_DELAY_MS + FADE_MS {
            self.fade = Fade::Over;
            return 255;
        }
        let shown = elapsed.saturating_sub(FADE_DELAY_MS).min(FADE_MS);
        u32::try_from((shown * 255).div_ceil(FADE_MS)).unwrap_or(255)
    }
}

#[derive(Default)]
pub(crate) struct TitleTexts {
    screen: Option<GuiExtent>,
    lines: Vec<TitleText>,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "caches the notice and version lines, so it allocates only when the GUI size changes"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(8))]
pub(crate) fn draw(
    mut state: ResMut<TitleScreenState>,
    atlas: Option<Res<GuiAtlas>>,
    mut scene: ResMut<GuiScene>,
    mut texts: Local<TitleTexts>,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let atlas = &atlas.0;
    let scene = &mut *scene;
    let screen = GuiExtent::new(scene.size.width, scene.size.height);
    if texts.screen != Some(screen) {
        texts.lines = notice_lines(screen, &atlas.font);
        texts.lines.push(version_text(screen, &atlas.font));
        texts.screen = Some(screen);
    }
    let logo = *state.logo.get_or_insert_with(|| {
        if RandomState::new()
            .hash_one(0u8)
            .is_multiple_of(MINCERAFT_ONE_IN)
        {
            GuiTextureId::TitleMinceraft
        } else {
            GuiTextureId::TitleLogo
        }
    });
    let out = &mut scene.quads;
    let centre = screen.width / 2;
    draw_top(
        atlas,
        logo,
        IRect::new(centre - 128, 30, centre + 128, 74),
        44.0 / 64.0,
        out,
    );
    let edition = IRect::new(centre - 64, 67, centre + 64, 81);
    draw_top(atlas, GuiTextureId::TitleEdition, edition, 14.0 / 16.0, out);
    for TitleText { text, at } in &texts.lines {
        let origin = Vec2::new(at.x as f32, at.y as f32);
        draw_str(&atlas.font, origin, text, WHITE, false, out);
    }
}

pub(crate) fn fade_title(
    mut state: ResMut<TitleScreenState>,
    time: Res<Time<Real>>,
    mut scene: ResMut<GuiScene>,
) {
    let alpha = state.alpha(menu_clock(&time));
    if alpha < 255 {
        for quad in scene.quads.iter_mut() {
            fade(quad, alpha);
        }
    }
}

fn draw_top(
    atlas: &GuiAtlasData,
    texture: GuiTextureId,
    rect: IRect,
    share: f32,
    out: &mut Vec<GuiQuad>,
) {
    let Some(region) = atlas.textures.get(texture as usize) else {
        return;
    };
    let region = region.as_rect();
    let texels = Rect::new(
        region.min.x,
        region.min.y,
        region.max.x,
        region.min.y + region.height() * share,
    );
    out.push(GuiQuad::Texels {
        rect,
        texels,
        color: WHITE,
    });
}

fn fade(quad: &mut GuiQuad, alpha: u32) {
    let scale = |color: &mut u32| {
        let faded = (*color >> 24) * alpha / 255;
        *color = (*color & 0x00FF_FFFF) | (faded << 24);
    };
    match quad {
        GuiQuad::Sprite { color, .. }
        | GuiQuad::Fill { color, .. }
        | GuiQuad::Texels { color, .. }
        | GuiQuad::Glyph { color, .. } => scale(color),
        GuiQuad::FillGradient { top, bottom, .. } => {
            scale(top);
            scale(bottom);
        }
        GuiQuad::Icon { .. }
        | GuiQuad::Figure { .. }
        | GuiQuad::Highlight { .. }
        | GuiQuad::Clip { .. }
        | GuiQuad::EndClip => {}
    }
}
