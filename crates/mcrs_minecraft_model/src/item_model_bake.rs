use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf};
use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::{DVec3, Mat4, Vec3};
use bevy::prelude::Resource;
use mcrs_minecraft_core::Direction;

use crate::atlas::{MISSING_SPRITE, SpriteRegistry};
use crate::colormap::load_colormap;
use crate::item_model_asset::{
    ClientItem, ConditionProperty, RangeProperty, SelectSwitch, SpecialModel, TintSource,
    UnbakedItemModel,
};
use crate::item_model_generator;
use crate::item_model_transform::compose;
use crate::resource_set::ResourceSet;
use mcrs_minecraft_model_file::face_bake::{self, VariantTransform};
use mcrs_minecraft_model_file::model_file::{
    DisplayContext, DisplayTransform, Element, FaceEntry, GuiLight,
};
use mcrs_minecraft_model_file::model_resolver::{MergedModel, ModelResolver};

#[derive(Debug, Clone)]
pub struct ItemQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub dir: Direction,
    pub sprite: u16,
    pub tint: Option<u32>,
}

#[derive(Debug)]
pub struct BakedItemModel {
    pub quads: Vec<ItemQuad>,
    /// The `gui` display transform, the identity when the model declares none.
    pub display: DisplayTransform,
    pub gui_light: GuiLight,
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

#[derive(Resource)]
pub struct ItemModels {
    pub by_id: hashbrown::HashMap<ResourceLocationBuf, Arc<BakedNode>>,
    pub missing: Arc<BakedNode>,
    pub grass_colormap: Option<Vec<u8>>,
}

impl ItemModels {
    pub fn get(&self, id: &str) -> &BakedNode {
        ResourceLocation::read(id)
            .ok()
            .and_then(|id| self.by_id.get(&id))
            .unwrap_or(&self.missing)
    }
}

struct Baker<'a> {
    pack: &'a ResourceSet,
    resolver: ModelResolver<'a>,
    sprites: &'a mut SpriteRegistry,
    models: HashMap<String, Arc<BakedItemModel>>,
    missing: Arc<BakedItemModel>,
}

/// Bakes every `items/*.json` of the pack; a file that fails to parse, validate or bake
/// fails the whole load, naming the file.
pub fn bake_all(pack: &ResourceSet, sprites: &mut SpriteRegistry) -> Result<ItemModels, String> {
    sprites
        .load_atlases(pack)
        .map_err(|error| error.to_string())?;
    let missing = Arc::new(missing_model(pack, sprites)?);
    let mut baker = Baker {
        pack,
        resolver: ModelResolver::new(pack.files()),
        sprites,
        models: HashMap::new(),
        missing: missing.clone(),
    };
    let mut by_id = hashbrown::HashMap::new();
    for (id, bytes) in pack.entries("items", "json") {
        let item = ClientItem::parse(bytes).map_err(|error| format!("items/{id}: {error}"))?;
        let root = baker
            .node(&item.model, Mat4::IDENTITY)
            .map_err(|error| format!("items/{id}: {error}"))?;
        let location = ResourceLocationBuf::read(&id).map_err(|error| format!("{id}: {error}"))?;
        by_id.insert(location, Arc::new(root));
    }
    Ok(ItemModels {
        by_id,
        grass_colormap: Some(load_colormap(pack, "grass").map_err(|error| error.to_string())?),
        missing: Arc::new(BakedNode::Model {
            model: missing,
            tints: Vec::new(),
            transform: Mat4::IDENTITY,
        }),
    })
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
                    1 => children.pop().unwrap_or(BakedNode::Empty),
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
        let pack = self.pack;
        let resolved = self.resolver.resolve(id);
        let model = Arc::new(bake_model(pack, self.sprites, resolved)?);
        self.models.insert(id.to_string(), model.clone());
        Ok(model)
    }
}

fn bake_model(
    pack: &ResourceSet,
    sprites: &mut SpriteRegistry,
    model: &MergedModel,
) -> Result<BakedItemModel, String> {
    let mut quads = Vec::new();
    if model.generated {
        for layer in 0..5u32 {
            let Some(texture) = model.textures.get(&format!("layer{layer}")) else {
                break;
            };
            let sprite = sprites
                .intern(pack, &texture.sprite)
                .map_err(|error| error.to_string())?;
            let side = sprites
                .sprite(sprite)
                .and_then(|interned| sprites.arrays().get(usize::from(interned.array)))
                .ok_or_else(|| format!("{} names no texture array", texture.sprite))?
                .size;
            let frames = sprites.frames(sprite);
            quads.extend(item_model_generator::extrude(sprite, side, &frames, layer)?);
        }
    } else {
        for face in face_bake::bake_elements(&model.elements, VariantTransform::default())
            .map_err(|error| error.to_string())?
        {
            let sprite = sprites
                .intern(pack, &model.sprite(&face.texture).sprite)
                .map_err(|error| error.to_string())?;
            quads.push(ItemQuad {
                positions: face.positions,
                uvs: face.uvs,
                dir: face.direction,
                sprite,
                tint: face.tint,
            });
        }
    }
    Ok(BakedItemModel {
        quads,
        display: model
            .display(DisplayContext::Gui)
            .unwrap_or(DisplayTransform::IDENTITY),
        gui_light: model.gui_light,
    })
}

