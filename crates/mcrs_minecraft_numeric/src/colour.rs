use crate::conversion::truncate_to_i32;

/// The packed `0xRRGGBB` colour of a hue, a saturation and a value. A hue of `-1/6` or less is
/// outside the domain and gives `0`.
#[must_use]
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> i32 {
    let sextant_position = hue * 6.0;
    let sextant = truncate_to_i32(sextant_position) % 6;
    let fraction = sextant_position - sextant as f32;
    let low = value * (1.0 - saturation);
    let falling = value * (1.0 - fraction * saturation);
    let rising = value * (1.0 - (1.0 - fraction) * saturation);
    let [red, green, blue] = match sextant {
        0 => [value, rising, low],
        1 => [falling, value, low],
        2 => [low, value, rising],
        3 => [low, falling, value],
        4 => [rising, low, value],
        5 => [value, low, falling],
        _ => return 0,
    };
    channel(red) << 16 | channel(green) << 8 | channel(blue)
}

fn channel(component: f32) -> i32 {
    truncate_to_i32(component * 255.0).clamp(0, 255)
}
