use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::{Mat4, Vec3};
use bevy::prelude::Resource;
use mcrs_minecraft_core::ResourceLocation;

use super::asset::{
    ClientItem, ConditionProperty, RangeProperty, SelectSwitch, SpecialModel, TintSource,
    UnbakedItemModel,
};
use super::generator;
use super::transform::compose;
use crate::atlas::{MISSING_SPRITE, SpriteRegistry};
use crate::blocks::load_colormap;
use crate::bake::{Dir, VariantRotation, draws_face, face_geometry};
use crate::model::{Element, Face, GuiLight, ItemTransform, Pack, ResolvedModel, resolve_model};

#[derive(Debug, Clone)]
pub struct ItemQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub dir: Dir,
    pub sprite: u16,
    pub tint: Option<u32>,
}

#[derive(Debug)]
pub struct BakedItemModel {
    pub quads: Vec<ItemQuad>,
    pub display: [ItemTransform; 9],
    pub gui_light: GuiLight,
    pub animated: bool,
}

/// The unbaked tree with every model reference replaced by baked geometry and every
/// transformation composed down from the root.
#[derive(Debug)]
pub enum BakedNode {
    Empty,
    Model {
        model: Arc<BakedItemModel>,
        tints: Vec<TintSource>,
        transform: Mat4,
    },
    Composite(Vec<BakedNode>),
    Condition {
        property: ConditionProperty,
        on_true: Box<BakedNode>,
        on_false: Box<BakedNode>,
    },
    /// `cases[i]` is the baked model of the switch's i-th case.
    // ponytail: the switch keeps its unbaked case models and lookup is a scan of `when`
    // lists; a per-property `HashMap<T, BakedNode>` is the upgrade if selects show in a profile.
    Select {
        switch: SelectSwitch,
        cases: Vec<BakedNode>,
        fallback: Box<BakedNode>,
    },
    RangeDispatch {
        property: RangeProperty,
        scale: f32,
        thresholds: Vec<f32>,
        models: Vec<BakedNode>,
        fallback: Box<BakedNode>,
    },
    Special {
        model: SpecialModel,
        properties: Arc<BakedItemModel>,
        transform: Mat4,
    },
    BundleSelectedItem,
}

impl BakedNode {
    /// The entry to draw for `value`, `None` meaning the fallback.
    pub fn range_index(thresholds: &[f32], value: f32) -> Option<usize> {
        if value.is_nan() {
            return None;
        }
        thresholds
            .partition_point(|threshold| *threshold <= value)
            .checked_sub(1)
    }
}

#[derive(Debug)]
pub struct BakedClientItem {
    pub root: BakedNode,
    pub oversized_in_gui: bool,
    pub hand_animation_on_swap: bool,
    pub swap_animation_scale: f32,
}

#[derive(Resource)]
pub struct ItemModels {
    pub by_id: HashMap<ResourceLocation, Arc<BakedClientItem>>,
    pub missing: Arc<BakedClientItem>,
    pub grass_colormap: Option<Vec<u8>>,
}

impl ItemModels {
    pub fn get(&self, id: &str) -> &Arc<BakedClientItem> {
        self.by_id.get(id).unwrap_or(&self.missing)
    }
}

struct Baker<'a> {
    pack: &'a Pack,
    sprites: &'a mut SpriteRegistry,
    models: HashMap<String, Arc<BakedItemModel>>,
    missing: Arc<BakedItemModel>,
}

/// Bakes every `items/*.json` of the pack; a file that fails to parse, validate or bake
/// fails the whole load, naming the file.
pub fn bake_all(pack: &Pack, sprites: &mut SpriteRegistry) -> Result<ItemModels, String> {
    sprites.load_atlases(pack)?;
    let missing = Arc::new(missing_model(pack, sprites)?);
    let mut baker = Baker {
        pack,
        sprites,
        models: HashMap::new(),
        missing: missing.clone(),
    };
    let mut by_id = HashMap::new();
    for (id, bytes) in pack.entries("items", "json") {
        let item = ClientItem::parse(bytes).map_err(|error| format!("items/{id}: {error}"))?;
        let root = baker
            .node(&item.model, Mat4::IDENTITY)
            .map_err(|error| format!("items/{id}: {error}"))?;
        let location = ResourceLocation::parse(&id).map_err(|error| format!("{id}: {error}"))?;
        by_id.insert(location, Arc::new(client_item(&item, root)));
    }
    Ok(ItemModels {
        by_id,
        grass_colormap: Some(load_colormap(pack, "grass")?),
        missing: Arc::new(BakedClientItem {
            root: BakedNode::Model {
                model: missing,
                tints: Vec::new(),
                transform: Mat4::IDENTITY,
            },
            oversized_in_gui: false,
            hand_animation_on_swap: true,
            swap_animation_scale: 1.0,
        }),
    })
}

