use std::collections::HashMap;

use bevy::math::Vec3;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_mesh::variant_pick::{PartSource, VariantPick};

use crate::bake_error::BakeError;
use crate::resource_set::ResourceSet;
use mcrs_minecraft_model_file::block_state_file::{BlockStateFile, Selection, Variant};
use mcrs_minecraft_model_file::content_error::{ContentError, ContentErrorKind};
use mcrs_minecraft_model_file::face_bake::{self, ElementFace, VariantTransform};
use mcrs_minecraft_model_file::model_resolver::ModelResolver;
use mcrs_minecraft_model_file::property_table::PropertyTable;
use mcrs_minecraft_model_file::resource_id::resource_path;

/// One face of a block in block space, with the sprite it draws as an index into the sprites of
/// its [`BakedBlock`].
#[derive(Clone, Debug, PartialEq)]
pub struct BakedFace {
    pub face: ElementFace,
    pub sprite: usize,
    pub dye: Option<u8>,
    /// The pass the face must draw in, when its texture alone does not decide it.
    pub pass: Option<Pass>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BakedBlock {
    pub quads: Vec<BakedFace>,
    pub sprites: Vec<String>,
    pub ambient_occlusion: bool,
}

/// The index of `id` in `sprites`, added at the end when it is new.
pub fn intern(sprites: &mut Vec<String>, id: &str) -> usize {
    match sprites.iter().position(|sprite| sprite == id) {
        Some(index) => index,
        None => {
            sprites.push(id.to_owned());
            sprites.len() - 1
        }
    }
}

/// The direction nearest to a normal, `None` for a normal with no length.
#[must_use]
pub fn nearest(normal: Vec3) -> Option<Direction> {
    let normal = normal.as_dvec3();
    (normal.length_squared() > 0.0 && normal.is_finite()).then(|| face_bake::nearest(normal))
}

/// The selected parts of a state, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct StateParts {
    pub source: PartSource,
    pub parts: Vec<Vec<Variant>>,
}

impl StateParts {
    fn missing() -> Self {
        Self {
            source: PartSource::Variants,
            parts: vec![vec![Variant::missing()]],
        }
    }

    /// The pick of the state, with the seed rule that its properties give.
    #[must_use]
    pub fn pick(&self, properties: &PropertyTable, state: &[usize]) -> VariantPick {
        VariantPick {
            seed_offset: seed_offset(properties, state),
            source: self.source,
            weights: self
                .parts
                .iter()
                .map(|part| part.iter().map(|variant| variant.weight).collect())
                .collect(),
        }
    }
}

/// The seed rule of a state: the upper half of a two-block-high block and the foot of a bed
/// seed from their other half.
#[must_use]
pub fn seed_offset(properties: &PropertyTable, state: &[usize]) -> [i32; 3] {
    let has_values = |name: &str, values: [&str; 2]| {
        properties.index_of(name).is_some_and(|index| {
            properties.properties.get(index).is_some_and(|domain| {
                values
                    .iter()
                    .all(|value| domain.values.iter().any(|known| known == value))
            })
        })
    };
    let value = |name: &str| properties.value_of(state, name);
    if has_values("half", ["upper", "lower"]) {
        return match value("half") {
            Some("upper") => [0, -1, 0],
            _ => [0, 0, 0],
        };
    }
    if has_values("part", ["head", "foot"]) && value("part") == Some("foot") {
        return match value("facing") {
            Some("north") => [0, 0, -1],
            Some("south") => [0, 0, 1],
            Some("east") => [1, 0, 0],
            Some("west") => [-1, 0, 0],
            _ => [0, 0, 0],
        };
    }
    [0, 0, 0]
}

/// Reads blockstate files and model files from a resource set and bakes the faces of block
/// states, each file once.
pub struct BlockModels<'a> {
    set: &'a ResourceSet,
    resolver: ModelResolver<'a>,
    block_states: HashMap<String, Option<BlockStateFile>>,
    errors: Vec<ContentError>,
}

