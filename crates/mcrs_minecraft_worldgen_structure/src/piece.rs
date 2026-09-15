use std::fmt;

use bevy_math::IVec3;
use mcrs_minecraft_core::{BoundingBox, ResourceLocation, Rotation, rotation};
use mcrs_minecraft_nbt::{nbt_flag, nbt_int_array};
use mcrs_minecraft_worldgen_feature::template::Projection;
use serde::de::{DeserializeSeed, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::frozen::{
    ElementId, FrozenElement, FrozenStructures, StructureId, StructureKind, TemplateId,
};
use crate::orient::Orientation;
use crate::{LiquidSettings, PoolElement, SingleElement, TerrainAdaptation};

pub const TERRAIN_MARGIN: i32 = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Junction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
    pub delta_y: i32,
    #[serde(rename = "dest_proj")]
    pub dest_projection: Projection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JigsawPiece {
    pub element: ElementId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    pub projection: Projection,
    pub ground_level_delta: i32,
    pub junctions: Vec<Junction>,
}

/// The one piece of a desert pyramid: its box as laid out, at the fixed floor
/// of 64, and the ground it is sunk to at placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesertPyramidPiece {
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    /// The lowest ground under the box, which the reference reads from the
    /// live heightmap of the first decorating chunk and this layout fixes from
    /// the density heights.
    pub height_position: i32,
}

impl DesertPyramidPiece {
    pub const WIDTH: i32 = 21;
    pub const HEIGHT: i32 = 15;
    pub const DEPTH: i32 = 21;
    pub const LAYOUT_FLOOR: i32 = 64;
}

/// The one piece of a buried treasure: the block at (9, 90, 9) of its chunk,
/// from which placement walks the column down to the chest's resting block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuriedTreasurePiece {
    pub bounds: BoundingBox,
}

impl BuriedTreasurePiece {
    pub const LAYOUT_Y: i32 = 90;
    pub const CHUNK_OFFSET: i32 = 9;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Jigsaw(JigsawPiece),
    DesertPyramid(DesertPyramidPiece),
    BuriedTreasure(BuriedTreasurePiece),
}

impl Piece {
    pub fn bounds(&self) -> BoundingBox {
        match self {
            Piece::Jigsaw(piece) => piece.bounds,
            Piece::DesertPyramid(piece) => piece.bounds,
            Piece::BuriedTreasure(piece) => piece.bounds,
        }
    }

    /// `StructurePiece.move`, which for a jigsaw piece carries its position too.
    pub fn move_by(&mut self, delta: IVec3) {
        match self {
            Piece::Jigsaw(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
            Piece::DesertPyramid(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::BuriedTreasure(piece) => piece.bounds = piece.bounds.moved(delta),
        }
    }

    pub fn nbt<'a>(&'a self, context: &'a PieceContext<'a>) -> PieceNbt<'a> {
        PieceNbt {
            piece: self,
            context,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub structure: StructureId,
    pub pieces: Vec<Piece>,
    pub bounds: BoundingBox,
}

impl Start {
    pub fn new(frozen: &FrozenStructures, structure: StructureId, pieces: Vec<Piece>) -> Self {
        let union = pieces
            .iter()
            .map(Piece::bounds)
            .reduce(BoundingBox::union)
            .expect("a start has at least one piece");
        let bounds =
            if frozen.structures[structure.0 as usize].adaptation == TerrainAdaptation::None {
                union
            } else {
                union.inflated(TERRAIN_MARGIN)
            };
        Start {
            structure,
            pieces,
            bounds,
        }
    }
}

/// What the reference's `StructurePieceSerializationContext` supplies: the
/// tables that turn the ids a piece holds back into the names the save carries.
#[derive(Clone, Copy)]
pub struct PieceContext<'a> {
    pub frozen: &'a FrozenStructures,
    pub structure: StructureId,
}

impl PieceContext<'_> {
    fn liquid_settings(&self) -> LiquidSettings {
        match &self.frozen.structures[self.structure.0 as usize].kind {
            StructureKind::Jigsaw { config, .. } => config.liquid_settings,
            _ => LiquidSettings::default(),
        }
    }

    fn template_name(&self, template: TemplateId) -> ResourceLocation {
        self.frozen
            .template_ids
            .iter()
            .find(|(_, id)| **id == template)
            .map(|(location, _)| location.clone())
            .expect("every frozen template has a name")
    }

