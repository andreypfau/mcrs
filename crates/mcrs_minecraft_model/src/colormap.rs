use crate::atlas::decode_png;
use crate::resource_set::{ResourceSet, resource_path};

/// The pixels of `textures/colormap/<name>.png`, which vanilla samples as a 256x256 grid.
pub fn load_colormap(pack: &ResourceSet, name: &str) -> Result<Vec<u8>, String> {
    let path = resource_path(&format!("minecraft:colormap/{name}"), "textures", "png");
    let (data, width, height) = decode_png(pack.read(&path)?, &path)?;
    if width != 256 || height != 256 {
        return Err(format!(
            "{path} is {width}x{height}, and a colormap is sampled as a 256x256 grid"
        ));
    }
    Ok(data)
}

/// An item's grass colour, where vanilla's colormap resolves it, as RGBA channels in `0..=1`.
pub fn colormap_rgba(map: Option<&[u8]>, temperature: f32, downfall: f32) -> Option<[f32; 4]> {
    let color = sample_colormap(Some(map?), temperature, downfall);
    let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
    Some([channel(16), channel(8), channel(0), 1.0])
}

/// Vanilla's colormap lookup, in doubles as it computes it. A missing map, or a climate that
/// lands outside it, gives the magenta vanilla uses to flag the same.
pub fn sample_colormap(map: Option<&[u8]>, temperature: f32, downfall: f32) -> u32 {
    const MISSING: u32 = 0xff00ff;
    let Some(map) = map else {
        return MISSING;
    };
    let temperature = f64::from(temperature.clamp(0.0, 1.0));
    let downfall = f64::from(downfall.clamp(0.0, 1.0)) * temperature;
    let x = ((1.0 - temperature) * 255.0) as usize;
    let y = ((1.0 - downfall) * 255.0) as usize;
    let offset = (y << 8 | x) * 4;
    match map.get(offset..offset + 3) {
        Some(&[r, g, b]) => u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        _ => MISSING,
    }
}