fn client_item(item: &ClientItem, root: BakedNode) -> BakedClientItem {
    BakedClientItem {
        root,
        oversized_in_gui: item.oversized_in_gui,
        hand_animation_on_swap: item.hand_animation_on_swap,
        swap_animation_scale: item.swap_animation_scale,
    }
}

impl Baker<'_> {
    fn node(&mut self, unbaked: &UnbakedItemModel, parent: Mat4) -> Result<BakedNode, String> {
        Ok(match unbaked {
            UnbakedItemModel::Empty => BakedNode::Empty,
            UnbakedItemModel::BundleSelectedItem => BakedNode::BundleSelectedItem,
            UnbakedItemModel::Model {
                model,
                transformation,
                tints,
            } => BakedNode::Model {
                model: self.model(model.as_str())?,
                tints: tints.clone(),
                transform: compose(parent, transformation.as_ref()),
            },
            UnbakedItemModel::Composite {
                models,
                transformation,
            } => {
                let transform = compose(parent, transformation.as_ref());
                let mut children = models
                    .iter()
                    .map(|child| self.node(child, transform))
                    .collect::<Result<Vec<_>, _>>()?;
                match children.len() {
                    0 => BakedNode::Empty,
                    1 => children.pop().unwrap(),
                    _ => BakedNode::Composite(children),
                }
            }
            UnbakedItemModel::Condition {
                transformation,
                property,
                on_true,
                on_false,
            } => {
                let transform = compose(parent, transformation.as_ref());
                BakedNode::Condition {
                    property: property.clone(),
                    on_true: Box::new(self.node(on_true, transform)?),
                    on_false: Box::new(self.node(on_false, transform)?),
                }
            }
            UnbakedItemModel::Select {
                transformation,
                switch,
                fallback,
            } => {
                let transform = compose(parent, transformation.as_ref());
                let cases = switch
                    .case_models()
                    .into_iter()
                    .map(|model| self.node(model, transform))
                    .collect::<Result<Vec<_>, _>>()?;
                BakedNode::Select {
                    switch: switch.clone(),
                    cases,
                    fallback: Box::new(self.fallback(fallback.as_deref(), transform)?),
                }
            }
            UnbakedItemModel::RangeDispatch {
                transformation,
                property,
                scale,
                entries,
                fallback,
            } => {
                let transform = compose(parent, transformation.as_ref());
                let mut sorted: Vec<&_> = entries.iter().collect();
                sorted.sort_by(|a, b| a.threshold.total_cmp(&b.threshold));
                BakedNode::RangeDispatch {
                    property: property.clone(),
                    scale: *scale,
                    thresholds: sorted.iter().map(|entry| entry.threshold).collect(),
                    models: sorted
                        .iter()
                        .map(|entry| self.node(&entry.model, transform))
                        .collect::<Result<Vec<_>, _>>()?,
                    fallback: Box::new(self.fallback(fallback.as_deref(), transform)?),
                }
            }
            UnbakedItemModel::Special {
                base,
                transformation,
                model,
            } => {
                let baked = self.model(base.as_str())?;
                BakedNode::Special {
                    model: model.clone(),
                    properties: Arc::new(BakedItemModel {
                        quads: Vec::new(),
                        display: baked.display,
                        gui_light: baked.gui_light,
                        animated: false,
                    }),
                    transform: compose(parent, transformation.as_ref()),
                }
            }
        })
    }

    fn fallback(
        &mut self,
        fallback: Option<&UnbakedItemModel>,
        transform: Mat4,
    ) -> Result<BakedNode, String> {
        match fallback {
            Some(fallback) => self.node(fallback, transform),
            None => Ok(BakedNode::Model {
                model: self.missing.clone(),
                tints: Vec::new(),
                transform,
            }),
        }
    }

    fn model(&mut self, id: &str) -> Result<Arc<BakedItemModel>, String> {
        if let Some(model) = self.models.get(id) {
            return Ok(model.clone());
        }
        let resolved = resolve_model(self.pack, id)?;
        let model = Arc::new(bake_model(self.pack, self.sprites, &resolved)?);
        self.models.insert(id.to_string(), model.clone());
        Ok(model)
    }
}

fn bake_model(
    pack: &Pack,
    sprites: &mut SpriteRegistry,
    model: &ResolvedModel,
) -> Result<BakedItemModel, String> {
    let mut quads = Vec::new();
    let mut animated = false;
    if model.generated {
        for layer in 0..5u32 {
            let Some(texture) = model.textures.get(&format!("layer{layer}")) else {
                break;
            };
            let sprite = sprites.intern(pack, texture)?;
            animated |= sprites.is_animated(sprite);
            let side = sprites.arrays()[sprites.sprite(sprite).array as usize].size;
            let frames = sprites.frames(sprite);
            quads.extend(generator::extrude(sprite, side, &frames, layer)?);
        }
    } else {
        for element in &model.elements {
            for dir in Dir::all() {
                let Some(face) = element.faces.get(dir.name()) else {
                    continue;
                };
                if !draws_face(element, dir) {
                    continue;
                }
                let sprite = sprites.intern(pack, model.sprite_of(face)?)?;
                animated |= sprites.is_animated(sprite);
                quads.push(item_quad(element, dir, face, sprite)?);
            }
        }
    }
    Ok(BakedItemModel {
        quads,
        display: model.display,
        gui_light: model.gui_light,
        animated,
    })
}

