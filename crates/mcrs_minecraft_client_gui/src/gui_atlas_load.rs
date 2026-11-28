use crate::gui_atlas_error::GuiAtlasError;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_image_handles::GuiImageHandles;
use crate::gui_images::GuiImages;
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::inflate_zip::inflate_zip;
use crate::sprite::{GuiSprite, Scaling};
use crate::stitch_request::StitchRequest;
use crate::stitch_requests::StitchRequests;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{IRect, IVec2, UVec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_font::{
    CELL, COLUMNS, ClientLanguage, Font, FontOptions, MISSING_GLYPH_SIZE, ROWS,
    missing_glyph_pixels,
};
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_resource_pack::{MISSING_SIZE, RgbaImage, missing_pixels};
use std::collections::HashMap;

pub(crate) const ATLAS_WIDTH: u32 = 512;
pub(crate) const SWITCHER_WIDTH: u32 = 125;
pub(crate) const SWITCHER_HEIGHT: u32 = 75;
pub(crate) const BLANK: &str = "blank";
pub(crate) const MISSING: &str = "missing";
pub(crate) const MISSING_GLYPH: &str = "font/missing";
pub(crate) const GLYPH_CACHE: &str = "glyph cache";

pub(crate) const GUI_TEXTURES: [&str; 26] = [
    "hud/hotbar",
    "hud/hotbar_selection",
    "hud/hotbar_offhand_left",
    "hud/heart/container",
    "hud/heart/container_hardcore",
    "hud/heart/full",
    "hud/heart/half",
    "hud/heart/hardcore_full",
    "hud/heart/hardcore_half",
    "hud/food_empty",
    "hud/food_full",
    "hud/food_half",
    "container/slot_highlight_back",
    "container/slot_highlight_front",
    "container/slot/helmet",
    "container/slot/chestplate",
    "container/slot/leggings",
    "container/slot/boots",
    "container/slot/shield",
    "recipe_book/button",
    "recipe_book/button_highlighted",
    "container/gamemode_switcher",
    "gamemode_switcher/slot",
    "gamemode_switcher/selection",
    "misc/enchanted_glint_item",
    "icon/chat_modified",
];

fn texture_path(name: &str) -> String {
    match name {
        "container/gamemode_switcher" => {
            format!("minecraft/textures/gui/{name}.png")
        }
        "misc/enchanted_glint_item" => format!("minecraft/textures/{name}.png"),
        _ => format!("minecraft/textures/gui/sprites/{name}.png"),
    }
}

pub struct GuiAtlasParts {
    pub layout: GuiAtlasLayout,
    pub atlas: Image,
    pub glint: Image,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Packed {
    Named(&'static str),
    Page(usize),
}

pub fn load_gui_atlas(
    pack: &ResourceSet,
    requests: &StitchRequests,
    mut commands: Commands,
) -> Result<(), GuiAtlasError> {
    let loaded =
        Font::load(pack.files(), FontOptions::default(), inflate_zip).map_err(|failure| {
            for notice in &failure.notices {
                warn!("{notice}");
            }
            GuiAtlasError::Font(failure.error)
        })?;
    for notice in &loaded.notices {
        warn!("{notice}");
    }
    let font = loaded.font;
    let parts = build_gui_atlas(pack, &font, requests).map_err(GuiAtlasError::Textures)?;
    let language = ClientLanguage::load(pack.files()).map_err(GuiAtlasError::Translations)?;
    let GuiAtlasParts {
        layout,
        atlas,
        glint,
    } = parts;
    let glyph_origin = layout.glyph_origin.as_uvec2();
    commands.insert_resource(font);
    commands.insert_resource(layout);
    commands.insert_resource(language);
    commands.queue(move |world: &mut World| {
        let Some(mut assets) = world.get_resource_mut::<Assets<Image>>() else {
            error!("the GUI atlas has no Assets<Image> to enter");
            return;
        };
        let handles = GuiImageHandles {
            atlas: assets.add(atlas),
            glint: assets.add(glint),
        };
        let images = GuiImages {
            atlas: handles.atlas.id(),
            glint: handles.glint.id(),
            glyph_origin,
        };
        world.insert_resource(handles);
        world.insert_resource(images);
    });
    Ok(())
}

fn srgb_image(width: u32, height: u32, pixels: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn glint_image(width: u32, height: u32, pixels: Vec<u8>) -> Image {
    let mut image = srgb_image(width, height, pixels);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        label: Some("enchanted glint".to_owned()),
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

#[expect(
    clippy::too_many_lines,
    reason = "one load packs every texture in the order each placement depends on the last"
)]
pub fn build_gui_atlas(
    pack: &ResourceSet,
    font: &Font,
    requests: &StitchRequests,
) -> Result<GuiAtlasParts, String> {
    let mut images = Vec::new();
    let mut glint = None;
    let sprite_scalings = load_widget_images(pack, &mut images);
    for name in GUI_TEXTURES {
        let path = texture_path(name);
        let RgbaImage {
            rgba: pixels,
            width,
            height,
        } = RgbaImage::decode_png(pack.read(&path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("cannot decode {path}: {error}"))?;
        match name {
            "misc/enchanted_glint_item" => glint = Some((width, height, pixels)),
            "container/gamemode_switcher" => images.push((
                Packed::Named(name),
                crop(&pixels, width, SWITCHER_WIDTH, SWITCHER_HEIGHT),
                SWITCHER_WIDTH,
                SWITCHER_HEIGHT,
            )),
            _ => images.push((Packed::Named(name), pixels, width, height)),
        }
    }
    for request in requests.iter() {
        match request {
            StitchRequest::Sprite(location) => {
                let name = location.path();
                load_image(
                    pack,
                    name,
                    &texture_path(name),
                    Ok(Scaling::Stretch),
                    &mut images,
                );
            }
            StitchRequest::Background {
                texture,
                region,
                extent,
            } => {
                let path = format!("{}/{}", texture.namespace(), texture.path());
                let RgbaImage {
                    rgba: pixels,
                    width,
                    height,
                } = RgbaImage::decode_png(pack.read(&path).map_err(|error| error.to_string())?)
                    .map_err(|error| format!("cannot decode {path}: {error}"))?;
                let extent = extent.min(UVec2::new(width, height));
                images.push((
                    Packed::Named(region),
                    crop(&pixels, width, extent.x, extent.y),
                    extent.x,
                    extent.y,
                ));
            }
        }
    }
    for (source, page) in font.bitmap_pages() {
        let (width, height) = page.size();
        images.push((Packed::Page(source), page.pixels().to_vec(), width, height));
    }
    images.push((Packed::Named(BLANK), vec![255; 16], 2, 2));
    images.push((
        Packed::Named(MISSING),
        missing_pixels(),
        MISSING_SIZE,
        MISSING_SIZE,
    ));
    images.push((
        Packed::Named(MISSING_GLYPH),
        missing_glyph_pixels(),
        MISSING_GLYPH_SIZE.0,
        MISSING_GLYPH_SIZE.1,
    ));
    let (cache_width, cache_height) = ((COLUMNS * CELL) as u32, (ROWS * CELL) as u32);
    images.push((
        Packed::Named(GLYPH_CACHE),
        vec![0; (cache_width * cache_height * 4) as usize],
        cache_width,
        cache_height,
    ));
    images.sort_by_key(|(name, _, _, height)| (std::cmp::Reverse(*height), *name));
    let atlas_width = images
        .iter()
        .map(|(_, _, width, _)| *width)
        .fold(ATLAS_WIDTH, u32::max);
    let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
    let mut placed = Vec::new();
    for (name, pixels, width, height) in images {
        if x + width > atlas_width {
            x = 0;
            y += row;
            row = 0;
        }
        placed.push((
            name,
            pixels,
            IRect::new(x as i32, y as i32, (x + width) as i32, (y + height) as i32),
        ));
        x += width;
        row = row.max(height);
    }
    let atlas_height = (y + row).next_power_of_two();
    let mut pixels = vec![0u8; (atlas_width * atlas_height * 4) as usize];
    let mut regions = HashMap::new();
    let mut page_origins = Vec::new();
    for (name, source, rect) in placed {
        let width = rect.width() as usize;
        for (line, source_row) in source.chunks_exact(width * 4).enumerate() {
            let start =
                ((rect.min.y as usize + line) * atlas_width as usize + rect.min.x as usize) * 4;
            if let Some(target) = pixels.get_mut(start..start + width * 4) {
                target.copy_from_slice(source_row);
            }
        }
        match name {
            Packed::Named(name) => {
                regions.insert(name, rect);
            }
            Packed::Page(source) => page_origins.push((source, rect.min)),
        }
    }
    let missing_region = regions.get(MISSING).copied().unwrap_or_default();
    let missing = GuiSprite::new(missing_region, Scaling::Stretch);
    let mut sprites = [missing; GuiSpriteId::ALL.len()];
    for (id, scaling) in sprite_scalings {
        let region = regions.get(id.name());
        if let (Some(slot), Some(&region), Some(scaling)) =
            (sprites.get_mut(id as usize), region, scaling)
        {
            *slot = GuiSprite::new(region, scaling);
        }
    }
    let mut textures = [missing_region; GuiTextureId::ALL.len()];
    for id in GuiTextureId::ALL {
        if let (Some(slot), Some(&region)) = (textures.get_mut(id as usize), regions.get(id.name()))
        {
            *slot = region;
        }
    }
    let missing_glyph = regions.get(MISSING_GLYPH).copied().unwrap_or_default();
    let glyph_origin = regions
        .get(GLYPH_CACHE)
        .map_or(IVec2::ZERO, |region| region.min);
    let chat_modified = regions
        .get("icon/chat_modified")
        .copied()
        .unwrap_or_default();
    let layout = GuiAtlasLayout {
        width: atlas_width,
        height: atlas_height,
        regions,
        sprites,
        textures,
        missing,
        page_origins,
        missing_glyph,
        glyph_origin,
        chat_modified,
    };
    let (glint_width, glint_height, glint_pixels) = glint.ok_or("the GUI atlas lists no glint")?;
    Ok(GuiAtlasParts {
        layout,
        atlas: srgb_image(atlas_width, atlas_height, pixels),
        glint: glint_image(glint_width, glint_height, glint_pixels),
    })
}

pub(crate) fn load_widget_images(
    pack: &ResourceSet,
    images: &mut Vec<(Packed, Vec<u8>, u32, u32)>,
) -> [(GuiSpriteId, Option<Scaling>); GuiSpriteId::ALL.len()] {
    for id in GuiTextureId::ALL {
        let path = format!("minecraft/{}", id.name());
        load_image(pack, id.name(), &path, Ok(Scaling::Stretch), images);
    }
    GuiSpriteId::ALL.map(|id| {
        let path = texture_path(id.name());
        let scaling = Scaling::read(pack.get(&format!("{path}.mcmeta")));
        (id, load_image(pack, id.name(), &path, scaling, images))
    })
}

pub(crate) fn load_image(
    pack: &ResourceSet,
    name: &'static str,
    path: &str,
    scaling: Result<Scaling, String>,
    images: &mut Vec<(Packed, Vec<u8>, u32, u32)>,
) -> Option<Scaling> {
    let bytes = pack.get(path)?;
    let loaded = scaling.and_then(|scaling| {
        let image = RgbaImage::decode_png(bytes)
            .map_err(|error| format!("cannot decode {path}: {error}"))?;
        Ok((scaling, image))
    });
    match loaded {
        Ok((scaling, image)) => {
            images.push((Packed::Named(name), image.rgba, image.width, image.height));
            Some(scaling)
        }
        Err(reason) => {
            error!("GUI image minecraft:{name}: {reason}");
            None
        }
    }
}

pub(crate) fn crop(pixels: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let stride = stride as usize * 4;
    let width = width as usize * 4;
    (0..height as usize)
        .flat_map(|row| {
            pixels
                .get(row * stride..row * stride + width)
                .into_iter()
                .flatten()
                .copied()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus_font::corpus_font;
    use crate::menu_test_app::{ATLAS, stitch_requests};

    #[test]
    fn the_gui_atlas_holds_every_region_and_the_digit_widths() {
        let GuiAtlasParts {
            layout: atlas,
            atlas: image,
            glint,
        } = &*ATLAS;
        for name in GUI_TEXTURES
            .iter()
            .filter(|n| **n != "misc/enchanted_glint_item")
        {
            assert!(atlas.regions.contains_key(name), "{name}");
        }
        for request in stitch_requests().iter() {
            let region = request.region();
            assert!(atlas.regions.contains_key(region), "{region}");
            if let StitchRequest::Background { extent, .. } = request {
                assert_eq!(atlas.region(region).size().as_uvec2(), extent, "{region}");
            }
        }
        assert_eq!(
            atlas.region("container/generic_54").size(),
            IVec2::new(176, 222)
        );
        assert_eq!(
            atlas
                .region("container/creative_inventory/tab_items")
                .size(),
            IVec2::new(195, 136)
        );
        assert_eq!(atlas.region("hud/hotbar").size(), IVec2::new(182, 22));
        assert_eq!(
            atlas.region("container/inventory").size(),
            IVec2::new(176, 166)
        );
        assert_eq!(
            atlas.region("container/gamemode_switcher").size(),
            IVec2::new(125, 75)
        );
        assert_eq!(
            atlas.region("gamemode_switcher/slot").size(),
            IVec2::new(26, 26)
        );
        assert_eq!(corpus_font().advance('7', false), 6.0);
        assert_eq!(glint.width(), 128);
        assert!(image.height() <= 2048);
        let blank = atlas.region(BLANK);
        let start = (blank.min.y as usize * image.width() as usize + blank.min.x as usize) * 4;
        let pixels = image.data.as_deref().unwrap();
        assert_eq!(&pixels[start..start + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn every_sprite_id_is_stitched_and_a_sprite_the_pack_lacks_is_the_missing_sprite() {
        let atlas = &ATLAS.layout;
        for id in GuiSpriteId::ALL {
            assert_ne!(atlas.sprite(id), atlas.missing, "{}", id.name());
        }
        for id in GuiTextureId::ALL {
            assert!(atlas.regions.contains_key(id.name()), "{}", id.name());
        }
        let gui = [
            "minecraft/font/",
            "minecraft/textures/font/",
            "minecraft/textures/gui/",
            "minecraft/textures/misc/",
        ];
        let without_button: ResourceSet = mcrs_minecraft_model::vanilla_source::resource_files()
            .iter()
            .filter(|(path, _)| gui.iter().any(|folder| path.starts_with(folder)))
            .filter(|(path, _)| !path.contains("sprites/widget/button.png"))
            .cloned()
            .collect();
        let GuiAtlasParts { layout: atlas, .. } =
            build_gui_atlas(&without_button, corpus_font(), &StitchRequests::default()).unwrap();
        assert_eq!(atlas.sprite(GuiSpriteId::Button), atlas.missing);
        assert_ne!(atlas.sprite(GuiSpriteId::ButtonHighlighted), atlas.missing);
    }
}
