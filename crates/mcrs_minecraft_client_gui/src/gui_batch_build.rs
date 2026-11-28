use crate::atlas_quad::AtlasQuad;
use crate::batch_glint::BatchGlint;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_atlas_load::BLANK;
use crate::gui_batch::{GLINT_BIT, GUI_ATLAS_BIT, GuiBatch, GuiDraw, GuiDrawKind};
use crate::gui_fade::GuiFade;
use crate::gui_layer::GuiLayer;
use crate::gui_layer_slot::LayerSlot;
use crate::gui_quad::GuiQuad;
use crate::gui_quad::WHITE;
use crate::gui_size::GuiSize;
use crate::item_decorations::{CountLabel, Decorated, decorations};
use crate::item_icons::ItemIcons;
use crate::scene_icon::SceneIcon;
use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::glyph_cache::GlyphCache;
use mcrs_minecraft_item_icon::{GuiVertex, IconLayer, IconMeshes, ItemIcon, Overlays};
use mcrs_minecraft_model::atlas::rgba;

pub(crate) fn push_quad(vertices: &mut Vec<GuiVertex>, quad: AtlasQuad, sprite: u32) {
    let AtlasQuad { rect, uv, colors } = quad;
    let corners = [
        [rect.min.x, rect.min.y, 0.0],
        [rect.max.x, rect.min.y, 0.0],
        [rect.max.x, rect.max.y, 0.0],
        [rect.min.x, rect.max.y, 0.0],
    ];
    let vertex = |pos: [f32; 3], uv: [f32; 2], color: u32| GuiVertex {
        pos,
        uv,
        color: rgba(color),
        sprite,
    };
    let [c0, c1, c2, c3] = corners;
    let [u0, u1, u2, u3] = uv;
    let [k0, k1, k2, k3] = colors;
    let (a, b, c, d) = (
        vertex(c0, u0, k0),
        vertex(c1, u1, k1),
        vertex(c2, u2, k2),
        vertex(c3, u3, k3),
    );
    push_triangles(vertices, [a, b, c, d]);
}

fn push_triangles(vertices: &mut Vec<GuiVertex>, [a, b, c, d]: [GuiVertex; 4]) {
    vertices.extend([a, b, c, a, c, d]);
}

pub(crate) fn build_gui_batch(
    layers: Query<(Entity, &GuiLayer)>,
    size: Res<GuiSize>,
    atlas: Option<Res<GuiAtlasLayout>>,
    font: Option<Res<Font>>,
    icons: ItemIcons,
    fade: Res<GuiFade>,
    mut cache: ResMut<GlyphCache>,
    mut batch: ResMut<GuiBatch>,
    mut glint: ResMut<BatchGlint>,
    mut clips: Local<Vec<IRect>>,
    mut decorations_buffer: Local<Vec<GuiQuad>>,
    mut order: Local<Vec<LayerSlot>>,
) {
    let batch = &mut *batch;
    cache.begin_frame();
    clips.clear();
    batch.vertices.clear();
    batch.draws.clear();
    batch.scale = size.scale;
    let (Some(atlas), Some(font)) = (atlas.as_deref(), font.as_deref()) else {
        glint.set_if_neq(BatchGlint { has_glint: false });
        return;
    };
    let blank = atlas.uv(atlas.region(BLANK));
    order.clear();
    order.extend(layers.iter().map(|(entity, layer)| LayerSlot {
        order: layer.order(),
        entity,
    }));
    order.sort_unstable_by_key(|slot| slot.order);
    let mut emitter = BatchEmitter {
        batch,
        clips: &mut clips,
        cache: &mut cache,
        atlas,
        blank,
        meshes: icons.meshes(),
        fade: *fade,
        flat_start: None,
        has_glint: false,
    };
    for slot in order.iter() {
        let Ok((_, layer)) = layers.get(slot.entity) else {
            continue;
        };
        for (quad, icon) in layer.drawn() {
            emitter.emit(quad, icon);
            let GuiQuad::Icon { origin, .. } = quad else {
                continue;
            };
            let Some(entry) = icon else {
                continue;
            };
            let count = match entry.overlays {
                Overlays::Stack => CountLabel::Plain(entry.overlay.count),
                Overlays::Capped => CountLabel::Capped(entry.overlay.count),
                _ => continue,
            };
            let decorated = Decorated {
                count,
                damage: entry.overlay.durability,
            };
            decorations_buffer.clear();
            decorations(origin, &decorated, font, &mut decorations_buffer);
            for &decoration in decorations_buffer.iter() {
                emitter.emit(decoration, None);
            }
        }
    }
    emitter.finish();
    let has_glint = emitter.has_glint;
    glint.set_if_neq(BatchGlint { has_glint });
}

