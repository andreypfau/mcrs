use std::ops::RangeInclusive;
use std::path::Path;

use mcrs_minecraft_mesh::smooth_light::SmoothLight;
use serde::{Deserialize, Serialize};

use crate::camera::FovOptions;
use crate::config::MAX_VIEW_DISTANCE;
use crate::sky::Clouds;
use crate::stream::LightOptions;

pub const FOV_RANGE: RangeInclusive<u8> = 30..=110;
pub const VIEW_DISTANCE_RANGE: RangeInclusive<u8> = 2..=MAX_VIEW_DISTANCE;

/// The video options of the client, with the meaning of the vanilla option of the same name.
/// `MCRS_OPTIONS=<file>` names a JSON file that holds any of these fields; an absent field keeps
/// its default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientOptions {
    /// Degrees, in `FOV_RANGE`.
    pub fov: u8,
    /// In `[0, 1]`.
    pub fov_effect_scale: f32,
    pub smooth_light: bool,
    pub clouds: bool,
    /// From 0 (Moody) through 0.5 (the default) to 1 (Bright).
    pub brightness: f32,
    /// Columns, in `VIEW_DISTANCE_RANGE`; the `MCRS_VIEW` setting wins when both are set.
    pub view_distance: Option<u8>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        let fov = FovOptions::default();
        Self {
            fov: fov.fov,
            fov_effect_scale: fov.effect_scale,
            smooth_light: true,
            clouds: true,
            brightness: 0.5,
            view_distance: None,
        }
    }
}

impl ClientOptions {
    /// Reads and validates the options file at `path`.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let options: Self = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        options.validate()?;
        Ok(options)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !FOV_RANGE.contains(&self.fov) {
            return Err(format!(
                "fov is {}, expected {} to {} degrees",
                self.fov,
                FOV_RANGE.start(),
                FOV_RANGE.end()
            ));
        }
        if !(0.0..=1.0).contains(&self.fov_effect_scale) {
            return Err(format!(
                "fov_effect_scale is {}, expected 0 to 1",
                self.fov_effect_scale
            ));
        }
        if !(0.0..=1.0).contains(&self.brightness) {
            return Err(format!(
                "brightness is {}, expected 0 to 1",
                self.brightness
            ));
        }
        if let Some(view) = self.view_distance
            && !VIEW_DISTANCE_RANGE.contains(&view)
        {
            return Err(format!(
                "view_distance is {view}, expected {} to {} columns",
                VIEW_DISTANCE_RANGE.start(),
                VIEW_DISTANCE_RANGE.end()
            ));
        }
        Ok(())
    }

    pub fn fov_options(&self) -> FovOptions {
        FovOptions {
            fov: self.fov,
            effect_scale: self.fov_effect_scale,
        }
    }

    pub fn light_options(&self) -> LightOptions {
        LightOptions {
            smooth_light: if self.smooth_light {
                SmoothLight::On
            } else {
                SmoothLight::Off
            },
        }
    }

    pub fn clouds(&self) -> Clouds {
        self.clouds.into()
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

        let too_wide = ClientOptions {
            fov: 111,
            ..options
        };
        assert!(too_wide.validate().unwrap_err().starts_with("fov is 111"));
        assert!(serde_json::from_str::<ClientOptions>(r#"{"fog": 1}"#).is_err());
    }
}
