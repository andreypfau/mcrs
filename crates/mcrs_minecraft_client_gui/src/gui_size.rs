use crate::gui_config::GuiConfig;
use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiSize {
    pub width: i32,
    pub height: i32,
    pub scale: u32,
}

impl GuiSize {
    pub fn auto_scale(framebuffer: UVec2) -> u32 {
        (framebuffer.x / 320).min(framebuffer.y / 240).max(1)
    }

    pub fn of(framebuffer: UVec2, scale: u32) -> Self {
        let auto = Self::auto_scale(framebuffer);
        let scale = if scale == 0 { auto } else { scale.min(auto) };
        Self {
            width: framebuffer.x.div_ceil(scale) as i32,
            height: framebuffer.y.div_ceil(scale) as i32,
            scale,
        }
    }
}

pub(crate) fn write_gui_size(
    window: Single<&Window, With<PrimaryWindow>>,
    config: Res<GuiConfig>,
    mut size: ResMut<GuiSize>,
) {
    size.set_if_neq(GuiSize::of(window_framebuffer(&window), config.scale));
}

pub(crate) fn window_framebuffer(window: &Window) -> UVec2 {
    UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_size_rounds_up_and_auto_scale_keeps_320x240() {
        assert_eq!(
            GuiSize::of(UVec2::new(855, 480), 2),
            GuiSize {
                width: 428,
                height: 240,
                scale: 2
            }
        );
        assert_eq!(GuiSize::auto_scale(UVec2::new(854, 480)), 2);
        assert_eq!(GuiSize::auto_scale(UVec2::new(1920, 1080)), 4);
        assert_eq!(GuiSize::auto_scale(UVec2::new(300, 200)), 1);
        assert_eq!(GuiSize::of(UVec2::new(1920, 1080), 0).scale, 4);
        assert_eq!(
            GuiSize::of(UVec2::new(854, 480), 3),
            GuiSize {
                width: 427,
                height: 240,
                scale: 2
            }
        );
    }
}