struct BatchEmitter<'a> {
    batch: &'a mut GuiBatch,
    clips: &'a mut Vec<IRect>,
    cache: &'a mut GlyphCache,
    atlas: &'a GuiAtlasLayout,
    blank: [[f32; 2]; 4],
    meshes: Option<&'a IconMeshes>,
    fade: GuiFade,
    flat_start: Option<u32>,
    has_glint: bool,
}

impl BatchEmitter<'_> {
    fn finish(&mut self) {
        let end = self.batch.vertices.len() as u32;
        close_flat(&mut self.flat_start, end, &mut self.batch.draws);
    }

    fn emit(&mut self, mut quad: GuiQuad, icon: Option<&SceneIcon>) {
        self.fade.apply(&mut quad);
        let vertices = &mut self.batch.vertices;
        match quad {
            GuiQuad::Icon { origin, .. } => {
                let end = vertices.len() as u32;
                close_flat(&mut self.flat_start, end, &mut self.batch.draws);
                let (Some(entry), Some(meshes)) = (icon, self.meshes) else {
                    return;
                };
                self.has_glint |= entry.icon.foil();
                push_icon(vertices, &entry.icon, meshes, origin);
                push_item_draw(end, vertices, &mut self.batch.draws);
            }
            GuiQuad::Highlight { rect, invert } => {
                let end = vertices.len() as u32;
                close_flat(&mut self.flat_start, end, &mut self.batch.draws);
                let white = GuiQuad::Fill { rect, color: WHITE };
                push_flat_quad(
                    vertices,
                    self.atlas,
                    self.blank,
                    self.cache,
                    white,
                    self.clips.last().copied(),
                );
                let range = end..vertices.len() as u32;
                if range.is_empty() {
                    return;
                }
                if invert {
                    self.batch.draws.push(GuiDraw {
                        range: range.clone(),
                        kind: GuiDrawKind::InvertRedGreen,
                    });
                }
                self.batch.draws.push(GuiDraw {
                    range,
                    kind: GuiDrawKind::FullBlue,
                });
            }
            GuiQuad::Clip { rect } => {
                let inner = self
                    .clips
                    .last()
                    .map_or(rect, |outer| outer.intersect(rect));
                self.clips.push(inner);
            }
            GuiQuad::EndClip => {
                self.clips.pop();
            }
            GuiQuad::Figure { rect } => {
                let rect = self.clips.last().map_or(rect, |clip| clip.intersect(rect));
                if rect.is_empty() {
                    return;
                }
                let end = vertices.len() as u32;
                close_flat(&mut self.flat_start, end, &mut self.batch.draws);
                self.batch.draws.push(GuiDraw {
                    range: end..end,
                    kind: GuiDrawKind::Figure { rect },
                });
            }
            flat => {
                if self.flat_start.is_none() {
                    self.flat_start = Some(vertices.len() as u32);
                }
                push_flat_quad(
                    vertices,
                    self.atlas,
                    self.blank,
                    self.cache,
                    flat,
                    self.clips.last().copied(),
                );
            }
        }
    }
}

fn close_flat(start: &mut Option<u32>, end: u32, draws: &mut Vec<GuiDraw>) {
    if let Some(begin) = start.take()
        && begin != end
    {
        draws.push(GuiDraw {
            range: begin..end,
            kind: GuiDrawKind::Flat,
        });
    }
}

fn push_item_draw(start: u32, vertices: &[GuiVertex], draws: &mut Vec<GuiDraw>) {
    let after = vertices.len() as u32;
    if after != start {
        draws.push(GuiDraw {
            range: start..after,
            kind: GuiDrawKind::Item,
        });
    }
}

fn push_icon(vertices: &mut Vec<GuiVertex>, icon: &ItemIcon, meshes: &IconMeshes, origin: IVec2) {
    let glint = if icon.foil() { GLINT_BIT } else { 0 };
    for IconLayer { model, tints } in icon.layers() {
        for quad in meshes.get(*model).into_iter().flatten() {
            push_item_quad(vertices, &quad.tinted(tints), origin, glint);
        }
    }
}