/// The quads of the faces of one element, each drawing `sprite`.
pub(super) fn element_quads(element: Element, sprite: u16) -> Result<Vec<ItemQuad>, String> {
    let faces = face_bake::bake_elements(&[element], VariantTransform::default())
        .map_err(|error| error.to_string())?;
    Ok(faces
        .into_iter()
        .map(|face| ItemQuad {
            positions: face.positions,
            uvs: face.uvs,
            dir: face.direction,
            sprite,
            tint: face.tint,
        })
        .collect())
}

/// An element from `from` to `to` in pixel space with no turn and one face entry per key.
pub(super) fn plain_element(from: DVec3, to: DVec3, faces: [Option<FaceEntry>; 6]) -> Element {
    Element {
        from,
        to,
        rotation: None,
        faces,
        shade_direction_override: None,
        light_emission: 0,
    }
}

/// A full cube of the missing texture with no display transforms at all.
fn missing_model(
    pack: &ResourceSet,
    sprites: &mut SpriteRegistry,
) -> Result<BakedItemModel, String> {
    let sprite = sprites
        .intern(pack, MISSING_SPRITE)
        .map_err(|error| error.to_string())?;
    let face = FaceEntry {
        texture: MISSING_SPRITE.to_string(),
        uv: None,
        cullface: None,
        rotation: 0,
        tint: None,
    };
    let element = plain_element(
        DVec3::ZERO,
        DVec3::splat(16.0),
        std::array::from_fn(|_| Some(face.clone())),
    );
    Ok(BakedItemModel {
        quads: element_quads(element, sprite)?,
        display: DisplayTransform::IDENTITY,
        gui_light: GuiLight::Side,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    mod jar {
        use super::*;

        #[test]
        fn every_item_of_the_corpus_bakes() {
            let models = bake_all(ResourceSet::corpus(), &mut SpriteRegistry::default()).unwrap();
            assert_eq!(
                models.by_id.len(),
                ResourceSet::corpus().entries("items", "json").count()
            );
            a_generated_item_is_extruded_and_front_lit(&models);
            an_unknown_id_is_the_missing_cube_without_display_transforms(&models);
            range_entries_are_sorted_and_looked_up_by_last_threshold_at_or_below(&models);
            a_composite_of_one_collapses_and_a_special_keeps_only_its_display(&models);
        }
    }

    fn a_generated_item_is_extruded_and_front_lit(models: &ItemModels) {
        let BakedNode::Model {
            model, transform, ..
        } = models.get("minecraft:stick")
        else {
            panic!("stick is a plain model");
        };
        assert_eq!(*transform, Mat4::IDENTITY);
        assert_eq!(model.gui_light, GuiLight::Front);
        assert!(model.quads.len() > 2);
        assert!(model.quads.iter().all(|q| q.tint == Some(0)));
        assert_eq!(model.display, DisplayTransform::IDENTITY);
    }

    fn an_unknown_id_is_the_missing_cube_without_display_transforms(models: &ItemModels) {
        let BakedNode::Model { model, .. } = models.get("minecraft:no_such_item") else {
            panic!("missing is a plain model");
        };
        assert_eq!(model.quads.len(), 6);
        assert_eq!(model.display, DisplayTransform::IDENTITY);
        assert_eq!(model.gui_light, GuiLight::Side);
    }

    fn range_entries_are_sorted_and_looked_up_by_last_threshold_at_or_below(models: &ItemModels) {
        let BakedNode::Condition { on_true, .. } = models.get("minecraft:bow") else {
            panic!("bow switches on using_item");
        };
        let BakedNode::RangeDispatch {
            thresholds,
            models: entries,
            ..
        } = on_true.as_ref()
        else {
            panic!("a used bow dispatches on pull");
        };
        assert!(thresholds.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(thresholds.len(), entries.len());
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], -0.1), None);
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], 0.5), Some(1));
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], 2.0), Some(2));
        assert_eq!(BakedNode::range_index(&[0.0, 0.5, 0.9], f32::NAN), None);
    }

    fn a_composite_of_one_collapses_and_a_special_keeps_only_its_display(models: &ItemModels) {
        assert!(matches!(
            models.get("minecraft:shield"),
            BakedNode::Condition { .. }
        ));
        let BakedNode::Special { properties, .. } = models.get("minecraft:conduit") else {
            panic!("conduit is special");
        };
        assert!(properties.quads.is_empty());
    }
}