fn item_quad(element: &Element, dir: Dir, face: &Face, sprite: u16) -> Result<ItemQuad, String> {
    let geometry = face_geometry(element, dir, face, VariantRotation::default(), false)?;
    Ok(ItemQuad {
        positions: geometry.positions,
        uvs: geometry.uvs,
        dir: geometry.facing,
        sprite,
        tint: face.tint_index,
    })
}

/// A full cube of the missing texture with no display transforms at all.
fn missing_model(pack: &Pack, sprites: &mut SpriteRegistry) -> Result<BakedItemModel, String> {
    let sprite = sprites.intern(pack, MISSING_SPRITE)?;
    let element = Element {
        from: [0.0; 3],
        to: [16.0; 3],
        rotation: None,
        shade: true,
        faces: HashMap::new(),
    };
    let face = Face {
        texture: MISSING_SPRITE.to_string(),
        uv: None,
        cullface: None,
        rotation: 0,
        tint_index: None,
    };
    let quads = Dir::all()
        .into_iter()
        .map(|dir| item_quad(&element, dir, &face, sprite))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BakedItemModel {
        quads,
        display: [ItemTransform::NONE; 9],
        gui_light: GuiLight::Side,
        animated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DISPLAY_CONTEXTS;

    fn baked() -> ItemModels {
        let mut sprites = SpriteRegistry::new();
        bake_all(Pack::corpus(), &mut sprites).unwrap()
    }

    #[test]
    fn every_item_of_the_corpus_bakes() {
        let models = baked();
        assert_eq!(models.by_id.len(), Pack::corpus().entries("items", "json").count());
        assert!(models.by_id.len() > 1600);
    }

    #[test]
    fn a_generated_item_is_extruded_and_front_lit() {
        let models = baked();
        let BakedNode::Model { model, transform, .. } = &models.get("minecraft:stick").root else {
            panic!("stick is a plain model");
        };
        assert_eq!(*transform, Mat4::IDENTITY);
        assert_eq!(model.gui_light, GuiLight::Front);
        assert!(model.quads.len() > 2);
        assert!(model.quads.iter().all(|q| q.tint == Some(0)));
        let gui = DISPLAY_CONTEXTS.iter().position(|c| *c == "gui").unwrap();
        assert_eq!(model.display[gui], ItemTransform::NONE);
    }

    #[test]
    fn a_block_item_keeps_its_cube_and_side_lighting() {
        let models = baked();
        let BakedNode::Model { model, .. } = &models.get("minecraft:stone").root else {
            panic!("stone is a plain model");
        };
        assert_eq!(model.quads.len(), 6);
        assert_eq!(model.gui_light, GuiLight::Side);
        let gui = DISPLAY_CONTEXTS.iter().position(|c| *c == "gui").unwrap();
        assert_eq!(model.display[gui].rotation_deg, Vec3::new(30.0, 225.0, 0.0));
    }

    #[test]
    fn an_unknown_id_is_the_missing_cube_without_display_transforms() {
        let models = baked();
        let BakedNode::Model { model, .. } = &models.get("minecraft:no_such_item").root else {
            panic!("missing is a plain model");
        };
        assert_eq!(model.quads.len(), 6);
        assert_eq!(model.display, [ItemTransform::NONE; 9]);
        assert_eq!(model.gui_light, GuiLight::Side);
    }

    #[test]
    fn range_entries_are_sorted_and_looked_up_by_last_threshold_at_or_below() {
        let models = baked();
        let BakedNode::Condition { on_true, .. } = &models.get("minecraft:bow").root else {
            panic!("bow switches on using_item");
        };
        let BakedNode::RangeDispatch { thresholds, models: entries, .. } = on_true.as_ref() else {
            panic!("a used bow dispatches on pull");
        };
        assert!(thresholds.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(thresholds.len(), entries.len());
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], -0.1), None);
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], 0.5), Some(1));
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], 2.0), Some(2));
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], f32::NAN), None);
    }

    #[test]
    fn a_composite_of_one_collapses_and_a_special_keeps_only_its_display() {
        let models = baked();
        assert!(matches!(
            models.get("minecraft:shield").root,
            BakedNode::Condition { .. }
        ));
        let BakedNode::Special { properties, .. } = &models.get("minecraft:conduit").root else {
            panic!("conduit is special");
        };
        assert!(properties.quads.is_empty());
    }
}