    fn pool_element(&self, element: ElementId) -> PoolElement {
        match &self.frozen.elements[element.0 as usize] {
            FrozenElement::Single {
                template,
                legacy,
                processors,
                projection,
                liquid_settings,
            } => {
                let single = SingleElement {
                    location: self.template_name(*template),
                    processors: processors.clone(),
                    projection: *projection,
                    override_liquid_settings: *liquid_settings,
                };
                if *legacy {
                    PoolElement::LegacySingle(single)
                } else {
                    PoolElement::Single(single)
                }
            }
            FrozenElement::List {
                elements,
                projection,
            } => PoolElement::List {
                elements: elements.iter().map(|id| self.pool_element(*id)).collect(),
                projection: *projection,
            },
            FrozenElement::Feature {
                feature,
                projection,
            } => PoolElement::Feature {
                feature: feature.clone(),
                projection: *projection,
            },
            FrozenElement::Empty => PoolElement::Empty {},
        }
    }

    // ponytail: a scan of every frozen element per loaded piece; an index keyed
    // on element content if loading a saved region ever shows up in a profile.
    fn element_id<E: serde::de::Error>(&self, wanted: &PoolElement) -> Result<ElementId, E> {
        let frozen = self.frozen;
        (0..frozen.elements.len())
            .map(|index| ElementId(index as u32))
            .find(|id| self.element_matches(*id, wanted))
            .ok_or_else(|| E::custom("the pool element is not one any loaded pool holds"))
    }

    fn element_matches(&self, id: ElementId, wanted: &PoolElement) -> bool {
        match (&self.frozen.elements[id.0 as usize], wanted) {
            (
                FrozenElement::Single {
                    template,
                    legacy,
                    processors,
                    projection,
                    liquid_settings,
                },
                PoolElement::Single(single) | PoolElement::LegacySingle(single),
            ) => {
                self.frozen.template_ids.get(&single.location) == Some(template)
                    && *legacy == matches!(wanted, PoolElement::LegacySingle(_))
                    && *processors == single.processors
                    && *projection == single.projection
                    && *liquid_settings == single.override_liquid_settings
            }
            (
                FrozenElement::List {
                    elements,
                    projection,
                },
                PoolElement::List {
                    elements: wanted,
                    projection: wanted_projection,
                },
            ) => {
                projection == wanted_projection
                    && elements.len() == wanted.len()
                    && elements
                        .iter()
                        .zip(wanted)
                        .all(|(id, element)| self.element_matches(*id, element))
            }
            (
                FrozenElement::Feature {
                    feature,
                    projection,
                },
                PoolElement::Feature {
                    feature: wanted,
                    projection: wanted_projection,
                },
            ) => feature == wanted && projection == wanted_projection,
            (FrozenElement::Empty, PoolElement::Empty {}) => true,
            _ => false,
        }
    }
}

/// `StructurePiece.createTag` plus each type's `addAdditionalSaveData`; `id`
/// picks the type, so the save reads back whatever order its keys came in.
#[derive(Serialize, Deserialize)]
#[serde(tag = "id", deny_unknown_fields)]
enum PieceTag {
    #[serde(rename = "minecraft:jigsaw")]
    Jigsaw {
        #[serde(rename = "BB", serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        #[serde(rename = "O")]
        orientation: i32,
        #[serde(rename = "GD")]
        gen_depth: i32,
        #[serde(rename = "PosX")]
        pos_x: i32,
        #[serde(rename = "PosY")]
        pos_y: i32,
        #[serde(rename = "PosZ")]
        pos_z: i32,
        ground_level_delta: i32,
        pool_element: PoolElement,
        #[serde(with = "rotation::legacy")]
        rotation: Rotation,
        junctions: Vec<Junction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        liquid_settings: Option<LiquidSettings>,
    },
    #[serde(rename = "minecraft:tedp")]
    DesertPyramid {
        #[serde(rename = "BB", serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        #[serde(rename = "O")]
        orientation: i32,
        #[serde(rename = "GD")]
        gen_depth: i32,
        #[serde(rename = "Width")]
        width: i32,
        #[serde(rename = "Height")]
        height: i32,
        #[serde(rename = "Depth")]
        depth: i32,
        #[serde(rename = "HPos")]
        height_position: i32,
        #[serde(rename = "hasPlacedChest0", deserialize_with = "nbt_flag")]
        has_placed_chest_0: bool,
        #[serde(rename = "hasPlacedChest1", deserialize_with = "nbt_flag")]
        has_placed_chest_1: bool,
        #[serde(rename = "hasPlacedChest2", deserialize_with = "nbt_flag")]
        has_placed_chest_2: bool,
        #[serde(rename = "hasPlacedChest3", deserialize_with = "nbt_flag")]
        has_placed_chest_3: bool,
    },
    #[serde(rename = "minecraft:btp")]
    BuriedTreasure {
        #[serde(rename = "BB", serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        #[serde(rename = "O")]
        orientation: i32,
        #[serde(rename = "GD")]
        gen_depth: i32,
    },
}

