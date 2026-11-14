use core::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::OptionsError;

/// Degrees.
pub const FOV_RANGE: RangeInclusive<u8> = 30..=110;
/// Columns.
pub const VIEW_DISTANCE_RANGE: RangeInclusive<u8> = 2..=96;
/// Columns; the view distance of `None`.
pub const VIEW_DISTANCE_DEFAULT: u8 = 96;
/// Frames per second; the end sets no limit.
pub const MAX_FRAMERATE_RANGE: RangeInclusive<u16> = 10..=260;
/// Frames per second between two values of `max_framerate`.
pub const MAX_FRAMERATE_STEP: u16 = 10;

/// The video options of the client, with the meaning of the vanilla option of the same name. In
/// JSON, an absent field keeps its default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct ClientOptions {
    /// Degrees, in `FOV_RANGE`.
    pub fov: u8,
    /// In `[0, 1]`.
    pub fov_effect_scale: f32,
    pub smooth_light: bool,
    pub clouds: bool,
    /// From 0 (Moody) through 0.5 (the default) to 1 (Bright).
    pub brightness: f32,
    /// Columns, in `VIEW_DISTANCE_RANGE`; `None` is `VIEW_DISTANCE_DEFAULT`.
    pub view_distance: Option<u8>,
    /// `0` picks the largest scale that keeps 320 x 240 GUI units on screen.
    pub gui_scale: u32,
    /// The turn of the mouse, in `[0, 1]`; 0.5 turns the player 0.15 degree for each unit of
    /// motion.
    pub sensitivity: f32,
    pub vsync: bool,
    pub fullscreen: bool,
    /// Frames per second, in `MAX_FRAMERATE_RANGE` and a multiple of `MAX_FRAMERATE_STEP`.
    pub max_framerate: u16,
    pub improved_transparency: bool,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            fov: 70,
            fov_effect_scale: 1.0,
            smooth_light: true,
            clouds: true,
            brightness: 0.5,
            view_distance: None,
            gui_scale: 0,
            sensitivity: 0.5,
            vsync: false,
            fullscreen: false,
            max_framerate: 120,
            improved_transparency: false,
        }
    }
}

impl ClientOptions {
    /// Checks that each field lies in its range.
    pub fn validate(&self) -> Result<(), OptionsError> {
        if !FOV_RANGE.contains(&self.fov) {
            return Err(OptionsError::Fov(self.fov));
        }
        if !(0.0..=1.0).contains(&self.fov_effect_scale) {
            return Err(OptionsError::FovEffectScale(self.fov_effect_scale));
        }
        if !(0.0..=1.0).contains(&self.brightness) {
            return Err(OptionsError::Brightness(self.brightness));
        }
        if let Some(view) = self.view_distance
            && !VIEW_DISTANCE_RANGE.contains(&view)
        {
            return Err(OptionsError::ViewDistance(view));
        }
        if !(0.0..=1.0).contains(&self.sensitivity) {
            return Err(OptionsError::Sensitivity(self.sensitivity));
        }
        if !MAX_FRAMERATE_RANGE.contains(&self.max_framerate)
            || !self.max_framerate.is_multiple_of(MAX_FRAMERATE_STEP)
        {
            return Err(OptionsError::MaxFramerate(self.max_framerate));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_file_keeps_the_defaults_and_a_range_error_names_its_field() {
        let options: ClientOptions = serde_json::from_str(r#"{"fov": 90}"#).unwrap();
        assert_eq!(options.fov, 90);
        assert_eq!(options.brightness, 0.5);
        assert_eq!(options.validate(), Ok(()));

        let out_of_range = [
            (
                ClientOptions {
                    fov: 111,
                    ..options
                },
                OptionsError::Fov(111),
            ),
            (
                ClientOptions {
                    sensitivity: 1.01,
                    ..options
                },
                OptionsError::Sensitivity(1.01),
            ),
            (
                ClientOptions {
                    max_framerate: 270,
                    ..options
                },
                OptionsError::MaxFramerate(270),
            ),
            (
                ClientOptions {
                    max_framerate: 125,
                    ..options
                },
                OptionsError::MaxFramerate(125),
            ),
        ];
        for (options, error) in out_of_range {
            assert_eq!(options.validate(), Err(error));
        }
        assert!(serde_json::from_str::<ClientOptions>(r#"{"fog": 1}"#).is_err());
    }
}
