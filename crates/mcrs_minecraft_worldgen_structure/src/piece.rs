pub use mcrs_minecraft_anvil::piece::{
    DoorTag, GridTag, Junction, PortalProperties, SavedPiece, SmallDoor, StairsDownTag,
};
use mcrs_minecraft_anvil::piece::{
    MonumentBuildingTag, MonumentChildTag, MonumentRoomTag, RegistryId, SavedPoolElement,
    SavedSingleElement,
};
use mcrs_minecraft_resource::ResourceLocationBuf;
use std::fmt;

use glam::IVec3;
use mcrs_minecraft_core::{BoundingBox, Direction, Mirror, Rotation};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_voxel::ColumnPos;
use serde::de::DeserializeSeed;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::frozen::{
    ElementId, FrozenElement, FrozenStructures, StructureId, StructureKind, TemplateId,
};
use crate::hardcoded::igloo::IglooTemplate;
use crate::orient::{Boxed, Orientation, union_of};
use crate::{MineshaftType, OceanTemperature, PortalPlacement, TerrainAdaptation};
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_worldgen_feature::pool::LiquidSettings;

pub const TERRAIN_MARGIN: i32 = 12;

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

/// The one piece of a jungle temple, at the fixed floor of 64 as laid out and
/// raised to its ground at placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JungleTemplePiece {
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    /// The mean ground under the box, which the reference averages over the
    /// live heightmap of the first decorating chunk and this layout fixes
    /// from the density heights.
    pub height_position: i32,
}

impl JungleTemplePiece {
    pub const WIDTH: i32 = 12;
    pub const HEIGHT: i32 = 10;
    pub const DEPTH: i32 = 15;
    pub const LAYOUT_FLOOR: i32 = 64;
}

/// The one piece of a swamp hut: its box as laid out, at the fixed floor of
/// 64, and the ground it is raised to at placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwampHutPiece {
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    /// The mean ground under the box, which the reference reads from the
    /// live heightmap and this layout fixes from the density heights.
    pub height_position: i32,
}

impl SwampHutPiece {
    pub const WIDTH: i32 = 7;
    pub const HEIGHT: i32 = 7;
    pub const DEPTH: i32 = 9;
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

/// The fortress piece types, each with the state its constructor draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FortressKind {
    BridgeCrossing,
    BridgeEndFiller { seed: i32 },
    BridgeStraight,
    CorridorStairs,
    CorridorBalcony,
    CastleEntrance,
    SmallCorridorCrossing,
    SmallCorridorLeftTurn { chest: bool },
    SmallCorridor,
    SmallCorridorRightTurn { chest: bool },
    StalkRoom,
    MonsterThrone,
    RoomCrossing,
    StairsRoom,
}

/// The mineshaft piece types, each with the state its constructor draws or
/// its children hand back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MineshaftKind {
    /// The start: the box each child corridor cuts through its wall, kept so
    /// the wall is opened whichever column places the room.
    Room {
        entrances: Vec<BoundingBox>,
    },
    Corridor {
        has_rails: bool,
        spider_corridor: bool,
        num_sections: i32,
        /// The one column that places a spider corridor's spawner, chosen at
        /// layout; not in the reference's tag, so a reloaded piece has none.
        spawner_host: Option<ColumnPos>,
    },
    Crossing {
        two_floored: bool,
    },
    Stairs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineshaftPiece {
    pub kind: MineshaftKind,
    pub bounds: BoundingBox,
    /// `None` for the room. A crossing keeps its direction without orienting
    /// its writes; only corridors and stairs draw through their facing.
    pub direction: Option<Orientation>,
    pub gen_depth: i32,
}

impl MineshaftPiece {
    pub fn orientation(&self) -> Option<Orientation> {
        match self.kind {
            MineshaftKind::Corridor { .. } | MineshaftKind::Stairs => self.direction,
            MineshaftKind::Room { .. } | MineshaftKind::Crossing { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortressPiece {
    pub kind: FortressKind,
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    pub gen_depth: i32,
}

/// The one template piece of a shipwreck: laid out at the fixed floor of 90,
/// with the height it is lowered to at placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipwreckPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub is_beached: bool,
    pub bounds: BoundingBox,
    /// The template position's `y` once lowered to the ground, which the
    /// reference reads from the live heightmaps of the first decorating chunk
    /// and this layout fixes from the density heights.
    pub height: i32,
}

impl ShipwreckPiece {
    pub const LAYOUT_FLOOR: i32 = 90;
    pub const PIVOT: IVec3 = IVec3::new(4, 0, 15);

    pub fn placed_position(&self) -> IVec3 {
        IVec3::new(self.position.x, self.height, self.position.z)
    }
}

/// One ruin of an ocean ruin start: a template at its laid-out position on
/// the reference's fixed layout floor, and the floor it is placed on.
#[derive(Debug, Clone, PartialEq)]
pub struct OceanRuinPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    pub integrity: f32,
    pub biome_temp: OceanTemperature,
    pub large: bool,
    /// The height the reference reads from the live world at placement, fixed
    /// here at layout from the density heights; the save carries it as the
    /// template's `TPY`, which the reference rewrites once placed.
    pub floor_y: i32,
}

impl OceanRuinPiece {
    pub const LAYOUT_FLOOR: i32 = 90;
}

/// The one piece of a ruined portal: a template placed mirrored and rotated
/// about the pivot at half its size.
#[derive(Debug, Clone, PartialEq)]
pub struct RuinedPortalPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub mirror: Mirror,
    pub bounds: BoundingBox,
    pub placement: PortalPlacement,
    pub properties: PortalProperties,
}

/// One cell of the monument's room graph: which of its six faces are open,
/// indexed by `Direction::id`, and which cell lies past each face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonumentRoom {
    pub index: i32,
    pub has_opening: [bool; 6],
    pub connections: [Option<u8>; 6],
}

impl MonumentRoom {
    pub const GRID_FLOOR: i32 = 25;
    pub const LEFT_WING: i32 = 1001;
    pub const RIGHT_WING: i32 = 1002;
    pub const ROOF: i32 = 1003;

    pub fn is_special(&self) -> bool {
        self.index >= 75
    }

    /// The room past `direction`, its faces indexed by `Direction::id`.
    pub fn connection(&self, direction: Direction) -> Option<usize> {
        face(&self.connections, direction).map(usize::from)
    }

    pub fn set_connection(&mut self, direction: Direction, room: u8) {
        *face_mut(&mut self.connections, direction) = Some(room);
    }

    pub fn opens(&self, direction: Direction) -> bool {
        face(&self.has_opening, direction)
    }

    pub fn set_opening(&mut self, direction: Direction, open: bool) {
        *face_mut(&mut self.has_opening, direction) = open;
    }
}

fn face<T: Copy>(faces: &[T; 6], direction: Direction) -> T {
    let [down, up, north, south, west, east] = *faces;
    match direction {
        Direction::Down => down,
        Direction::Up => up,
        Direction::North => north,
        Direction::South => south,
        Direction::West => west,
        Direction::East => east,
    }
}

fn face_mut<T>(faces: &mut [T; 6], direction: Direction) -> &mut T {
    let [down, up, north, south, west, east] = faces;
    match direction {
        Direction::Down => down,
        Direction::Up => up,
        Direction::North => north,
        Direction::South => south,
        Direction::West => west,
        Direction::East => east,
    }
}

/// The rooms fitted into the graph, each naming the cell it grows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonumentRoomKind {
    Entry { room: u8 },
    Core { room: u8 },
    DoubleX { room: u8 },
    DoubleXY { room: u8 },
    DoubleY { room: u8 },
    DoubleYZ { room: u8 },
    DoubleZ { room: u8 },
    Simple { room: u8, main_design: i32 },
    SimpleTop { room: u8 },
    Wing { main_design: i32 },
    Penthouse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonumentChild {
    pub kind: MonumentRoomKind,
    pub bounds: BoundingBox,
}

/// The one piece of an ocean monument the save carries: the building, whose
/// rooms the reference keeps in memory and rebuilds from the seed on load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OceanMonumentPiece {
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    pub rooms: Vec<MonumentRoom>,
    pub children: Vec<MonumentChild>,
}

impl OceanMonumentPiece {
    pub const WIDTH: i32 = 58;
    pub const HEIGHT: i32 = 23;
    pub const DEPTH: i32 = 58;
    pub const FLOOR: i32 = 39;
}

/// One template of an igloo at the layout height of 90, with the height the
/// whole igloo is lowered to at placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IglooPiece {
    pub template: IglooTemplate,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    /// The template position's `y` once lowered to the ground under the
    /// entrance, which the reference reads from the live heightmaps in every
    /// decorating chunk and this layout fixes from the density heights.
    pub height: i32,
}

impl IglooPiece {
    pub fn placed_position(&self) -> IVec3 {
        IVec3::new(self.position.x, self.height, self.position.z)
    }
}

/// One fossil template at its position, unmirrored and unpivoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetherFossilPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
}

/// One template of an end city at its position and rotation; `overwrite`
/// says whether the template's air is written over what stands there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndCityPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub overwrite: bool,
    pub bounds: BoundingBox,
    /// The collision tag the layout stamps on each recursion's children.
    pub gen_depth: i32,
}

pub const END_CITY_TEMPLATE_PREFIX: &str = "end_city/";

/// The stronghold piece types, each with the state its constructor draws or
/// derives from its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrongholdKind {
    /// The stairs the whole stronghold grows from.
    Start,
    StairsDown,
    Straight {
        left: bool,
        right: bool,
    },
    PrisonHall,
    LeftTurn,
    RightTurn,
    RoomCrossing {
        variant: i32,
    },
    StraightStairsDown,
    FiveCrossing {
        left_low: bool,
        left_high: bool,
        right_low: bool,
        right_high: bool,
    },
    ChestCorridor,
    Library {
        tall: bool,
    },
    PortalRoom,
    FillerCorridor {
        steps: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongholdPiece {
    pub kind: StrongholdKind,
    pub entry_door: SmallDoor,
    pub bounds: BoundingBox,
    pub orientation: Orientation,
    pub gen_depth: i32,
}

/// One template of a woodland mansion at its position under a rotation and a
/// mirror, with no pivot; `StructureBlock` markers stay where the template
/// has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WoodlandMansionPiece {
    pub template: TemplateId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub mirror: Mirror,
    pub bounds: BoundingBox,
}