impl<'a> BlockModels<'a> {
    #[must_use]
    pub fn new(set: &'a ResourceSet) -> Self {
        Self {
            set,
            resolver: ModelResolver::new(set.files()),
            block_states: HashMap::new(),
            errors: Vec::new(),
        }
    }

    #[must_use]
    pub fn set(&self) -> &'a ResourceSet {
        self.set
    }

    pub fn resolver(&mut self) -> &mut ModelResolver<'a> {
        &mut self.resolver
    }

    /// The content errors found so far, each naming its file.
    pub fn take_errors(&mut self) -> Vec<ContentError> {
        let mut errors = std::mem::take(&mut self.errors);
        errors.extend(self.resolver.take_errors());
        errors
    }

    /// The selected parts of the state of `block` with the value indices `state`.
    pub fn parts(
        &mut self,
        block: &str,
        properties: &PropertyTable,
        state: &[usize],
    ) -> StateParts {
        let pack = self.resolver.pack();
        let file = self
            .block_states
            .entry(block.to_owned())
            .or_insert_with(|| {
                let path = resource_path(block, "blockstates", "json");
                let Some(bytes) = pack.get(&path) else {
                    self.errors.push(ContentError {
                        path,
                        kind: ContentErrorKind::NoBlockState,
                    });
                    return None;
                };
                match BlockStateFile::parse(bytes, properties) {
                    Ok(file) => {
                        self.errors
                            .extend(file.errors().iter().map(|error| ContentError {
                                path: path.clone(),
                                kind: ContentErrorKind::BlockState(error.clone()),
                            }));
                        Some(file)
                    }
                    Err(error) => {
                        self.errors.push(ContentError {
                            path,
                            kind: ContentErrorKind::BlockState(error),
                        });
                        None
                    }
                }
            });
        let Some(file) = file else {
            return StateParts::missing();
        };
        match file.select(state) {
            Selection::Variants(part) => StateParts {
                source: PartSource::Variants,
                parts: vec![part.to_vec()],
            },
            Selection::Multipart(parts) => StateParts {
                source: PartSource::Multipart,
                parts: parts.into_iter().map(<[Variant]>::to_vec).collect(),
            },
            Selection::Missing => {
                self.errors.push(ContentError {
                    path: block.to_owned(),
                    kind: ContentErrorKind::NoPart {
                        state: state.to_vec(),
                    },
                });
                StateParts::missing()
            }
        }
    }

    /// The faces of the variants, in order, with the ambient occlusion flag of the first.
    pub fn bake(&mut self, variants: &[&Variant]) -> Result<BakedBlock, BakeError> {
        let mut baked = BakedBlock::default();
        for (index, variant) in variants.iter().enumerate() {
            let model = self.resolver.resolve(&variant.model);
            if index == 0 {
                baked.ambient_occlusion = model.ambient_occlusion;
            }
            let faces = face_bake::bake_elements(&model.elements, VariantTransform::from(*variant))
                .map_err(|cause| BakeError::Face {
                    model: variant.model.clone(),
                    cause,
                })?;
            for face in faces {
                let texture = model.sprite(&face.texture);
                baked.quads.push(BakedFace {
                    sprite: intern(&mut baked.sprites, &texture.sprite),
                    dye: None,
                    pass: texture.force_translucent.then_some(Pass::Translucent),
                    face,
                });
            }
        }
        Ok(baked)
    }

    /// The faces of the state of `block` with these property values, at `position`.
    pub fn bake_state(
        &mut self,
        block: &str,
        properties: &PropertyTable,
        values: &[(&str, &str)],
        position: [i32; 3],
    ) -> Result<BakedBlock, BakeError> {
        let state = properties
            .state(values)
            .ok_or_else(|| BakeError::NotAState {
                state: format!("{block}{values:?}"),
            })?;
        let parts = self.parts(block, properties, &state);
        let chosen = parts.pick(properties, &state).chosen(position);
        let variants: Vec<&Variant> = parts
            .parts
            .iter()
            .zip(chosen)
            .filter_map(|(part, index)| part.get(index))
            .collect();
        self.bake(&variants)
    }
}
