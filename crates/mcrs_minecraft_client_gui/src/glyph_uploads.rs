use bevy::asset::AssetEvent;
use bevy::prelude::*;

use crate::gui_images::GuiImages;
use mcrs_minecraft_font::{CELL_BYTES, Font, GlyphCache};

pub(crate) struct GlyphCell {
    pub(crate) cell: usize,
    pub(crate) pixels: [u8; CELL_BYTES],
}

#[derive(Resource, Default)]
pub(crate) struct GlyphUploads {
    pub(crate) cells: Vec<GlyphCell>,
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub(crate) fn glyphs_pending(uploads: Res<GlyphUploads>) -> bool {
    !uploads.cells.is_empty()
}

pub(crate) fn queue_glyphs(
    cache: Option<Res<GlyphCache>>,
    font: Option<Res<Font>>,
    images: Option<Res<GuiImages>>,
    mut events: MessageReader<AssetEvent<Image>>,
    mut sent: Local<Vec<Option<char>>>,
    mut uploads: ResMut<GlyphUploads>,
) {
    let Some(images) = images else {
        return;
    };
    let mut prepared = false;
    for event in events.read() {
        prepared |= matches!(event, AssetEvent::Added { id } | AssetEvent::Modified { id }
            if *id == images.atlas);
    }
    if prepared || images.is_changed() {
        sent.clear();
    }
    let (Some(cache), Some(font)) = (cache, font) else {
        return;
    };
    sent.resize(cache.resident().len(), None);
    for (cell, (wanted, drawn)) in cache.resident().iter().zip(sent.iter_mut()).enumerate() {
        if wanted == drawn {
            continue;
        }
        *drawn = *wanted;
        let Some(pixels) = wanted.and_then(|glyph| font.unifont_cell(glyph)) else {
            continue;
        };
        uploads.cells.push(GlyphCell { cell, pixels });
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetId;

    use super::*;
    use crate::inflate_zip::inflate_zip;
    use mcrs_minecraft_font::FontOptions;
    use mcrs_minecraft_model::resource_set::ResourceSet;

    const GLYPHS: [char; 3] = ['\u{4E00}', '\u{4E01}', '\u{4E8C}'];

    #[test]
    fn resident_cells_are_queued_once_and_again_after_the_atlas_is_added() {
        let font = Font::load(
            ResourceSet::corpus().files(),
            FontOptions::default(),
            inflate_zip,
        )
        .unwrap()
        .font;
        let mut cache = GlyphCache::default();
        cache.begin_frame();
        for glyph in GLYPHS {
            cache.cell(glyph).unwrap();
        }
        let atlas = AssetId::<Image>::default();
        let mut app = App::new();
        app.add_message::<AssetEvent<Image>>();
        let world = app.world_mut();
        world.insert_resource(font);
        world.insert_resource(cache);
        world.insert_resource(GuiImages {
            atlas,
            glint: atlas,
            glyph_origin: UVec2::ZERO,
        });
        world.init_resource::<GlyphUploads>();
        let system = world.register_system(queue_glyphs);

        world.run_system(system).unwrap();
        let queued: Vec<usize> = world
            .resource_mut::<GlyphUploads>()
            .cells
            .drain(..)
            .map(|glyph| glyph.cell)
            .collect();
        assert_eq!(queued, [0, 1, 2]);

        world.run_system(system).unwrap();
        assert!(world.resource::<GlyphUploads>().cells.is_empty());

        world.write_message(AssetEvent::Added { id: atlas });
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<GlyphUploads>().cells.len(), GLYPHS.len());
    }
}