const NO_ORIENTATION: i32 = -1;

fn box_array(bounds: BoundingBox) -> [i32; 6] {
    [
        bounds.min.x,
        bounds.min.y,
        bounds.min.z,
        bounds.max.x,
        bounds.max.y,
        bounds.max.z,
    ]
}

fn box_of(array: [i32; 6]) -> BoundingBox {
    BoundingBox {
        min: IVec3::new(array[0], array[1], array[2]).into(),
        max: IVec3::new(array[3], array[4], array[5]).into(),
    }
}

pub struct PieceNbt<'a> {
    piece: &'a Piece,
    context: &'a PieceContext<'a>,
}

impl Serialize for PieceNbt<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let tag = match self.piece {
            Piece::Jigsaw(piece) => {
                let liquid = self.context.liquid_settings();
                PieceTag::Jigsaw {
                    bounds: box_array(piece.bounds),
                    orientation: NO_ORIENTATION,
                    gen_depth: 0,
                    pos_x: piece.position.x,
                    pos_y: piece.position.y,
                    pos_z: piece.position.z,
                    ground_level_delta: piece.ground_level_delta,
                    pool_element: self.context.pool_element(piece.element),
                    rotation: piece.rotation,
                    junctions: piece.junctions.clone(),
                    liquid_settings: (liquid != LiquidSettings::default()).then_some(liquid),
                }
            }
            Piece::DesertPyramid(piece) => PieceTag::DesertPyramid {
                bounds: box_array(piece.bounds),
                orientation: piece.orientation.data_2d(),
                gen_depth: 0,
                width: DesertPyramidPiece::WIDTH,
                height: DesertPyramidPiece::HEIGHT,
                depth: DesertPyramidPiece::DEPTH,
                height_position: piece.height_position,
                has_placed_chest_0: false,
                has_placed_chest_1: false,
                has_placed_chest_2: false,
                has_placed_chest_3: false,
            },
            Piece::BuriedTreasure(piece) => PieceTag::BuriedTreasure {
                bounds: box_array(piece.bounds),
                orientation: NO_ORIENTATION,
                gen_depth: 0,
            },
        };
        tag.serialize(serializer)
    }
}

impl fmt::Debug for PieceNbt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.piece.fmt(f)
    }
}

/// Reads one piece, resolving the names it carries against the context's
/// tables; the box is read as written, so a piece the expansion hack stretched
/// keeps its stretched box.
pub struct PieceSeed<'a>(pub PieceContext<'a>);