impl WoodlandMansionPiece {
    pub const TEMPLATE_PREFIX: &'static str = "woodland_mansion/";
}

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Jigsaw(JigsawPiece),
    DesertPyramid(DesertPyramidPiece),
    JungleTemple(JungleTemplePiece),
    SwampHut(SwampHutPiece),
    BuriedTreasure(BuriedTreasurePiece),
    Fortress(FortressPiece),
    Shipwreck(ShipwreckPiece),
    OceanRuin(OceanRuinPiece),
    RuinedPortal(RuinedPortalPiece),
    OceanMonument(OceanMonumentPiece),
    Mineshaft(MineshaftPiece),
    Igloo(IglooPiece),
    NetherFossil(NetherFossilPiece),
    EndCity(EndCityPiece),
    Stronghold(StrongholdPiece),
    WoodlandMansion(WoodlandMansionPiece),
}

impl Piece {
    pub fn bounds(&self) -> BoundingBox {
        match self {
            Piece::Jigsaw(piece) => piece.bounds,
            Piece::DesertPyramid(piece) => piece.bounds,
            Piece::JungleTemple(piece) => piece.bounds,
            Piece::SwampHut(piece) => piece.bounds,
            Piece::BuriedTreasure(piece) => piece.bounds,
            Piece::Fortress(piece) => piece.bounds,
            Piece::Shipwreck(piece) => piece.bounds,
            Piece::OceanRuin(piece) => piece.bounds,
            Piece::RuinedPortal(piece) => piece.bounds,
            Piece::OceanMonument(piece) => piece.bounds,
            Piece::Mineshaft(piece) => piece.bounds,
            Piece::Igloo(piece) => piece.bounds,
            Piece::NetherFossil(piece) => piece.bounds,
            Piece::Stronghold(piece) => piece.bounds,
            Piece::EndCity(piece) => piece.bounds,
            Piece::WoodlandMansion(piece) => piece.bounds,
        }
    }