pub(crate) fn push_item_quad(
    vertices: &mut Vec<GuiVertex>,
    quad: &[GuiVertex; 4],
    origin: IVec2,
    glint: u32,
) {
    let shifted = |v: &GuiVertex| GuiVertex {
        pos: [
            v.pos[0] + origin.x as f32,
            v.pos[1] + origin.y as f32,
            v.pos[2],
        ],
        sprite: v.sprite | glint,
        ..*v
    };
    let [a, b, c, d] = quad.each_ref().map(shifted);
    push_triangles(vertices, [a, b, c, d]);
}

pub(crate) fn push_flat_quad(
    vertices: &mut Vec<GuiVertex>,
    atlas: &GuiAtlasLayout,
    blank: [[f32; 2]; 4],
    cache: &mut GlyphCache,
    quad: GuiQuad,
    clip: Option<IRect>,
) {
    let flat = match quad {
        GuiQuad::Sprite {
            rect,
            region,
            color,
        } => AtlasQuad {
            rect: rect.as_rect(),
            uv: atlas.uv(atlas.region(region)),
            colors: [color; 4],
        },
        GuiQuad::Fill { rect, color } => AtlasQuad {
            rect: rect.as_rect(),
            uv: blank,
            colors: [color; 4],
        },
        GuiQuad::Texels {
            rect,
            texels,
            color,
        } => AtlasQuad {
            rect: rect.as_rect(),
            uv: atlas.texel_uv(texels),
            colors: [color; 4],
        },
        GuiQuad::FillGradient { rect, top, bottom } => AtlasQuad {
            rect: rect.as_rect(),
            uv: blank,
            colors: [top, top, bottom, bottom],
        },
        GuiQuad::Glyph {
            pen,
            glyph,
            image,
            color,
            weight,
        } => {
            let Some(texels) = atlas.glyph_texels(image.source, glyph, cache) else {
                return;
            };
            AtlasQuad {
                rect: image.strike(pen, weight),
                uv: atlas.texel_uv(texels),
                colors: [color; 4],
            }
        }
        GuiQuad::Icon { .. }
        | GuiQuad::Highlight { .. }
        | GuiQuad::Clip { .. }
        | GuiQuad::EndClip
        | GuiQuad::Figure { .. } => return,
    };
    let clipped = match clip {
        Some(clip) => flat.clip(clip),
        None => Some(flat),
    };
    if let Some(flat) = clipped {
        push_quad(vertices, flat, GUI_ATLAS_BIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use mcrs_minecraft_item_icon::{IconSources, resolve};
    use mcrs_minecraft_item_stack::{Count, ItemStack, Kind, NameLookup, NameTable};
    use mcrs_minecraft_model::item_model_asset::DisplayContext;
    use mcrs_minecraft_model::item_model_context::ItemModelContext;
    use mcrs_minecraft_model::stack_view::StackView;
    use mcrs_minecraft_resource::ResourceLocation;

    use crate::gui_band::GuiBand;
    use crate::menu_test_app::{MODELS, TABLE, insert_gui_assets};
    use mcrs_minecraft_item_icon::StackOverlay;

    #[test]
    fn a_stack_overlay_draws_the_count_beside_an_icon() {
        let location = ResourceLocation::read("minecraft:stone").unwrap();
        let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
        let stack = ItemStack::new(kind, Count::new(17).unwrap());
        let view = StackView {
            stack: &stack,
            table: &TABLE,
        };
        let sources = IconSources {
            models: &MODELS,
            registries: None,
        };
        let icon = resolve(view, &sources, &ItemModelContext::new(DisplayContext::Gui));
        for (overlays, glyph_vertices) in [(Overlays::Stack, 24), (Overlays::None, 0)] {
            let mut layer = GuiLayer::new(GuiBand::Hud, 0);
            ItemIcons::draw_icon(
                &mut layer,
                IVec2::ZERO,
                icon,
                StackOverlay::of(view),
                overlays,
            );
            let mut world = World::new();
            world.spawn(layer);
            world.insert_resource(GuiSize::default());
            insert_gui_assets(&mut world);
            world.init_resource::<crate::gui_fade::GuiFade>();
            world.init_resource::<GlyphCache>();
            world.init_resource::<GuiBatch>();
            world.init_resource::<BatchGlint>();
            world.run_system_once(build_gui_batch).unwrap();
            let batch = world.resource::<GuiBatch>();
            let flat: usize = batch
                .draws
                .iter()
                .filter(|draw| draw.kind == GuiDrawKind::Flat)
                .map(|draw| draw.range.len())
                .sum();
            assert_eq!(
                flat, glyph_vertices,
                "{overlays:?}: two digits with their shadows, two triangles each"
            );
        }
    }
}
