use crate::resource_set::ResourceSet;
use crate::sprite_error::SpriteError;
use mcrs_minecraft_model_file::resource_id::resource_path;
use mcrs_minecraft_resource_pack::RgbaImage;

pub fn load_colormap(pack: &ResourceSet, name: &str) -> Result<Vec<u8>, SpriteError> {
    let path = resource_path(&format!("minecraft:colormap/{name}"), "textures", "png");
    let image = RgbaImage::decode_png(pack.read(&path)?).map_err(|error| SpriteError::Decode {
        path: path.clone(),
        error,
    })?;
    if image.width != 256 || image.height != 256 {
        return Err(SpriteError::ColormapSize {
            path,
            width: image.width,
            height: image.height,
        });
    }
    Ok(image.rgba)
}

pub fn colormap_rgba(map: Option<&[u8]>, temperature: f32, downfall: f32) -> Option<[f32; 4]> {
    let color = sample_colormap(Some(map?), temperature, downfall);
    let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
    Some([channel(16), channel(8), channel(0), 1.0])
}

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
