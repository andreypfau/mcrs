use crate::{FOV_RANGE, VIEW_DISTANCE_RANGE};

/// A field of [`ClientOptions`](crate::ClientOptions) outside its range, with the value it holds.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OptionsError {
    /// `fov` is outside `FOV_RANGE`.
    #[error(
        "fov is {0}, expected {start} to {end} degrees",
        start = FOV_RANGE.start(),
        end = FOV_RANGE.end()
    )]
    Fov(u8),
    /// `fov_effect_scale` is outside `[0, 1]`.
    #[error("fov_effect_scale is {0}, expected 0 to 1")]
    FovEffectScale(f32),
    /// `brightness` is outside `[0, 1]`.
    #[error("brightness is {0}, expected 0 to 1")]
    Brightness(f32),
    /// `view_distance` is outside `VIEW_DISTANCE_RANGE`.
    #[error(
        "view_distance is {0}, expected {start} to {end} columns",
        start = VIEW_DISTANCE_RANGE.start(),
        end = VIEW_DISTANCE_RANGE.end()
    )]
    ViewDistance(u8),
}
