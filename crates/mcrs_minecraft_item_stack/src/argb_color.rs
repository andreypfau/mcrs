#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArgbColor(pub i32);

impl ArgbColor {
    #[must_use]
    pub fn from_rgb_floats(red: f32, green: f32, blue: f32) -> Self {
        let channel = |value: f32| (((value * 255.0).floor() as i32) & 0xFF).cast_unsigned();
        let argb = 0xFF00_0000 | channel(red) << 16 | channel(green) << 8 | channel(blue);
        Self(argb.cast_signed())
    }
}