    /// `StructurePiece.move`, which for a template piece carries its position too.
    pub fn move_by(&mut self, delta: IVec3) {
        match self {
            Piece::Jigsaw(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
            Piece::DesertPyramid(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::JungleTemple(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::SwampHut(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::BuriedTreasure(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::Fortress(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::Shipwreck(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
                piece.height += delta.y;
            }
            Piece::OceanRuin(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
                piece.floor_y += delta.y;
            }
            Piece::RuinedPortal(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
            Piece::OceanMonument(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                for child in &mut piece.children {
                    child.bounds = child.bounds.moved(delta);
                }
            }
            Piece::Mineshaft(piece) => Boxed::move_by(piece, delta),
            Piece::Igloo(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
                piece.height += delta.y;
            }
            Piece::NetherFossil(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
            Piece::Stronghold(piece) => piece.bounds = piece.bounds.moved(delta),
            Piece::EndCity(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
            Piece::WoodlandMansion(piece) => {
                piece.bounds = piece.bounds.moved(delta);
                piece.position += delta;
            }
        }
    }

    pub fn nbt<'a>(&'a self, context: &'a PieceContext<'a>) -> PieceNbt<'a> {
        PieceNbt {
            piece: self,
            context,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    pub structure: StructureId,
    pub pieces: Vec<Piece>,
    pub bounds: BoundingBox,
}

impl Start {
    /// `None` for no pieces or a structure `frozen` does not hold.
    pub fn new(
        frozen: &FrozenStructures,
        structure: StructureId,
        pieces: Vec<Piece>,
    ) -> Option<Self> {
        let union = union_of(&pieces)?;
        let bounds = if frozen.structure(structure)?.adaptation == TerrainAdaptation::None {
            union
        } else {
            union.inflated(TERRAIN_MARGIN)
        };
        Some(Start {
            structure,
            pieces,
            bounds,
        })
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
    fn kind(&self) -> Option<&StructureKind> {
        self.frozen
            .structure(self.structure)
            .map(|structure| &structure.kind)
    }

    fn liquid_settings(&self) -> LiquidSettings {
        match self.kind() {
            Some(StructureKind::Jigsaw { config, .. }) => config.liquid_settings,
            _ => LiquidSettings::default(),
        }
    }

    /// `MST`: the structure's `mineshaft_type` ordinal, which every piece
    /// writes and none holds.
    fn mineshaft_type(&self) -> i32 {
        match self.kind() {
            Some(StructureKind::Mineshaft {
                mineshaft_type: MineshaftType::Mesa,
                ..
            }) => 1,
            _ => 0,
        }
    }

    fn mineshaft<E: serde::de::Error>(
        &self,
        mineshaft_type: i32,
        piece: MineshaftPiece,
    ) -> Result<Piece, E> {
        if mineshaft_type != self.mineshaft_type() {
            return Err(E::custom("a mineshaft piece of another mineshaft type"));
        }
        Ok(Piece::Mineshaft(piece))
    }

    /// `None` for a template the context's tables do not name.
    pub fn template_name(&self, template: TemplateId) -> Option<ResourceLocationBuf> {
        self.frozen
            .template_ids
            .iter()
            .find(|(_, id)| **id == template)
            .map(|(location, _)| location.clone())
    }

    fn template_name_or<E: serde::ser::Error>(
        &self,
        template: TemplateId,
    ) -> Result<ResourceLocationBuf, E> {
        self.template_name(template)
            .ok_or_else(|| E::custom("a piece names a template no loaded pool holds"))
    }

    /// The template's path under `prefix`, the name a save of a piece that
    /// only draws from one directory carries.
    fn template_suffix<E: serde::ser::Error>(
        &self,
        template: TemplateId,
        prefix: &str,
    ) -> Result<String, E> {
        let name = self.template_name_or::<E>(template)?;
        name.path()
            .strip_prefix(prefix)
            .map(str::to_owned)
            .ok_or_else(|| E::custom(format!("the piece's template {name} is not under {prefix}")))
    }

    fn pool_element(&self, element: ElementId) -> Option<SavedPoolElement> {
        Some(match self.frozen.element(element)? {
            FrozenElement::Single {
                template,
                legacy,
                processors,
                projection,
                liquid_settings,
            } => {
                let Holder::Reference(processors) = processors else {
                    return None;
                };
                let single = SavedSingleElement {
                    location: self.template_name(*template)?,
                    processors: RegistryId(processors.clone()),
                    projection: *projection,
                    override_liquid_settings: *liquid_settings,
                };
                if *legacy {
                    SavedPoolElement::LegacySingle(single)
                } else {
                    SavedPoolElement::Single(single)
                }
            }
            FrozenElement::List {
                elements,
                projection,
            } => SavedPoolElement::List {
                elements: elements
                    .iter()
                    .map(|id| self.pool_element(*id))
                    .collect::<Option<_>>()?,
                projection: *projection,
            },
            FrozenElement::Feature {
                feature,
                projection,
            } => {
                let Holder::Reference(feature) = feature else {
                    return None;
                };
                SavedPoolElement::Feature {
                    feature: RegistryId(feature.clone()),
                    projection: *projection,
                }
            }
            FrozenElement::Empty => SavedPoolElement::Empty {},
        })
    }

    fn element_id<E: serde::de::Error>(
        &self,
        wanted: &SavedPoolElement,
    ) -> Result<(ElementId, &FrozenElement), E> {
        self.frozen
            .elements
            .iter()
            .enumerate()
            .find(|(_, element)| self.element_matches(element, wanted))
            .map(|(index, element)| (ElementId(index as u32), element))
            .ok_or_else(|| E::custom("the pool element is not one any loaded pool holds"))
    }

    fn element_matches(&self, element: &FrozenElement, wanted: &SavedPoolElement) -> bool {
        match (element, wanted) {
            (
                FrozenElement::Single {
                    template,
                    legacy,
                    processors,
                    projection,
                    liquid_settings,
                },
                SavedPoolElement::Single(single) | SavedPoolElement::LegacySingle(single),
            ) => {
                self.frozen.template_ids.get(&single.location) == Some(template)
                    && *legacy == matches!(wanted, SavedPoolElement::LegacySingle(_))
                    && matches!(processors, Holder::Reference(id) if *id == single.processors.0)
                    && *projection == single.projection
                    && *liquid_settings == single.override_liquid_settings
            }
            (
                FrozenElement::List {
                    elements,
                    projection,
                },
                SavedPoolElement::List {
                    elements: wanted,
                    projection: wanted_projection,
                },
            ) => {
                *projection == *wanted_projection
                    && elements.len() == wanted.len()
                    && elements.iter().zip(wanted).all(|(id, wanted)| {
                        self.frozen
                            .element(*id)
                            .is_some_and(|element| self.element_matches(element, wanted))
                    })
            }
            (
                FrozenElement::Feature {
                    feature,
                    projection,
                },
                SavedPoolElement::Feature {
                    feature: wanted,
                    projection: wanted_projection,
                },
            ) => {
                matches!(feature, Holder::Reference(id) if *id == wanted.0)
                    && *projection == *wanted_projection
            }
            (FrozenElement::Empty, SavedPoolElement::Empty {}) => true,
            _ => false,
        }
    }
}

/// `Direction.NORTH.get2DDataValue()`: every template piece faces north.
const NORTH_ORIENTATION: i32 = 2;

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

trait DoorTagExt: Sized {
    fn of(piece: &StrongholdPiece) -> Self;
    fn piece<E: serde::de::Error>(self, kind: StrongholdKind) -> Result<Piece, E>;
}

trait MonumentBuildingTagExt: Sized {
    fn of(piece: &OceanMonumentPiece) -> Self;
    fn piece<E: serde::de::Error>(self) -> Result<OceanMonumentPiece, E>;
}

trait MonumentRoomTagExt: Sized {
    fn of(room: &MonumentRoom) -> Self;
    fn room<E: serde::de::Error>(self) -> Result<MonumentRoom, E>;
}

trait MonumentChildTagExt: Sized {
    fn of(child: &MonumentChild) -> Self;
    fn child<E: serde::de::Error>(self) -> Result<MonumentChild, E>;
}

trait GridTagExt: Sized {
    fn of(piece: &FortressPiece) -> Self;
    fn piece<E: serde::de::Error>(self, kind: FortressKind) -> Result<Piece, E>;
    fn oriented<E: serde::de::Error>(&self) -> Result<Orientation, E>;
}

fn room_byte<E: serde::de::Error>(value: i32) -> Result<u8, E> {
    u8::try_from(value).map_err(|_| E::custom("a monument room is not a byte"))
}

impl DoorTagExt for DoorTag {
    fn of(piece: &StrongholdPiece) -> Self {
        DoorTag {
            header: GridTag {
                bounds: box_array(piece.bounds),
                orientation: piece.orientation.data_2d(),
                gen_depth: piece.gen_depth,
            },
            entry_door: piece.entry_door,
        }
    }

    fn piece<E: serde::de::Error>(self, kind: StrongholdKind) -> Result<Piece, E> {
        Ok(Piece::Stronghold(StrongholdPiece {
            kind,
            entry_door: self.entry_door,
            bounds: box_of(self.header.bounds),
            orientation: Orientation::from_data_2d(self.header.orientation)
                .ok_or_else(|| E::custom("a stronghold piece without an orientation"))?,
            gen_depth: self.header.gen_depth,
        }))
    }
}

/// Gives a saved monument building the room graph that generation of the start at `chunk` makes
/// from `seed`. A building whose bounds or orientation differ from that generation is not a
/// start of this seed, and stays as saved.
pub fn rebuild_monument_rooms(
    seed: i64,
    chunk: ColumnPos,
    saved: &mut MonumentBuildingTag,
) -> bool {
    let Some(built) = crate::hardcoded::ocean_monument::rebuild(seed, chunk) else {
        return false;
    };
    let built = MonumentBuildingTag::of(&built);
    if built.header.bounds != saved.header.bounds
        || built.header.orientation != saved.header.orientation
    {
        return false;
    }
    *saved = built;
    true
}

impl MonumentBuildingTagExt for MonumentBuildingTag {
    fn of(piece: &OceanMonumentPiece) -> Self {
        MonumentBuildingTag {
            header: GridTag {
                bounds: box_array(piece.bounds),
                orientation: piece.orientation.data_2d(),
                gen_depth: 0,
            },
            rooms: piece.rooms.iter().map(MonumentRoomTag::of).collect(),
            children: piece.children.iter().map(MonumentChildTag::of).collect(),
        }
    }

    fn piece<E: serde::de::Error>(self) -> Result<OceanMonumentPiece, E> {
        Ok(OceanMonumentPiece {
            bounds: box_of(self.header.bounds),
            orientation: Orientation::from_data_2d(self.header.orientation)
                .ok_or_else(|| E::custom("a monument without an orientation"))?,
            rooms: self
                .rooms
                .into_iter()
                .map(MonumentRoomTag::room)
                .collect::<Result<_, _>>()?,
            children: self
                .children
                .into_iter()
                .map(MonumentChildTag::child)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl MonumentRoomTagExt for MonumentRoomTag {
    fn of(room: &MonumentRoom) -> Self {
        let mut openings = [0; 6];
        for (saved, open) in openings.iter_mut().zip(room.has_opening) {
            *saved = i8::from(open);
        }
        MonumentRoomTag {
            index: room.index,
            openings,
            connections: room.connections.map(|room| room.map_or(-1, i32::from)),
        }
    }

    fn room<E: serde::de::Error>(self) -> Result<MonumentRoom, E> {
        let mut openings = [false; 6];
        for (open, saved) in openings.iter_mut().zip(self.openings) {
            *open = match saved {
                0 => false,
                1 => true,
                _ => return Err(E::custom("a monument room opening is not a flag")),
            };
        }
        let mut connections = [None; 6];
        for (connection, saved) in connections.iter_mut().zip(self.connections) {
            *connection = match saved {
                -1 => None,
                0..=255 => Some(room_byte(saved)?),
                _ => return Err(E::custom("a monument room connection is not a room")),
            };
        }
        Ok(MonumentRoom {
            index: self.index,
            has_opening: openings,
            connections,
        })
    }
}

impl MonumentChildTagExt for MonumentChildTag {
    fn of(child: &MonumentChild) -> Self {
        let bounds = box_array(child.bounds);
        match child.kind {
            MonumentRoomKind::Entry { room } => Self::Entry {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::Core { room } => Self::Core {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::DoubleX { room } => Self::DoubleX {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::DoubleXY { room } => Self::DoubleXY {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::DoubleY { room } => Self::DoubleY {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::DoubleYZ { room } => Self::DoubleYZ {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::DoubleZ { room } => Self::DoubleZ {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::Simple { room, main_design } => Self::Simple {
                bounds,
                room: i32::from(room),
                design: main_design,
            },
            MonumentRoomKind::SimpleTop { room } => Self::SimpleTop {
                bounds,
                room: i32::from(room),
            },
            MonumentRoomKind::Wing { main_design } => Self::Wing {
                bounds,
                design: main_design,
            },
            MonumentRoomKind::Penthouse => Self::Penthouse { bounds },
        }
    }

    fn child<E: serde::de::Error>(self) -> Result<MonumentChild, E> {
        let (bounds, kind) = match self {
            Self::Entry { bounds, room } => (
                bounds,
                MonumentRoomKind::Entry {
                    room: room_byte(room)?,
                },
            ),
            Self::Core { bounds, room } => (
                bounds,
                MonumentRoomKind::Core {
                    room: room_byte(room)?,
                },
            ),
            Self::DoubleX { bounds, room } => (
                bounds,
                MonumentRoomKind::DoubleX {
                    room: room_byte(room)?,
                },
            ),
            Self::DoubleXY { bounds, room } => (
                bounds,
                MonumentRoomKind::DoubleXY {
                    room: room_byte(room)?,
                },
            ),
            Self::DoubleY { bounds, room } => (
                bounds,
                MonumentRoomKind::DoubleY {
                    room: room_byte(room)?,
                },
            ),
            Self::DoubleYZ { bounds, room } => (
                bounds,
                MonumentRoomKind::DoubleYZ {
                    room: room_byte(room)?,
                },
            ),
            Self::DoubleZ { bounds, room } => (
                bounds,
                MonumentRoomKind::DoubleZ {
                    room: room_byte(room)?,
                },
            ),
            Self::Simple {
                bounds,
                room,
                design,
            } => (
                bounds,
                MonumentRoomKind::Simple {
                    room: room_byte(room)?,
                    main_design: design,
                },
            ),
            Self::SimpleTop { bounds, room } => (
                bounds,
                MonumentRoomKind::SimpleTop {
                    room: room_byte(room)?,
                },
            ),
            Self::Wing { bounds, design } => (
                bounds,
                MonumentRoomKind::Wing {
                    main_design: design,
                },
            ),
            Self::Penthouse { bounds } => (bounds, MonumentRoomKind::Penthouse),
        };
        Ok(MonumentChild {
            kind,
            bounds: box_of(bounds),
        })
    }
}

impl GridTagExt for GridTag {
    fn of(piece: &FortressPiece) -> Self {
        GridTag {
            bounds: box_array(piece.bounds),
            orientation: piece.orientation.data_2d(),
            gen_depth: piece.gen_depth,
        }
    }

    fn piece<E: serde::de::Error>(self, kind: FortressKind) -> Result<Piece, E> {
        Ok(Piece::Fortress(FortressPiece {
            kind,
            bounds: box_of(self.bounds),
            orientation: self.oriented()?,
            gen_depth: self.gen_depth,
        }))
    }

    fn oriented<E: serde::de::Error>(&self) -> Result<Orientation, E> {
        Orientation::from_data_2d(self.orientation)
            .ok_or_else(|| E::custom("a grid piece without an orientation"))
    }
}

pub struct PieceNbt<'a> {
    piece: &'a Piece,
    context: &'a PieceContext<'a>,
}

impl Serialize for PieceNbt<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.context
            .saved::<S::Error>(self.piece)?
            .serialize(serializer)
    }
}

impl PieceContext<'_> {
    /// The saved form of `piece`, with every key its kind writes.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per piece kind, each writing that kind's tag, in the reference's kind order"
    )]
    pub fn saved<E: serde::ser::Error>(&self, piece: &Piece) -> Result<SavedPiece, E> {
        let tag = match piece {
            Piece::Jigsaw(piece) => {
                let liquid = self.liquid_settings();
                SavedPiece::Jigsaw {
                    header: GridTag {
                        bounds: box_array(piece.bounds),
                        orientation: NO_ORIENTATION,
                        gen_depth: 0,
                    },
                    pos_x: piece.position.x,
                    pos_y: piece.position.y,
                    pos_z: piece.position.z,
                    ground_level_delta: piece.ground_level_delta,
                    pool_element: self.pool_element(piece.element).ok_or_else(|| {
                        E::custom("a jigsaw piece of an element no loaded pool holds")
                    })?,
                    rotation: piece.rotation,
                    junctions: piece.junctions.clone(),
                    liquid_settings: (liquid != LiquidSettings::default()).then_some(liquid),
                }
            }
            Piece::DesertPyramid(piece) => SavedPiece::DesertPyramid {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: piece.orientation.data_2d(),
                    gen_depth: 0,
                },
                width: DesertPyramidPiece::WIDTH,
                height: DesertPyramidPiece::HEIGHT,
                depth: DesertPyramidPiece::DEPTH,
                height_position: piece.height_position,
                has_placed_chest_0: false,
                has_placed_chest_1: false,
                has_placed_chest_2: false,
                has_placed_chest_3: false,
            },
            Piece::JungleTemple(piece) => SavedPiece::JungleTemple {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: piece.orientation.data_2d(),
                    gen_depth: 0,
                },
                width: JungleTemplePiece::WIDTH,
                height: JungleTemplePiece::HEIGHT,
                depth: JungleTemplePiece::DEPTH,
                height_position: piece.height_position,
                placed_main_chest: false,
                placed_hidden_chest: false,
                placed_trap_1: false,
                placed_trap_2: false,
            },
            Piece::SwampHut(piece) => SavedPiece::SwampHut {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: piece.orientation.data_2d(),
                    gen_depth: 0,
                },
                width: SwampHutPiece::WIDTH,
                height: SwampHutPiece::HEIGHT,
                depth: SwampHutPiece::DEPTH,
                height_position: piece.height_position,
                witch: false,
                cat: false,
            },
            Piece::BuriedTreasure(piece) => SavedPiece::BuriedTreasure {
                bounds: box_array(piece.bounds),
                orientation: NO_ORIENTATION,
                gen_depth: 0,
            },
            Piece::Fortress(piece) => {
                let header = GridTag::of(piece);
                match piece.kind {
                    FortressKind::BridgeCrossing => SavedPiece::FortressBridgeCrossing(header),
                    FortressKind::BridgeEndFiller { seed } => {
                        SavedPiece::FortressBridgeEndFiller { header, seed }
                    }
                    FortressKind::BridgeStraight => SavedPiece::FortressBridgeStraight(header),
                    FortressKind::CorridorStairs => SavedPiece::FortressCorridorStairs(header),
                    FortressKind::CorridorBalcony => SavedPiece::FortressCorridorBalcony(header),
                    FortressKind::CastleEntrance => SavedPiece::FortressCastleEntrance(header),
                    FortressKind::SmallCorridorCrossing => {
                        SavedPiece::FortressSmallCorridorCrossing(header)
                    }
                    FortressKind::SmallCorridorLeftTurn { chest } => {
                        SavedPiece::FortressSmallCorridorLeftTurn { header, chest }
                    }
                    FortressKind::SmallCorridor => SavedPiece::FortressSmallCorridor(header),
                    FortressKind::SmallCorridorRightTurn { chest } => {
                        SavedPiece::FortressSmallCorridorRightTurn { header, chest }
                    }
                    FortressKind::StalkRoom => SavedPiece::FortressStalkRoom(header),
                    FortressKind::MonsterThrone => {
                        SavedPiece::FortressMonsterThrone { header, mob: false }
                    }
                    FortressKind::RoomCrossing => SavedPiece::FortressRoomCrossing(header),
                    FortressKind::StairsRoom => SavedPiece::FortressStairsRoom(header),
                }
            }
            Piece::Shipwreck(piece) => SavedPiece::Shipwreck {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: NORTH_ORIENTATION,
                    gen_depth: 0,
                },
                template_x: piece.position.x,
                template_y: piece.height,
                template_z: piece.position.z,
                template: self.template_name_or::<E>(piece.template)?.to_string(),
                is_beached: piece.is_beached,
                rotation: piece.rotation,
                height_adjusted: true,
            },
            Piece::OceanRuin(piece) => SavedPiece::OceanRuin {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: Orientation::North.data_2d(),
                    gen_depth: 0,
                },
                template_x: piece.position.x,
                template_y: piece.floor_y,
                template_z: piece.position.z,
                template: self.template_name_or::<E>(piece.template)?.to_string(),
                rotation: piece.rotation,
                integrity: piece.integrity,
                biome_temp: piece.biome_temp,
                large: piece.large,
            },
            Piece::RuinedPortal(piece) => SavedPiece::RuinedPortal {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: Orientation::North.data_2d(),
                    gen_depth: 0,
                },
                pos_x: piece.position.x,
                pos_y: piece.position.y,
                pos_z: piece.position.z,
                template: self.template_name_or::<E>(piece.template)?.to_string(),
                rotation: piece.rotation,
                mirror: piece.mirror,
                placement: piece.placement,
                properties: piece.properties,
            },
            Piece::OceanMonument(piece) => {
                SavedPiece::OceanMonumentBuilding(MonumentBuildingTag::of(piece))
            }
            Piece::Mineshaft(piece) => {
                let header = GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: piece
                        .orientation()
                        .map_or(NO_ORIENTATION, Orientation::data_2d),
                    gen_depth: piece.gen_depth,
                };
                let mineshaft_type = self.mineshaft_type();
                match &piece.kind {
                    MineshaftKind::Room { entrances } => SavedPiece::MineshaftRoom {
                        header,
                        mineshaft_type,
                        entrances: entrances.iter().map(|b| box_array(*b)).collect(),
                    },
                    MineshaftKind::Corridor {
                        has_rails,
                        spider_corridor,
                        num_sections,
                        spawner_host: _,
                    } => SavedPiece::MineshaftCorridor {
                        header,
                        mineshaft_type,
                        has_rails: *has_rails,
                        spider_corridor: *spider_corridor,
                        has_placed_spider: false,
                        num_sections: *num_sections,
                    },
                    MineshaftKind::Crossing { two_floored } => SavedPiece::MineshaftCrossing {
                        header,
                        mineshaft_type,
                        two_floored: *two_floored,
                        direction: piece
                            .direction
                            .ok_or_else(|| E::custom("a mineshaft crossing without a direction"))?
                            .data_2d()
                            .try_into()
                            .unwrap_or_default(),
                    },
                    MineshaftKind::Stairs => SavedPiece::MineshaftStairs {
                        header,
                        mineshaft_type,
                    },
                }
            }
            Piece::Igloo(piece) => SavedPiece::Igloo {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: NORTH_ORIENTATION,
                    gen_depth: 0,
                },
                template_x: piece.position.x,
                template_y: piece.height,
                template_z: piece.position.z,
                template: piece.template.location().to_string(),
                rotation: piece.rotation,
            },
            Piece::NetherFossil(piece) => SavedPiece::NetherFossil {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: NORTH_ORIENTATION,
                    gen_depth: 0,
                },
                template_x: piece.position.x,
                template_y: piece.position.y,
                template_z: piece.position.z,
                template: self.template_name_or::<E>(piece.template)?.to_string(),
                rotation: piece.rotation,
            },
            Piece::EndCity(piece) => SavedPiece::EndCity {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: NORTH_ORIENTATION,
                    gen_depth: piece.gen_depth,
                },
                template_x: piece.position.x,
                template_y: piece.position.y,
                template_z: piece.position.z,
                template: self.template_suffix::<E>(piece.template, END_CITY_TEMPLATE_PREFIX)?,
                rotation: piece.rotation,
                overwrite: piece.overwrite,
            },
            Piece::WoodlandMansion(piece) => SavedPiece::WoodlandMansion {
                header: GridTag {
                    bounds: box_array(piece.bounds),
                    orientation: NORTH_ORIENTATION,
                    gen_depth: 0,
                },
                template_x: piece.position.x,
                template_y: piece.position.y,
                template_z: piece.position.z,
                template: self
                    .template_suffix::<E>(piece.template, WoodlandMansionPiece::TEMPLATE_PREFIX)?,
                rotation: piece.rotation,
                mirror: piece.mirror,
            },
            Piece::Stronghold(piece) => {
                let door = DoorTag::of(piece);
                let DoorTag { header, entry_door } = door;
                let stairs = |source| StairsDownTag {
                    header,
                    entry_door,
                    source,
                };
                match piece.kind {
                    StrongholdKind::Start => SavedPiece::StrongholdStart(stairs(true)),
                    StrongholdKind::StairsDown => SavedPiece::StrongholdStairsDown(stairs(false)),
                    StrongholdKind::Straight { left, right } => SavedPiece::StrongholdStraight {
                        header,
                        entry_door,
                        left,
                        right,
                    },
                    StrongholdKind::PrisonHall => SavedPiece::StrongholdPrisonHall(door),
                    StrongholdKind::LeftTurn => SavedPiece::StrongholdLeftTurn(door),
                    StrongholdKind::RightTurn => SavedPiece::StrongholdRightTurn(door),
                    StrongholdKind::RoomCrossing { variant } => {
                        SavedPiece::StrongholdRoomCrossing {
                            header,
                            entry_door,
                            variant,
                        }
                    }
                    StrongholdKind::StraightStairsDown => {
                        SavedPiece::StrongholdStraightStairsDown(door)
                    }
                    StrongholdKind::FiveCrossing {
                        left_low,
                        left_high,
                        right_low,
                        right_high,
                    } => SavedPiece::StrongholdFiveCrossing {
                        header,
                        entry_door,
                        left_low,
                        left_high,
                        right_low,
                        right_high,
                    },
                    StrongholdKind::ChestCorridor => SavedPiece::StrongholdChestCorridor {
                        header,
                        entry_door,
                        chest: false,
                    },
                    StrongholdKind::Library { tall } => SavedPiece::StrongholdLibrary {
                        header,
                        entry_door,
                        tall,
                    },
                    StrongholdKind::PortalRoom => SavedPiece::StrongholdPortalRoom {
                        header,
                        entry_door,
                        mob: false,
                    },
                    StrongholdKind::FillerCorridor { steps } => {
                        SavedPiece::StrongholdFillerCorridor {
                            header,
                            entry_door,
                            steps,
                        }
                    }
                }
            }
        };
        Ok(tag)
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
        self.0
            .piece(<SavedPiece as Deserialize>::deserialize(deserializer)?)
    }
}

fn location<E: serde::de::Error>(text: &str) -> Result<ResourceLocationBuf, E> {
    ResourceLocationBuf::read(text)
        .map_err(|_| E::custom(format!("`{text}` is not a resource location")))
}

impl PieceContext<'_> {
    /// The piece that `saved` describes, resolving the names it carries against the tables.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per piece tag, each rebuilding that kind's piece, in the reference's kind order"
    )]
    pub fn piece<E: serde::de::Error>(&self, saved: SavedPiece) -> Result<Piece, E> {
        match saved {
            SavedPiece::Jigsaw {
                header: GridTag { bounds, .. },
                pos_x,
                pos_y,
                pos_z,
                ground_level_delta,
                pool_element,
                rotation,
                junctions,
                ..
            } => {
                let (element, frozen_element) = self.element_id(&pool_element)?;
                let projection = frozen_element
                    .projection()
                    .ok_or_else(|| E::custom("a jigsaw piece of an empty element"))?;
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
            SavedPiece::DesertPyramid {
                header:
                    GridTag {
                        bounds,
                        orientation,
                        ..
                    },
                height_position,
                ..
            } => Ok(Piece::DesertPyramid(DesertPyramidPiece {
                bounds: box_of(bounds),
                orientation: Orientation::from_data_2d(orientation)
                    .ok_or_else(|| E::custom("a desert pyramid without an orientation"))?,
                height_position,
            })),
            SavedPiece::JungleTemple {
                header:
                    GridTag {
                        bounds,
                        orientation,
                        ..
                    },
                height_position,
                ..
            } => Ok(Piece::JungleTemple(JungleTemplePiece {
                bounds: box_of(bounds),
                orientation: Orientation::from_data_2d(orientation)
                    .ok_or_else(|| E::custom("a jungle temple without an orientation"))?,
                height_position,
            })),
            SavedPiece::SwampHut {
                header:
                    GridTag {
                        bounds,
                        orientation,
                        ..
                    },
                height_position,
                ..
            } => Ok(Piece::SwampHut(SwampHutPiece {
                bounds: box_of(bounds),
                orientation: Orientation::from_data_2d(orientation)
                    .ok_or_else(|| E::custom("a swamp hut without an orientation"))?,
                height_position,
            })),
            SavedPiece::BuriedTreasure { bounds, .. } => {
                Ok(Piece::BuriedTreasure(BuriedTreasurePiece {
                    bounds: box_of(bounds),
                }))
            }
            SavedPiece::FortressBridgeCrossing(grid) => grid.piece(FortressKind::BridgeCrossing),
            SavedPiece::FortressBridgeEndFiller { header, seed } => {
                header.piece(FortressKind::BridgeEndFiller { seed })
            }
            SavedPiece::FortressBridgeStraight(grid) => grid.piece(FortressKind::BridgeStraight),
            SavedPiece::FortressCorridorStairs(grid) => grid.piece(FortressKind::CorridorStairs),
            SavedPiece::FortressCorridorBalcony(grid) => grid.piece(FortressKind::CorridorBalcony),
            SavedPiece::FortressCastleEntrance(grid) => grid.piece(FortressKind::CastleEntrance),
            SavedPiece::FortressSmallCorridorCrossing(grid) => {
                grid.piece(FortressKind::SmallCorridorCrossing)
            }
            SavedPiece::FortressSmallCorridorLeftTurn { header, chest } => {
                header.piece(FortressKind::SmallCorridorLeftTurn { chest })
            }
            SavedPiece::FortressSmallCorridor(grid) => grid.piece(FortressKind::SmallCorridor),
            SavedPiece::FortressSmallCorridorRightTurn { header, chest } => {
                header.piece(FortressKind::SmallCorridorRightTurn { chest })
            }
            SavedPiece::FortressStalkRoom(grid) => grid.piece(FortressKind::StalkRoom),
            SavedPiece::FortressMonsterThrone { header, .. } => {
                header.piece(FortressKind::MonsterThrone)
            }
            SavedPiece::FortressRoomCrossing(grid) => grid.piece(FortressKind::RoomCrossing),
            SavedPiece::FortressStairsRoom(grid) => grid.piece(FortressKind::StairsRoom),
            SavedPiece::Shipwreck {
                header: GridTag { bounds, .. },
                template_x,
                template_y,
                template_z,
                template,
                is_beached,
                rotation,
                ..
            } => {
                let bounds = box_of(bounds);
                let template = location::<E>(&template)?;
                let template =
                    *self.frozen.template_ids.get(&template).ok_or_else(|| {
                        E::custom(format!("the template {template} is not loaded"))
                    })?;
                Ok(Piece::Shipwreck(ShipwreckPiece {
                    template,
                    position: IVec3::new(template_x, bounds.min.y, template_z),
                    rotation,
                    is_beached,
                    bounds,
                    height: template_y,
                }))
            }
            SavedPiece::OceanRuin {
                header: GridTag { bounds, .. },
                template_x,
                template_y,
                template_z,
                template,
                rotation,
                integrity,
                biome_temp,
                large,
                ..
            } => {
                let template = location::<E>(&template)?;
                let template =
                    *self.frozen.template_ids.get(&template).ok_or_else(|| {
                        E::custom(format!("the template {template} is not loaded"))
                    })?;
                let bounds = box_of(bounds);
                Ok(Piece::OceanRuin(OceanRuinPiece {
                    template,
                    position: IVec3::new(template_x, bounds.min.y, template_z),
                    rotation,
                    bounds,
                    integrity,
                    biome_temp,
                    large,
                    floor_y: template_y,
                }))
            }
            SavedPiece::RuinedPortal {
                header: GridTag { bounds, .. },
                pos_x,
                pos_y,
                pos_z,
                template,
                rotation,
                mirror,
                placement,
                properties,
                ..
            } => {
                let template = location::<E>(&template)?;
                let template = *self.frozen.template_ids.get(&template).ok_or_else(|| {
                    E::custom(format!(
                        "the template {template} is not one any loaded structure draws"
                    ))
                })?;
                Ok(Piece::RuinedPortal(RuinedPortalPiece {
                    template,
                    position: IVec3::new(pos_x, pos_y, pos_z),
                    rotation,
                    mirror,
                    bounds: box_of(bounds),
                    placement,
                    properties,
                }))
            }
            SavedPiece::FortressStart(_)
            | SavedPiece::OceanMonumentEntry(_)
            | SavedPiece::OceanMonumentCore(_)
            | SavedPiece::OceanMonumentDoubleX(_)
            | SavedPiece::OceanMonumentDoubleXY(_)
            | SavedPiece::OceanMonumentDoubleY(_)
            | SavedPiece::OceanMonumentDoubleYZ(_)
            | SavedPiece::OceanMonumentDoubleZ(_)
            | SavedPiece::OceanMonumentPenthouse(_)
            | SavedPiece::OceanMonumentSimple(_)
            | SavedPiece::OceanMonumentSimpleTop(_)
            | SavedPiece::OceanMonumentWing(_) => Err(E::custom("the piece has no runtime form")),
            SavedPiece::OceanMonumentBuilding(tag) => tag.piece().map(Piece::OceanMonument),
            SavedPiece::MineshaftRoom {
                header: GridTag {
                    bounds, gen_depth, ..
                },
                mineshaft_type,
                entrances,
                ..
            } => self.mineshaft(
                mineshaft_type,
                MineshaftPiece {
                    kind: MineshaftKind::Room {
                        entrances: entrances.into_iter().map(box_of).collect(),
                    },
                    bounds: box_of(bounds),
                    direction: None,
                    gen_depth,
                },
            ),
            SavedPiece::MineshaftCorridor {
                header:
                    GridTag {
                        bounds,
                        orientation,
                        gen_depth,
                    },
                mineshaft_type,
                has_rails,
                spider_corridor,
                num_sections,
                ..
            } => self.mineshaft(
                mineshaft_type,
                MineshaftPiece {
                    kind: MineshaftKind::Corridor {
                        has_rails,
                        spider_corridor,
                        num_sections,
                        spawner_host: None,
                    },
                    bounds: box_of(bounds),
                    direction: Some(facing(orientation, "a mineshaft corridor")?),
                    gen_depth,
                },
            ),
            SavedPiece::MineshaftCrossing {
                header: GridTag {
                    bounds, gen_depth, ..
                },
                mineshaft_type,
                two_floored,
                direction,
                ..
            } => self.mineshaft(
                mineshaft_type,
                MineshaftPiece {
                    kind: MineshaftKind::Crossing { two_floored },
                    bounds: box_of(bounds),
                    direction: Some(facing(i32::from(direction), "a mineshaft crossing")?),
                    gen_depth,
                },
            ),
            SavedPiece::MineshaftStairs {
                header:
                    GridTag {
                        bounds,
                        orientation,
                        gen_depth,
                    },
                mineshaft_type,
            } => self.mineshaft(
                mineshaft_type,
                MineshaftPiece {
                    kind: MineshaftKind::Stairs,
                    bounds: box_of(bounds),
                    direction: Some(facing(orientation, "mineshaft stairs")?),
                    gen_depth,
                },
            ),
            SavedPiece::Igloo {
                header: GridTag { bounds, .. },
                template_x,
                template_y,
                template_z,
                template,
                rotation,
                ..
            } => {
                let bounds = box_of(bounds);
                let template = location::<E>(&template)?;
                Ok(Piece::Igloo(IglooPiece {
                    template: IglooTemplate::from_location(&template)
                        .ok_or_else(|| E::custom(format!("{template} is not an igloo template")))?,
                    position: IVec3::new(template_x, bounds.min.y, template_z),
                    rotation,
                    bounds,
                    height: template_y,
                }))
            }
            SavedPiece::NetherFossil {
                header: GridTag { bounds, .. },
                template_x,
                template_y,
                template_z,
                template,
                rotation,
                ..
            } => {
                let template = location::<E>(&template)?;
                Ok(Piece::NetherFossil(NetherFossilPiece {
                    template: *self.frozen.template_ids.get(&template).ok_or_else(|| {
                        E::custom(format!("the template {template} is not loaded"))
                    })?,
                    position: IVec3::new(template_x, template_y, template_z),
                    rotation,
                    bounds: box_of(bounds),
                }))
            }
            SavedPiece::EndCity {
                header: GridTag {
                    bounds, gen_depth, ..
                },
                template_x,
                template_y,
                template_z,
                template,
                rotation,
                overwrite,
                ..
            } => {
                let location = ResourceLocationBuf::minecraft(&format!(
                    "{END_CITY_TEMPLATE_PREFIX}{template}"
                ))
                .map_err(E::custom)?;
                let template = *self.frozen.template_ids.get(&location).ok_or_else(|| {
                    E::custom(format!("the end city template {location} is not loaded"))
                })?;
                Ok(Piece::EndCity(EndCityPiece {
                    template,
                    position: IVec3::new(template_x, template_y, template_z),
                    rotation,
                    overwrite,
                    bounds: box_of(bounds),
                    gen_depth,
                }))
            }
            SavedPiece::WoodlandMansion {
                header: GridTag { bounds, .. },
                template_x,
                template_y,
                template_z,
                template,
                rotation,
                mirror,
                ..
            } => {
                let name = ResourceLocationBuf::minecraft(&format!(
                    "{}{template}",
                    WoodlandMansionPiece::TEMPLATE_PREFIX
                ))
                .map_err(E::custom)?;
                let template = *self.frozen.template_ids.get(&name).ok_or_else(|| {
                    E::custom(format!("the mansion template {name} is not loaded"))
                })?;
                Ok(Piece::WoodlandMansion(WoodlandMansionPiece {
                    template,
                    position: IVec3::new(template_x, template_y, template_z),
                    rotation,
                    mirror,
                    bounds: box_of(bounds),
                }))
            }
            SavedPiece::StrongholdStart(tag) | SavedPiece::StrongholdStairsDown(tag) => DoorTag {
                header: tag.header,
                entry_door: tag.entry_door,
            }
            .piece(if tag.source {
                StrongholdKind::Start
            } else {
                StrongholdKind::StairsDown
            }),
            SavedPiece::StrongholdStraight {
                header,
                entry_door,
                left,
                right,
            } => DoorTag { header, entry_door }.piece(StrongholdKind::Straight { left, right }),
            SavedPiece::StrongholdPrisonHall(door) => door.piece(StrongholdKind::PrisonHall),
            SavedPiece::StrongholdLeftTurn(door) => door.piece(StrongholdKind::LeftTurn),
            SavedPiece::StrongholdRightTurn(door) => door.piece(StrongholdKind::RightTurn),
            SavedPiece::StrongholdRoomCrossing {
                header,
                entry_door,
                variant,
            } => DoorTag { header, entry_door }.piece(StrongholdKind::RoomCrossing { variant }),
            SavedPiece::StrongholdStraightStairsDown(door) => {
                door.piece(StrongholdKind::StraightStairsDown)
            }
            SavedPiece::StrongholdFiveCrossing {
                header,
                entry_door,
                left_low,
                left_high,
                right_low,
                right_high,
            } => DoorTag { header, entry_door }.piece(StrongholdKind::FiveCrossing {
                left_low,
                left_high,
                right_low,
                right_high,
            }),
            SavedPiece::StrongholdChestCorridor {
                header, entry_door, ..
            } => DoorTag { header, entry_door }.piece(StrongholdKind::ChestCorridor),
            SavedPiece::StrongholdLibrary {
                header,
                entry_door,
                tall,
            } => DoorTag { header, entry_door }.piece(StrongholdKind::Library { tall }),
            SavedPiece::StrongholdPortalRoom {
                header, entry_door, ..
            } => DoorTag { header, entry_door }.piece(StrongholdKind::PortalRoom),
            SavedPiece::StrongholdFillerCorridor {
                header,
                entry_door,
                steps,
            } => DoorTag { header, entry_door }.piece(StrongholdKind::FillerCorridor { steps }),
        }
    }
}

fn facing<E: serde::de::Error>(value: i32, what: &str) -> Result<Orientation, E> {
    Orientation::from_data_2d(value).ok_or_else(|| E::custom(format!("{what} without a facing")))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::Structure;
    use crate::frozen::{FrozenStructure, PoolId};
    use mcrs_minecraft_block_predicate::provider::Holder;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_nbt::deserializer::Deserializer as NbtDeserializer;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_nbt::{to_bytes, to_nbt_compound};
    use mcrs_minecraft_resource::rl;
    use mcrs_minecraft_worldgen_feature::placement::DecorationStep;
    use mcrs_voxel::BlockPos;

    fn frozen(liquid_settings: LiquidSettings) -> FrozenStructures {
        let mut frozen = FrozenStructures::default();
        let house = ResourceLocationBuf::read("minecraft:village/plains/houses/house_1").unwrap();
        let empty = ResourceLocationBuf::read("minecraft:empty").unwrap();
        frozen.template_ids.insert(house, TemplateId(0));
        frozen.template_ids.insert(
            rl!("minecraft:end_city/second_floor_1").to_buf(),
            TemplateId(1),
        );
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
            r##"{{"type":"minecraft:jigsaw","biomes":"#minecraft:is_overworld","spawn_overrides":{{}},"step":"surface_structures","start_pool":"minecraft:p","size":1,"start_height":{{"absolute":0}},"use_expansion_hack":true,"max_distance_from_center":80,"liquid_settings":"{}"}}"##,
            serde_json::to_value(liquid_settings)
                .unwrap()
                .as_str()
                .unwrap()
        );
        let Structure::Jigsaw { jigsaw, .. } = serde_json::from_str(&json).unwrap() else {
            panic!("the structure is a jigsaw")
        };
        frozen.structures.push(FrozenStructure {
            id: ResourceLocationBuf::read("minecraft:village_plains").unwrap(),
            step: DecorationStep::SurfaceStructures,
            step_index: 0,
            adaptation: TerrainAdaptation::BeardThin,
            biomes: Default::default(),
            kind: StructureKind::Jigsaw {
                start_pool: PoolId(0),
                config: jigsaw,
            },
        });
        frozen.structures.push(FrozenStructure {
            id: ResourceLocationBuf::read("minecraft:mineshaft_mesa").unwrap(),
            step: DecorationStep::UndergroundStructures,
            step_index: 0,
            adaptation: TerrainAdaptation::None,
            biomes: Default::default(),
            kind: StructureKind::Mineshaft {
                mineshaft_type: MineshaftType::Mesa,
                blocking: Default::default(),
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

    #[derive(Clone, Copy)]
    enum Field {
        Int(i32),
        Byte(i8),
        Str(&'static str),
        Ints(&'static [i32]),
    }

    type Fields = &'static [(&'static str, Field)];

    struct Case {
        liquid_settings: LiquidSettings,
        structure: StructureId,
        piece: Piece,
        fields: Vec<(&'static str, Field)>,
        child_count: Option<usize>,
    }

    fn case(piece: Piece, fields: &[(&'static str, Field)]) -> Case {
        Case {
            liquid_settings: LiquidSettings::ApplyWaterlogging,
            structure: StructureId(0),
            piece,
            fields: fields.to_vec(),
            child_count: None,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "lists one fixture per piece kind with the fields the game writes for it, so the cases read as the kinds"
    )]
    fn cases() -> Vec<Case> {
        use Field::*;
        let mut cases = vec![
            Case {
                child_count: Some(12),
                ..case(
                    Piece::JungleTemple(JungleTemplePiece {
                        bounds: BoundingBox {
                            min: BlockPos::new(-32, 64, 48),
                            max: BlockPos::new(-18, 73, 59),
                        },
                        orientation: Orientation::East,
                        height_position: 70,
                    }),
                    &[
                        ("id", Str("minecraft:tejp")),
                        ("O", Int(3)),
                        ("GD", Int(0)),
                        ("Width", Int(12)),
                        ("Height", Int(10)),
                        ("Depth", Int(15)),
                        ("HPos", Int(70)),
                        ("placedMainChest", Byte(0)),
                        ("placedTrap2", Byte(0)),
                    ],
                )
            },
            Case {
                child_count: Some(4),
                ..case(
                    Piece::BuriedTreasure(BuriedTreasurePiece {
                        bounds: BoundingBox::point(BlockPos::new(-23, 90, 41)),
                    }),
                    &[
                        ("id", Str("minecraft:btp")),
                        ("BB", Ints(&[-23, 90, 41, -23, 90, 41])),
                        ("O", Int(-1)),
                        ("GD", Int(0)),
                    ],
                )
            },
            Case {
                child_count: Some(9),
                ..case(
                    Piece::Igloo(IglooPiece {
                        template: IglooTemplate::Middle,
                        position: IVec3::new(-30, 84, 52),
                        rotation: Rotation::Counterclockwise90,
                        bounds: BoundingBox {
                            min: BlockPos::new(-30, 84, 52),
                            max: BlockPos::new(-28, 86, 54),
                        },
                        height: 57,
                    }),
                    &[
                        ("id", Str("minecraft:iglu")),
                        ("O", Int(2)),
                        ("GD", Int(0)),
                        ("TPX", Int(-30)),
                        ("TPY", Int(57)),
                        ("TPZ", Int(52)),
                        ("Template", Str("minecraft:igloo/middle")),
                        ("Rot", Str("COUNTERCLOCKWISE_90")),
                    ],
                )
            },
            Case {
                liquid_settings: LiquidSettings::IgnoreWaterlogging,
                ..case(
                    piece(0, Projection::Rigid),
                    &[("liquid_settings", Str("ignore_waterlogging"))],
                )
            },
        ];

        let fortress_bounds = BoundingBox {
            min: BlockPos::new(-94, 53, 146),
            max: BlockPos::new(-90, 62, 153),
        };
        let fortress: [(FortressKind, &str, Fields); 6] = [
            (FortressKind::BridgeCrossing, "minecraft:nebcr", &[]),
            (
                FortressKind::BridgeEndFiller { seed: -1_234_567 },
                "minecraft:nebef",
                &[("Seed", Int(-1_234_567))],
            ),
            (
                FortressKind::SmallCorridorLeftTurn { chest: true },
                "minecraft:nesclt",
                &[("Chest", Byte(1))],
            ),
            (
                FortressKind::SmallCorridorRightTurn { chest: false },
                "minecraft:nescrt",
                &[("Chest", Byte(0))],
            ),
            (
                FortressKind::MonsterThrone,
                "minecraft:nemt",
                &[("Mob", Byte(0))],
            ),
            (FortressKind::StairsRoom, "minecraft:nesr", &[]),
        ];
        for (kind, id, extra) in fortress {
            let mut fields = vec![
                ("id", Str(id)),
                ("O", Int(1)),
                ("GD", Int(7)),
                ("BB", Ints(&[-94, 53, 146, -90, 62, 153])),
            ];
            fields.extend_from_slice(extra);
            cases.push(case(
                Piece::Fortress(FortressPiece {
                    kind,
                    bounds: fortress_bounds,
                    orientation: Orientation::West,
                    gen_depth: 7,
                }),
                &fields,
            ));
        }

        let stronghold: [(StrongholdKind, SmallDoor, &str, &str, Fields); 13] = [
            (
                StrongholdKind::Start,
                SmallDoor::Opening,
                "minecraft:shstart",
                "OPENING",
                &[("Source", Byte(1))],
            ),
            (
                StrongholdKind::StairsDown,
                SmallDoor::WoodDoor,
                "minecraft:shsd",
                "WOOD_DOOR",
                &[("Source", Byte(0))],
            ),
            (
                StrongholdKind::Straight {
                    left: true,
                    right: false,
                },
                SmallDoor::IronDoor,
                "minecraft:shs",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::PrisonHall,
                SmallDoor::IronDoor,
                "minecraft:shph",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::LeftTurn,
                SmallDoor::IronDoor,
                "minecraft:shlt",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::RightTurn,
                SmallDoor::IronDoor,
                "minecraft:shrt",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::RoomCrossing { variant: 4 },
                SmallDoor::Opening,
                "minecraft:shrc",
                "OPENING",
                &[("Type", Int(4))],
            ),
            (
                StrongholdKind::StraightStairsDown,
                SmallDoor::IronDoor,
                "minecraft:shssd",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::FiveCrossing {
                    left_low: true,
                    left_high: false,
                    right_low: false,
                    right_high: true,
                },
                SmallDoor::Grates,
                "minecraft:sh5c",
                "GRATES",
                &[("leftLow", Byte(1)), ("rightLow", Byte(0))],
            ),
            (
                StrongholdKind::ChestCorridor,
                SmallDoor::Opening,
                "minecraft:shcc",
                "OPENING",
                &[("Chest", Byte(0))],
            ),
            (
                StrongholdKind::Library { tall: true },
                SmallDoor::IronDoor,
                "minecraft:shli",
                "IRON_DOOR",
                &[],
            ),
            (
                StrongholdKind::PortalRoom,
                SmallDoor::Opening,
                "minecraft:shpr",
                "OPENING",
                &[("Mob", Byte(0))],
            ),
            (
                StrongholdKind::FillerCorridor { steps: 3 },
                SmallDoor::Opening,
                "minecraft:shfc",
                "OPENING",
                &[("Steps", Int(3))],
            ),
        ];
        for (kind, entry_door, id, door, extra) in stronghold {
            let mut fields = vec![
                ("id", Str(id)),
                ("O", Int(0)),
                ("GD", Int(3)),
                ("EntryDoor", Str(door)),
            ];
            fields.extend_from_slice(extra);
            cases.push(case(
                Piece::Stronghold(StrongholdPiece {
                    kind,
                    entry_door,
                    bounds: BoundingBox {
                        min: BlockPos::new(1234, 24, -80),
                        max: BlockPos::new(1238, 28, -74),
                    },
                    orientation: Orientation::South,
                    gen_depth: 3,
                }),
                &fields,
            ));
        }

        type MineshaftRow = (
            MineshaftKind,
            Option<Orientation>,
            i32,
            &'static str,
            i32,
            Fields,
        );
        let mineshaft: [MineshaftRow; 4] = [
            (
                MineshaftKind::Room {
                    entrances: vec![mineshaft_entrance()],
                },
                None,
                0,
                "minecraft:msroom",
                -1,
                &[],
            ),
            (
                MineshaftKind::Corridor {
                    has_rails: true,
                    spider_corridor: false,
                    num_sections: 3,
                    spawner_host: None,
                },
                Some(Orientation::West),
                4,
                "minecraft:mscorridor",
                1,
                &[],
            ),
            (
                MineshaftKind::Crossing { two_floored: true },
                Some(Orientation::East),
                2,
                "minecraft:mscrossing",
                -1,
                &[("D", Byte(3)), ("tf", Byte(1))],
            ),
            (
                MineshaftKind::Stairs,
                Some(Orientation::South),
                7,
                "minecraft:msstairs",
                0,
                &[],
            ),
        ];
        for (kind, direction, gen_depth, id, orientation, extra) in mineshaft {
            let mut fields = vec![("id", Str(id)), ("O", Int(orientation)), ("MST", Int(1))];
            fields.extend_from_slice(extra);
            cases.push(Case {
                structure: StructureId(1),
                ..case(
                    Piece::Mineshaft(MineshaftPiece {
                        kind,
                        bounds: mineshaft_bounds(),
                        direction,
                        gen_depth,
                    }),
                    &fields,
                )
            });
        }
        cases
    }

    #[test]
    fn pieces_round_trip_through_nbt_as_the_reference_writes_them() {
        for Case {
            liquid_settings,
            structure,
            piece,
            fields,
            child_count,
        } in cases()
        {
            let frozen = frozen(liquid_settings);
            let context = PieceContext {
                frozen: &frozen,
                structure,
            };
            assert_eq!(round_trip(&context, &piece), piece);
            let tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
            for (key, field) in fields {
                let written = match field {
                    Field::Int(value) => tag.get_int(key) == Some(value),
                    Field::Byte(value) => tag.get_byte(key) == Some(value),
                    Field::Str(value) => tag.get_string(key) == Some(value),
                    Field::Ints(value) => tag.get_int_array(key) == Some(value),
                };
                assert!(written, "{key} of {piece:?}");
            }
            if let Some(count) = child_count {
                assert_eq!(tag.child_tags.len(), count, "{piece:?}");
            }
        }
    }

    fn mineshaft_bounds() -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(18, 25, -40),
            max: BlockPos::new(29, 33, -30),
        }
    }

    fn mineshaft_entrance() -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(20, 26, -40),
            max: BlockPos::new(22, 28, -39),
        }
    }

    #[test]
    fn the_monument_building_round_trips_with_its_room_graph() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let bounds = BoundingBox {
            min: BlockPos::new(-29, 39, -29),
            max: BlockPos::new(28, 61, 28),
        };
        let piece = Piece::OceanMonument(OceanMonumentPiece {
            bounds,
            orientation: Orientation::East,
            rooms: vec![MonumentRoom {
                index: 10,
                has_opening: [false, true, true, false, false, true],
                connections: [None, Some(1), Some(2), None, None, Some(3)],
            }],
            children: vec![MonumentChild {
                kind: MonumentRoomKind::Simple {
                    room: 0,
                    main_design: 2,
                },
                bounds: BoundingBox::point(BlockPos::new(-20, 39, -7)),
            }],
        });
        let tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
        assert_eq!(tag.get_string("id"), Some("minecraft:omb"));
        assert_eq!(tag.get_int("O"), Some(3));
        assert_eq!(tag.get_int("GD"), Some(0));
        assert_eq!(tag.child_tags.len(), 6);
        let Piece::OceanMonument(loaded) = round_trip(&context, &piece) else {
            panic!("not a monument");
        };
        let Piece::OceanMonument(expected) = piece else {
            panic!("not a monument");
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_monument_saved_without_the_room_extension_loads_without_a_graph() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let piece = Piece::OceanMonument(OceanMonumentPiece {
            bounds: BoundingBox::point(BlockPos::new(1, 2, 3)),
            orientation: Orientation::North,
            rooms: vec![MonumentRoom {
                index: 10,
                has_opening: [true; 6],
                connections: [Some(0); 6],
            }],
            children: vec![MonumentChild {
                kind: MonumentRoomKind::Penthouse,
                bounds: BoundingBox::point(BlockPos::new(1, 2, 3)),
            }],
        });
        let mut tag = to_nbt_compound(&piece.nbt(&context)).unwrap();
        tag.child_tags
            .retain(|(name, _)| name != "mcrs:rooms" && name != "mcrs:children");
        let mut bytes = Vec::new();
        to_bytes(&mcrs_minecraft_nbt::tag::NbtTag::Compound(tag), &mut bytes).unwrap();
        let loaded = PieceSeed(context)
            .deserialize(&mut NbtDeserializer::new(Cursor::new(bytes), true))
            .unwrap();
        let Piece::OceanMonument(loaded) = loaded else {
            panic!("not a monument");
        };
        assert!(loaded.rooms.is_empty());
        assert!(loaded.children.is_empty());
    }

    #[test]
    fn a_mineshaft_room_carries_its_entrances_and_its_mineshaft_type() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let mesa = PieceContext {
            frozen: &frozen,
            structure: StructureId(1),
        };
        let room = Piece::Mineshaft(MineshaftPiece {
            kind: MineshaftKind::Room {
                entrances: vec![mineshaft_entrance()],
            },
            bounds: mineshaft_bounds(),
            direction: None,
            gen_depth: 0,
        });
        let tag = to_nbt_compound(&room.nbt(&mesa)).unwrap();
        let entrances = tag.get_list("Entrances").unwrap();
        assert_eq!(
            entrances[0].extract_int_array().map(<[i32]>::to_vec),
            Some(vec![20, 26, -40, 22, 28, -39])
        );
        let mut moved = room.clone();
        moved.move_by(IVec3::new(0, 5, 0));
        let Piece::Mineshaft(MineshaftPiece {
            kind: MineshaftKind::Room { entrances },
            ..
        }) = &moved
        else {
            panic!("the moved piece is a mineshaft room")
        };
        assert_eq!(entrances[0].min.y, 31);

        let normal = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let mut bytes = Vec::new();
        to_bytes(&room.nbt(&mesa), &mut bytes).unwrap();
        assert!(
            PieceSeed(normal)
                .deserialize(&mut NbtDeserializer::new(Cursor::new(bytes), true))
                .is_err(),
            "a mesa piece does not load into a normal mineshaft"
        );
    }

    #[test]
    fn a_piece_the_frozen_set_cannot_resolve_is_refused() {
        let frozen = frozen(LiquidSettings::ApplyWaterlogging);
        let context = PieceContext {
            frozen: &frozen,
            structure: StructureId(0),
        };
        let jigsaw = |element: &str| {
            format!(
                r#"{{"id":"minecraft:jigsaw","BB":[0,0,0,1,1,1],"O":-1,"GD":0,"PosX":0,"PosY":0,"PosZ":0,"ground_level_delta":1,"pool_element":{element},"rotation":"NONE","junctions":[]}}"#
            )
        };
        for json in [
            jigsaw(
                r#"{"element_type":"minecraft:single_pool_element","location":"minecraft:nowhere","processors":"minecraft:empty","projection":"rigid"}"#,
            ),
            jigsaw(
                r#"{"element_type":"minecraft:single_pool_element","location":"minecraft:village/plains/houses/house_1","processors":"minecraft:mossify_10_percent","projection":"rigid"}"#,
            ),
            r#"{"id":"minecraft:iglu","BB":[0,0,0,1,1,1],"O":2,"GD":0,"TPX":0,"TPY":0,"TPZ":0,"Template":"minecraft:igloo/roof","Rot":"NONE"}"#.to_owned(),
        ] {
            let mut deserializer = serde_json::Deserializer::from_str(&json);
            assert!(PieceSeed(context).deserialize(&mut deserializer).is_err(), "{json}");
        }
    }

    fn sorted_tag(tag: &NbtTag) -> NbtTag {
        match tag {
            NbtTag::Compound(compound) => {
                let mut entries: Vec<_> = compound
                    .child_tags
                    .iter()
                    .map(|(key, value)| (key.clone(), sorted_tag(value)))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                NbtTag::Compound(NbtCompound {
                    child_tags: entries,
                })
            }
            NbtTag::List(items) => NbtTag::List(items.iter().map(sorted_tag).collect()),
            other => other.clone(),
        }
    }

    fn saved_piece(id: &str, keys: Vec<(&str, NbtTag)>) -> NbtCompound {
        let mut piece = NbtCompound::new();
        piece.put_string("id", format!("minecraft:{id}"));
        piece.put("BB", NbtTag::IntArray(vec![1, 2, 3, 4, 5, 6]));
        piece.put_int("O", 1);
        piece.put_int("GD", 3);
        for (key, value) in keys {
            piece.put(key, value);
        }
        piece
    }

    fn flags(keys: &[&'static str]) -> Vec<(&'static str, NbtTag)> {
        keys.iter().map(|key| (*key, NbtTag::Byte(1))).collect()
    }

    fn ints(keys: &[&'static str]) -> Vec<(&'static str, NbtTag)> {
        keys.iter().map(|key| (*key, NbtTag::Int(7))).collect()
    }

    fn door(mut keys: Vec<(&'static str, NbtTag)>) -> Vec<(&'static str, NbtTag)> {
        keys.insert(0, ("EntryDoor", NbtTag::String("WOOD_DOOR".to_owned())));
        keys
    }

    fn template(mut keys: Vec<(&'static str, NbtTag)>) -> Vec<(&'static str, NbtTag)> {
        keys.extend(ints(&["TPX", "TPY", "TPZ"]));
        keys.push(("Template", NbtTag::String("minecraft:igloo/top".to_owned())));
        keys
    }

    fn rot(mut keys: Vec<(&'static str, NbtTag)>) -> Vec<(&'static str, NbtTag)> {
        keys.push(("Rot", NbtTag::String("CLOCKWISE_90".to_owned())));
        keys
    }

    fn size(mut keys: Vec<(&'static str, NbtTag)>) -> Vec<(&'static str, NbtTag)> {
        keys.extend(ints(&["Width", "Height", "Depth", "HPos"]));
        keys
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one entry per piece type id of the saved layout"
    )]
    fn every_piece_type() -> Vec<NbtCompound> {
        let mut pieces = Vec::new();
        let mut add = |id: &str, keys: Vec<(&str, NbtTag)>| pieces.push(saved_piece(id, keys));
        let mut corridor = flags(&["hr", "sc", "hps"]);
        corridor.extend(ints(&["Num", "MST"]));
        add("mscorridor", corridor);
        let mut crossing = flags(&["tf"]);
        crossing.push(("D", NbtTag::Byte(3)));
        crossing.push(("MST", NbtTag::Int(1)));
        add("mscrossing", crossing);
        add(
            "msroom",
            vec![
                (
                    "Entrances",
                    NbtTag::List(vec![NbtTag::IntArray(vec![1, 2, 3, 4, 5, 6])]),
                ),
                ("MST", NbtTag::Int(1)),
            ],
        );
        add("msstairs", ints(&["MST"]));
        for id in [
            "nebcr",
            "nebs",
            "neccs",
            "nectb",
            "nece",
            "nescsc",
            "nesc",
            "necsr",
            "nerc",
            "nesr",
            "nestart",
            "btp",
            "omcr",
            "omdxr",
            "omdxyr",
            "omdyr",
            "omdyzr",
            "omdzr",
            "omentry",
            "ompenthouse",
            "omsimple",
            "omsimplet",
            "omwr",
        ] {
            add(id, Vec::new());
        }
        add("nebef", ints(&["Seed"]));
        add("nesclt", flags(&["Chest"]));
        add("nescrt", flags(&["Chest"]));
        add("nemt", flags(&["Mob"]));
        add("shcc", door(flags(&["Chest"])));
        add("shfc", door(ints(&["Steps"])));
        add(
            "sh5c",
            door(flags(&["leftLow", "leftHigh", "rightLow", "rightHigh"])),
        );
        for id in ["shlt", "shrt", "shph", "shssd"] {
            add(id, door(Vec::new()));
        }
        add("shli", door(flags(&["Tall"])));
        add("shpr", door(flags(&["Mob"])));
        add("shrc", door(ints(&["Type"])));
        add("shsd", door(flags(&["Source"])));
        add("shstart", door(flags(&["Source"])));
        add("shs", door(flags(&["Left", "Right"])));
        add(
            "tejp",
            size(flags(&[
                "placedMainChest",
                "placedHiddenChest",
                "placedTrap1",
                "placedTrap2",
            ])),
        );
        add(
            "tedp",
            size(flags(&[
                "hasPlacedChest0",
                "hasPlacedChest1",
                "hasPlacedChest2",
                "hasPlacedChest3",
            ])),
        );
        add("tesh", size(flags(&["Witch", "Cat"])));
        let mut room = NbtCompound::new();
        room.put_int("index", 4);
        room.put("openings", NbtTag::ByteArray(vec![1, 0, 1, 0, 1, 0].into()));
        room.put("connections", NbtTag::IntArray(vec![-1, 2, -1, 3, -1, 5]));
        let child = |kind: &str, extra: &[(&str, i32)]| {
            let mut child = NbtCompound::new();
            child.put_string("kind", kind.to_owned());
            child.put("bounds", NbtTag::IntArray(vec![1, 2, 3, 4, 5, 6]));
            for (key, value) in extra {
                child.put_int(key, *value);
            }
            NbtTag::Compound(child)
        };
        add(
            "omb",
            vec![
                ("mcrs:rooms", NbtTag::List(vec![NbtTag::Compound(room)])),
                (
                    "mcrs:children",
                    NbtTag::List(vec![
                        child("simple", &[("room", 4), ("design", 2)]),
                        child("wing", &[("design", 1)]),
                        child("penthouse", &[]),
                        child("double_xy", &[("room", 5)]),
                    ]),
                ),
            ],
        );
        add("iglu", rot(template(Vec::new())));
        add("nefos", rot(template(Vec::new())));
        let mut shipwreck = flags(&["isBeached", "height_adjusted"]);
        shipwreck.extend(template(Vec::new()));
        add("shipwreck", rot(shipwreck));
        let mut ruin = template(Vec::new());
        ruin.push(("Integrity", NbtTag::Float(0.5)));
        ruin.push(("BiomeType", NbtTag::String("COLD".to_owned())));
        ruin.extend(flags(&["IsLarge"]));
        add("orp", rot(ruin));
        add("ecp", rot(template(flags(&["OW"]))));
        let mut mansion = template(Vec::new());
        mansion.push(("Mi", NbtTag::String("LEFT_RIGHT".to_owned())));
        add("wmp", rot(mansion));
        let mut properties = NbtCompound::new();
        for key in [
            "cold",
            "air_pocket",
            "overgrown",
            "vines",
            "replace_with_blackstone",
        ] {
            properties.put_byte(key, 1);
        }
        properties.put_float("mossiness", 0.25);
        let mut portal = template(Vec::new());
        portal.push(("Rotation", NbtTag::String("CLOCKWISE_180".to_owned())));
        portal.push(("Mirror", NbtTag::String("FRONT_BACK".to_owned())));
        portal.push((
            "VerticalPlacement",
            NbtTag::String("partly_buried".to_owned()),
        ));
        portal.push(("Properties", NbtTag::Compound(properties)));
        add("rupo", portal);
        let mut junction = NbtCompound::new();
        for key in ["source_x", "source_ground_y", "source_z", "delta_y"] {
            junction.put_int(key, 3);
        }
        junction.put_string("dest_proj", "rigid".to_owned());
        let mut element = NbtCompound::new();
        element.put_string("element_type", "minecraft:empty_pool_element".to_owned());
        let mut jigsaw = ints(&["PosX", "PosY", "PosZ", "ground_level_delta"]);
        jigsaw.push(("pool_element", NbtTag::Compound(element)));
        jigsaw.push(("rotation", NbtTag::String("NONE".to_owned())));
        jigsaw.push(("junctions", NbtTag::List(vec![NbtTag::Compound(junction)])));
        jigsaw.push((
            "liquid_settings",
            NbtTag::String("apply_waterlogging".to_owned()),
        ));
        add("jigsaw", jigsaw);
        pieces
    }

    #[test]
    fn every_piece_type_loads_and_writes_the_same_compound() {
        use mcrs_minecraft_nbt::tag_deserializer::from_tag;
        let pieces = every_piece_type();
        assert_eq!(
            pieces.len(),
            mcrs_minecraft_anvil::structure_piece_keys::StructurePieceType::ALL.len()
        );
        for piece in pieces {
            let id = piece.get_string("id").unwrap().to_owned();
            let saved: SavedPiece = from_tag(NbtTag::Compound(piece.clone()))
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let written = to_nbt_compound(&saved).unwrap();
            assert_eq!(
                sorted_tag(&NbtTag::Compound(written)),
                sorted_tag(&NbtTag::Compound(piece)),
                "{id}"
            );
        }
    }
}
