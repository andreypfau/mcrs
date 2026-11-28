use crate::gui_band::GuiBand;
use crate::gui_quad::GuiQuad;
use crate::scene_icon::SceneIcon;
use bevy::prelude::*;

pub(crate) type GuiLayerOrder = (GuiBand, u8);

#[derive(Component, Debug)]
pub struct GuiLayer {
    pub band: GuiBand,
    pub part: u8,
    quads: Vec<GuiQuad>,
    icons: Vec<SceneIcon>,
    redraw_at_ms: Option<u64>,
}

impl GuiLayer {
    #[must_use]
    pub const fn new(band: GuiBand, part: u8) -> Self {
        Self {
            band,
            part,
            quads: Vec::new(),
            icons: Vec::new(),
            redraw_at_ms: None,
        }
    }

    #[must_use]
    pub(crate) const fn order(&self) -> GuiLayerOrder {
        (self.band, self.part)
    }

    pub(crate) fn drawn(&self) -> impl Iterator<Item = (GuiQuad, Option<&SceneIcon>)> {
        self.quads.iter().map(|&quad| {
            let icon = match quad {
                GuiQuad::Icon { index, .. } => self.icons.get(index),
                _ => None,
            };
            (quad, icon)
        })
    }

    #[must_use]
    pub fn quads(&self) -> &[GuiQuad] {
        &self.quads
    }

    #[must_use]
    pub fn icons(&self) -> &[SceneIcon] {
        &self.icons
    }

    pub fn redraw(&mut self) -> &mut Vec<GuiQuad> {
        self.quads.clear();
        self.icons.clear();
        self.redraw_at_ms = None;
        &mut self.quads
    }

    pub fn push(&mut self, quad: GuiQuad) {
        self.quads.push(quad);
    }

    pub fn push_icon(&mut self, origin: IVec2, icon: SceneIcon) {
        let index = self.icons.len();
        self.icons.push(icon);
        self.quads.push(GuiQuad::Icon { origin, index });
    }

    pub fn redraw_at(&mut self, ms: u64) {
        self.redraw_at_ms = Some(self.redraw_at_ms.map_or(ms, |current| current.min(ms)));
    }

    #[must_use]
    pub fn due(&self, now_ms: u64) -> bool {
        self.redraw_at_ms.is_some_and(|at| at <= now_ms)
    }
}

pub(crate) fn spawn_layer<Kind: Component + Default>(mut commands: Commands) {
    commands.spawn(Kind::default());
}
