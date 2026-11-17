#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogInputs {
    pub fog_color: [f32; 3],
    pub sky_color: [f32; 3],
    pub sunrise_color: [f32; 3],
    pub sunrise_alpha: f32,
    pub sun_angle: f32,
    pub fog_start_distance: f32,
    pub fog_end_distance: f32,
    pub sky_fog_end_distance: f32,
    pub cloud_fog_end_distance: f32,
    pub render_distance: u8,
    pub cloud_distance: u8,
    pub view_east: f32,
    pub rain_level: f32,
    pub thunder_level: f32,
}
