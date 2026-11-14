/// The values of one frame that the fog parameters depend on. A colour is three image values
/// in `[0, 1]`; a distance is in blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogInputs {
    /// The attribute `visual/fog_color`.
    pub fog_color: [f32; 3],
    /// The attribute `visual/sky_color`.
    pub sky_color: [f32; 3],
    /// The colour of the attribute `visual/sunrise_sunset_color`.
    pub sunrise_color: [f32; 3],
    /// The alpha of the attribute `visual/sunrise_sunset_color`, in `[0, 1]`.
    pub sunrise_alpha: f32,
    /// The attribute `visual/sun_angle`, in degrees.
    pub sun_angle: f32,
    /// The attribute `visual/fog_start_distance`.
    pub fog_start_distance: f32,
    /// The attribute `visual/fog_end_distance`.
    pub fog_end_distance: f32,
    /// The attribute `visual/sky_fog_end_distance`.
    pub sky_fog_end_distance: f32,
    /// The attribute `visual/cloud_fog_end_distance`.
    pub cloud_fog_end_distance: f32,
    /// The render distance of the client, in chunks.
    pub render_distance: u8,
    /// The option Cloud Distance, in chunks.
    pub cloud_distance: u8,
    /// The east part of the unit view direction of the camera.
    pub view_east: f32,
    /// The rain level, in `[0, 1]`.
    pub rain_level: f32,
    /// The thunder level multiplied by the rain level, in `[0, 1]`.
    pub thunder_level: f32,
}