impl<'de> DeserializeSeed<'de> for PieceSeed<'_> {
    type Value = Piece;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Piece, D::Error> {
        match PieceTag::deserialize(deserializer)? {
            PieceTag::Jigsaw {
                bounds,
                pos_x,
                pos_y,
                pos_z,
                ground_level_delta,
                pool_element,
                rotation,
                junctions,
                ..
            } => {
                let element = self.0.element_id(&pool_element)?;
                let projection = self.0.frozen.elements[element.0 as usize]
                    .projection()
                    .ok_or_else(|| D::Error::custom("a jigsaw piece of an empty element"))?;
                Ok(Piece::Jigsaw(JigsawPiece {
                    element,
                    position: IVec3::new(pos_x, pos_y, pos_z),
                    rotation,
                    bounds: box_of(bounds),
                    projection,
                    ground_level_delta,
                    junctions,
                }))
            }
            PieceTag::DesertPyramid {
                bounds,
                orientation,
                height_position,
                ..
            } => Ok(Piece::DesertPyramid(DesertPyramidPiece {
                bounds: box_of(bounds),
                orientation: Orientation::from_data_2d(orientation)
                    .ok_or_else(|| D::Error::custom("a desert pyramid without an orientation"))?,
                height_position,
            })),
            PieceTag::BuriedTreasure { bounds, .. } => {
                Ok(Piece::BuriedTreasure(BuriedTreasurePiece {
                    bounds: box_of(bounds),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::frozen::{FrozenStructure, PoolId};
    use crate::{DecorationStep, Structure};
    use mcrs_minecraft_core::BlockPos;
    use mcrs_minecraft_nbt::deserializer::Deserializer as NbtDeserializer;
    use mcrs_minecraft_nbt::{to_bytes, to_nbt_compound};
    use mcrs_minecraft_worldgen_feature::proto::Holder;

    fn frozen(liquid_settings: LiquidSettings) -> FrozenStructures {
        let mut frozen = FrozenStructures::default();
        let house = ResourceLocation::parse("minecraft:village/plains/houses/house_1").unwrap();
        let empty = ResourceLocation::parse("minecraft:empty").unwrap();
        frozen.template_ids.insert(house, TemplateId(0));
        let single = |legacy: bool| FrozenElement::Single {
            template: TemplateId(0),
            legacy,
            processors: Holder::Reference(empty.clone()),
            projection: Projection::Rigid,
            liquid_settings: None,
        };
        frozen.elements.push(single(false));
        frozen.elements.push(single(true));
        frozen.elements.push(FrozenElement::List {
            elements: vec![ElementId(1)],
            projection: Projection::Rigid,
        });
        frozen.elements.push(FrozenElement::Single {
            template: TemplateId(0),
            legacy: false,
            processors: Holder::Reference(empty),
            projection: Projection::TerrainMatching,
            liquid_settings: Some(LiquidSettings::IgnoreWaterlogging),
        });
        let json = format!(
            r##"{{"type":"minecraft:jigsaw","biomes":"#minecraft:x","spawn_overrides":{{}},"step":"surface_structures","start_pool":"minecraft:p","size":1,"start_height":{{"absolute":0}},"use_expansion_hack":true,"max_distance_from_center":80,"liquid_settings":"{}"}}"##,
            serde_json::to_value(liquid_settings)
                .unwrap()
                .as_str()
                .unwrap()
        );
        let Structure::Jigsaw { jigsaw, .. } = serde_json::from_str(&json).unwrap() else {
            unreachable!()
        };
        frozen.structures.push(FrozenStructure {
            id: ResourceLocation::parse("minecraft:village_plains").unwrap(),
            step: DecorationStep::SurfaceStructures,
            step_index: 0,
            adaptation: TerrainAdaptation::BeardThin,
            biomes: Default::default(),
            kind: StructureKind::Jigsaw {
                start_pool: PoolId(0),
                config: jigsaw,
            },
        });
        frozen
    }

    fn piece(element: u32, projection: Projection) -> Piece {
        Piece::Jigsaw(JigsawPiece {
            element: ElementId(element),
            position: IVec3::new(-17, 64, 1030),
            rotation: Rotation::Clockwise90,
            bounds: BoundingBox {
                min: BlockPos::new(-25, 63, 1030),
                max: BlockPos::new(-17, 90, 1041),
            },
            projection,
            ground_level_delta: 1,
            junctions: vec![Junction {
                source_x: -16,
                source_ground_y: 65,
                source_z: 1033,
                delta_y: -2,
                dest_projection: Projection::TerrainMatching,
            }],
        })
    }

    fn round_trip(context: &PieceContext<'_>, piece: &Piece) -> Piece {
        let mut bytes = Vec::new();
        to_bytes(&piece.nbt(context), &mut bytes).unwrap();
        PieceSeed(*context)
            .deserialize(&mut NbtDeserializer::new(Cursor::new(bytes), true))
            .unwrap()
    }

    #[test]
    fn a_jigsaw_piece_round_trips_through_nbt_as_the_reference_writes_it() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        for (element, projection) in [
            (0, Projection::Rigid),
            (1, Projection::Rigid),
            (2, Projection::Rigid),
            (3, Projection::TerrainMatching),
        ] {
            let piece = piece(element, projection);
            assert_eq!(round_trip(&context, &piece), piece);
        }

        let tag = to_nbt_compound(&piece(1, Projection::Rigid).nbt(&context)).unwrap();
        assert_eq!(tag.get_string("id"), Some("minecraft:jigsaw"));
        assert_eq!(
            tag.get_int_array("BB").map(<[i32]>::to_vec),
            Some(vec![-25, 63, 1030, -17, 90, 1041])
        );
        assert_eq!(tag.get_int("O"), Some(-1));
        assert_eq!(tag.get_int("GD"), Some(0));
        assert_eq!(tag.get_int("PosX"), Some(-17));
        assert_eq!(tag.get_int("ground_level_delta"), Some(1));
        assert_eq!(tag.get_string("rotation"), Some("CLOCKWISE_90"));
        assert!(tag.get_string("liquid_settings").is_none());
        let element = tag.get_compound("pool_element").unwrap();
        assert_eq!(
            element.get_string("element_type"),
            Some("minecraft:legacy_single_pool_element")
        );
        assert_eq!(
            element.get_string("location"),
            Some("minecraft:village/plains/houses/house_1")
        );
        assert_eq!(element.get_string("processors"), Some("minecraft:empty"));
        assert_eq!(element.get_string("projection"), Some("rigid"));
        let junction = &tag.get_list("junctions").unwrap()[0];
        let junction = junction.extract_compound().unwrap();
        assert_eq!(junction.get_int("source_ground_y"), Some(65));
        assert_eq!(junction.get_string("dest_proj"), Some("terrain_matching"));
    }

    #[test]
    fn a_desert_pyramid_piece_round_trips_with_its_constant_fields() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let piece = Piece::DesertPyramid(DesertPyramidPiece {
            bounds: BoundingBox {
                min: BlockPos::new(-32, 64, 48),
                max: BlockPos::new(-12, 78, 68),
            },
            orientation: Orientation::West,
            height_position: 71,
        });
        assert_eq!(round_trip(&context, &piece), piece);
        let tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
        assert_eq!(tag.get_string("id"), Some("minecraft:tedp"));
        assert_eq!(tag.get_int("O"), Some(1));
        assert_eq!(tag.get_int("GD"), Some(0));
        assert_eq!(tag.get_int("Width"), Some(21));
        assert_eq!(tag.get_int("Height"), Some(15));
        assert_eq!(tag.get_int("Depth"), Some(21));
        assert_eq!(tag.get_int("HPos"), Some(71));
        assert_eq!(tag.get_byte("hasPlacedChest3"), Some(0));
    }

    #[test]
    fn a_buried_treasure_piece_round_trips_as_its_box_alone() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let piece = Piece::BuriedTreasure(BuriedTreasurePiece {
            bounds: BoundingBox::point(BlockPos::new(-23, 90, 41)),
        });
        assert_eq!(round_trip(&context, &piece), piece);
        let tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
        assert_eq!(tag.get_string("id"), Some("minecraft:btp"));
        assert_eq!(
            tag.get_int_array("BB").map(<[i32]>::to_vec),
            Some(vec![-23, 90, 41, -23, 90, 41])
        );
        assert_eq!(tag.get_int("O"), Some(-1));
        assert_eq!(tag.get_int("GD"), Some(0));
        assert_eq!(tag.child_tags.len(), 4);
    }

    #[test]
    fn the_structure_liquid_settings_are_written_only_off_the_default() {
        let frozen = frozen(LiquidSettings::IgnoreWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let piece = piece(0, Projection::Rigid);
        let tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
        assert_eq!(
            tag.get_string("liquid_settings"),
            Some("ignore_waterlogging")
        );
        assert_eq!(round_trip(&context, &piece), piece);
    }

    #[test]
    fn a_piece_naming_an_unloaded_template_or_element_is_refused() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        for element in [
            r#"{"element_type":"minecraft:single_pool_element","location":"minecraft:nowhere","processors":"minecraft:empty","projection":"rigid"}"#,
            r#"{"element_type":"minecraft:single_pool_element","location":"minecraft:village/plains/houses/house_1","processors":"minecraft:mossify_10_percent","projection":"rigid"}"#,
        ] {
            let json = format!(
                r#"{{"id":"minecraft:jigsaw","BB":[0,0,0,1,1,1],"O":-1,"GD":0,"PosX":0,"PosY":0,"PosZ":0,"ground_level_delta":1,"pool_element":{element},"rotation":"NONE","junctions":[]}}"#
            );
            let mut deserializer = serde_json::Deserializer::from_str(&json);
            assert!(PieceSeed(context).deserialize(&mut deserializer).is_err());
        }
    }
}
