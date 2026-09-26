/// The `block_light_tint` environment attribute's default, `#ffd88c`.
const DEFAULT_TINT: [f32; 3] = [1.0, 216.0 / 255.0, 140.0 / 255.0];

fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    f / (4.0 - 3.0 * f)
}

/// The block term of the lightmap at the server's `level`, with the resolved
/// colour standing in for the tint. No sky, ambient or brightness term.
pub fn final_rgb(texel: [u8; 4], level: u8) -> [u8; 3] {
    let level = level as f32;
    let f = level / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let brightness = light_curve(level);
    let default = texel[3] as f32 / 255.0;
    std::array::from_fn(|c| {
        let tint = texel[c] as f32 / 255.0 + default * DEFAULT_TINT[c];
        let lit = (tint + (1.0 - tint) * 0.9 * parabolic) * brightness;
        (lit.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

/// Hue in degrees, or `None` when the colour is too grey for hue to mean
/// anything.
pub fn hue(rgb: [u8; 3]) -> Option<f32> {
    let [r, g, b] = rgb.map(|c| c as f32 / 255.0);
    let max = r.max(g).max(b);
    let chroma = max - r.min(g).min(b);
    if max <= 0.0 || chroma / max <= 0.05 {
        return None;
    }
    let sector = if max == r {
        (g - b) / chroma
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    Some((sector * 60.0).rem_euclid(360.0))
}

pub fn hue_difference(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}
