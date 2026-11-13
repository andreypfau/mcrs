use mcrs_minecraft_core::{Mirror, Rotation, mirror, rotation};
use mcrs_minecraft_nbt::{nbt_byte_array, nbt_flag, nbt_int_array};
use mcrs_minecraft_registries::dispatch;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    Rigid,
    TerrainMatching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidSettings {
    IgnoreWaterlogging,
    #[default]
    ApplyWaterlogging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Junction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
    #[serde(default)]
    pub delta_y: i32,
    #[serde(rename = "dest_proj")]
    pub dest_projection: Projection,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortalProperties {
    pub cold: bool,
    pub mossiness: f32,
    pub air_pocket: bool,
    pub overgrown: bool,
    pub vines: bool,
    pub replace_with_blackstone: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalPlacement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
    InNether,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OceanTemperature {
    Warm,
    Cold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SmallDoor {
    Opening,
    WoodDoor,
    Grates,
    IronDoor,
}

/// A registry entry of a saved pool element. The save holds the id of the entry. An inline
/// value does not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryId(pub ResourceLocationBuf);

impl Serialize for RegistryId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

fn registry_id<'de, D: Deserializer<'de>>(
    deserializer: D,
    key: &'static str,
) -> Result<RegistryId, D::Error> {
    struct Id(&'static str);
    impl serde::de::Visitor<'_> for Id {
        type Value = RegistryId;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "`{}` as a registry id, not an inline value", self.0)
        }

        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<RegistryId, E> {
            ResourceLocationBuf::read(text)
                .map(RegistryId)
                .map_err(|_| {
                    E::custom(format!("`{}`: `{text}` is not a resource location", self.0))
                })
        }
    }
    deserializer.deserialize_any(Id(key))
}

fn processors_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<RegistryId, D::Error> {
    registry_id(deserializer, "processors")
}

fn feature_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<RegistryId, D::Error> {
    registry_id(deserializer, "feature")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedSingleElement {
    pub location: ResourceLocationBuf,
    #[serde(deserialize_with = "processors_id")]
    pub processors: RegistryId,
    pub projection: Projection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_liquid_settings: Option<LiquidSettings>,
}

/// The saved form of a jigsaw pool element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "element_type", deny_unknown_fields)]
pub enum SavedPoolElement {
    #[serde(rename = "minecraft:single_pool_element")]
    Single(SavedSingleElement),
    #[serde(rename = "minecraft:legacy_single_pool_element")]
    LegacySingle(SavedSingleElement),
    #[serde(rename = "minecraft:list_pool_element")]
    List {
        elements: Vec<SavedPoolElement>,
        projection: Projection,
    },
    #[serde(rename = "minecraft:feature_pool_element")]
    Feature {
        #[serde(deserialize_with = "feature_id")]
        feature: RegistryId,
        projection: Projection,
    },
    #[serde(rename = "minecraft:empty_pool_element")]
    Empty {},
}

fn serialize_byte<S: Serializer>(value: &u8, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_i8(i8::try_from(*value).unwrap_or(0))
}

fn deserialize_direction<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    struct Direction;
    impl serde::de::Visitor<'_> for Direction {
        type Value = u8;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number")
        }

        fn visit_i64<E>(self, value: i64) -> Result<u8, E> {
            Ok(u8::try_from((value % 4).abs()).unwrap_or(0))
        }

        fn visit_u64<E>(self, value: u64) -> Result<u8, E> {
            Ok(u8::try_from(value % 4).unwrap_or(0))
        }

        #[expect(clippy::cast_possible_truncation, reason = "the remainder is below 4")]
        fn visit_f64<E>(self, value: f64) -> Result<u8, E> {
            Ok(((value as i64) % 4).unsigned_abs() as u8)
        }
    }
    deserializer.deserialize_any(Direction)
}

/// The saved form of a structure piece, chosen by its `id`. A key that the save leaves out reads
/// as its default.
#[dispatch(registry = crate::structure_piece_keys::StructurePieceType, key = "id")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum SavedPiece {
    Jigsaw {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "PosX")]
        pos_x: i32,
        #[serde(rename = "PosY")]
        pos_y: i32,
        #[serde(rename = "PosZ")]
        pos_z: i32,
        ground_level_delta: i32,
        pool_element: SavedPoolElement,
        #[serde(with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(default)]
        junctions: Vec<Junction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        liquid_settings: Option<LiquidSettings>,
    },
    #[dispatch(Tedp)]
    DesertPyramid {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Width", default)]
        width: i32,
        #[serde(rename = "Height", default)]
        height: i32,
        #[serde(rename = "Depth", default)]
        depth: i32,
        #[serde(rename = "HPos", default)]
        height_position: i32,
        #[serde(rename = "hasPlacedChest0", default, deserialize_with = "nbt_flag")]
        has_placed_chest_0: bool,
        #[serde(rename = "hasPlacedChest1", default, deserialize_with = "nbt_flag")]
        has_placed_chest_1: bool,
        #[serde(rename = "hasPlacedChest2", default, deserialize_with = "nbt_flag")]
        has_placed_chest_2: bool,
        #[serde(rename = "hasPlacedChest3", default, deserialize_with = "nbt_flag")]
        has_placed_chest_3: bool,
    },
    #[dispatch(Tejp)]
    JungleTemple {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Width", default)]
        width: i32,
        #[serde(rename = "Height", default)]
        height: i32,
        #[serde(rename = "Depth", default)]
        depth: i32,
        #[serde(rename = "HPos", default)]
        height_position: i32,
        #[serde(rename = "placedMainChest", default, deserialize_with = "nbt_flag")]
        placed_main_chest: bool,
        #[serde(rename = "placedHiddenChest", default, deserialize_with = "nbt_flag")]
        placed_hidden_chest: bool,
        #[serde(rename = "placedTrap1", default, deserialize_with = "nbt_flag")]
        placed_trap_1: bool,
        #[serde(rename = "placedTrap2", default, deserialize_with = "nbt_flag")]
        placed_trap_2: bool,
    },
    #[dispatch(Tesh)]
    SwampHut {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Width", default)]
        width: i32,
        #[serde(rename = "Height", default)]
        height: i32,
        #[serde(rename = "Depth", default)]
        depth: i32,
        #[serde(rename = "HPos", default)]
        height_position: i32,
        #[serde(rename = "Witch", default, deserialize_with = "nbt_flag")]
        witch: bool,
        #[serde(rename = "Cat", default, deserialize_with = "nbt_flag")]
        cat: bool,
    },
    #[dispatch(Btp)]
    BuriedTreasure {
        #[serde(rename = "BB", serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        #[serde(rename = "O", default)]
        orientation: i32,
        #[serde(rename = "GD", default)]
        gen_depth: i32,
    },
    #[dispatch(Nestart)]
    FortressStart(GridTag),
    #[dispatch(Omcr)]
    OceanMonumentCore(GridTag),
    #[dispatch(Omdxr)]
    OceanMonumentDoubleX(GridTag),
    #[dispatch(Omdxyr)]
    OceanMonumentDoubleXY(GridTag),
    #[dispatch(Omdyr)]
    OceanMonumentDoubleY(GridTag),
    #[dispatch(Omdyzr)]
    OceanMonumentDoubleYZ(GridTag),
    #[dispatch(Omdzr)]
    OceanMonumentDoubleZ(GridTag),
    #[dispatch(Omentry)]
    OceanMonumentEntry(GridTag),
    #[dispatch(Ompenthouse)]
    OceanMonumentPenthouse(GridTag),
    #[dispatch(Omsimple)]
    OceanMonumentSimple(GridTag),
    #[dispatch(Omsimplet)]
    OceanMonumentSimpleTop(GridTag),
    #[dispatch(Omwr)]
    OceanMonumentWing(GridTag),
    Shipwreck {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "isBeached", default, deserialize_with = "nbt_flag")]
        is_beached: bool,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(default, deserialize_with = "nbt_flag")]
        height_adjusted: bool,
    },
    #[dispatch(Nebcr)]
    FortressBridgeCrossing(GridTag),
    #[dispatch(Nebef)]
    FortressBridgeEndFiller {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Seed", default)]
        seed: i32,
    },
    #[dispatch(Nebs)]
    FortressBridgeStraight(GridTag),
    #[dispatch(Neccs)]
    FortressCorridorStairs(GridTag),
    #[dispatch(Nectb)]
    FortressCorridorBalcony(GridTag),
    #[dispatch(Nece)]
    FortressCastleEntrance(GridTag),
    #[dispatch(Nescsc)]
    FortressSmallCorridorCrossing(GridTag),
    #[dispatch(Nesclt)]
    FortressSmallCorridorLeftTurn {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Chest", default, deserialize_with = "nbt_flag")]
        chest: bool,
    },
    #[dispatch(Nesc)]
    FortressSmallCorridor(GridTag),
    #[dispatch(Nescrt)]
    FortressSmallCorridorRightTurn {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Chest", default, deserialize_with = "nbt_flag")]
        chest: bool,
    },
    #[dispatch(Necsr)]
    FortressStalkRoom(GridTag),
    #[dispatch(Nemt)]
    FortressMonsterThrone {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "Mob", default, deserialize_with = "nbt_flag")]
        mob: bool,
    },
    #[dispatch(Nerc)]
    FortressRoomCrossing(GridTag),
    #[dispatch(Nesr)]
    FortressStairsRoom(GridTag),
    #[dispatch(Orp)]
    OceanRuin {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(rename = "Integrity", default)]
        integrity: f32,
        #[serde(rename = "BiomeType", with = "biome_type")]
        biome_temp: OceanTemperature,
        #[serde(rename = "IsLarge", default, deserialize_with = "nbt_flag")]
        large: bool,
    },
    #[dispatch(Rupo)]
    RuinedPortal {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        pos_x: i32,
        #[serde(rename = "TPY", default)]
        pos_y: i32,
        #[serde(rename = "TPZ", default)]
        pos_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rotation", with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(rename = "Mirror", with = "mirror::legacy")]
        mirror: Mirror,
        #[serde(rename = "VerticalPlacement")]
        placement: PortalPlacement,
        #[serde(rename = "Properties")]
        properties: PortalProperties,
    },
    #[dispatch(Omb)]
    OceanMonumentBuilding(MonumentBuildingTag),
    #[dispatch(Msroom)]
    MineshaftRoom {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "MST", default)]
        mineshaft_type: i32,
        #[serde(rename = "Entrances", default, serialize_with = "nbt_int_arrays")]
        entrances: Vec<[i32; 6]>,
    },
    #[dispatch(Mscorridor)]
    MineshaftCorridor {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "MST", default)]
        mineshaft_type: i32,
        #[serde(rename = "hr", default, deserialize_with = "nbt_flag")]
        has_rails: bool,
        #[serde(rename = "sc", default, deserialize_with = "nbt_flag")]
        spider_corridor: bool,
        #[serde(rename = "hps", default, deserialize_with = "nbt_flag")]
        has_placed_spider: bool,
        #[serde(rename = "Num", default)]
        num_sections: i32,
    },
    #[dispatch(Mscrossing)]
    MineshaftCrossing {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "MST", default)]
        mineshaft_type: i32,
        #[serde(rename = "tf", default, deserialize_with = "nbt_flag")]
        two_floored: bool,
        #[serde(
            rename = "D",
            default,
            serialize_with = "serialize_byte",
            deserialize_with = "deserialize_direction"
        )]
        direction: u8,
    },
    #[dispatch(Msstairs)]
    MineshaftStairs {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "MST", default)]
        mineshaft_type: i32,
    },
    #[dispatch(Iglu)]
    Igloo {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
    },
    #[dispatch(Nefos)]
    NetherFossil {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
    },
    #[dispatch(Ecp)]
    EndCity {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(rename = "OW", default, deserialize_with = "nbt_flag")]
        overwrite: bool,
    },
    #[dispatch(Shstart)]
    StrongholdStart(StairsDownTag),
    #[dispatch(Shsd)]
    StrongholdStairsDown(StairsDownTag),
    #[dispatch(Shs)]
    StrongholdStraight {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Left", default, deserialize_with = "nbt_flag")]
        left: bool,
        #[serde(rename = "Right", default, deserialize_with = "nbt_flag")]
        right: bool,
    },
    #[dispatch(Shph)]
    StrongholdPrisonHall(DoorTag),
    #[dispatch(Shlt)]
    StrongholdLeftTurn(DoorTag),
    #[dispatch(Shrt)]
    StrongholdRightTurn(DoorTag),
    #[dispatch(Shrc)]
    StrongholdRoomCrossing {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Type", default)]
        variant: i32,
    },
    #[dispatch(Shssd)]
    StrongholdStraightStairsDown(DoorTag),
    #[dispatch(Sh5c)]
    StrongholdFiveCrossing {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "leftLow", default, deserialize_with = "nbt_flag")]
        left_low: bool,
        #[serde(rename = "leftHigh", default, deserialize_with = "nbt_flag")]
        left_high: bool,
        #[serde(rename = "rightLow", default, deserialize_with = "nbt_flag")]
        right_low: bool,
        #[serde(rename = "rightHigh", default, deserialize_with = "nbt_flag")]
        right_high: bool,
    },
    #[dispatch(Shcc)]
    StrongholdChestCorridor {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Chest", default, deserialize_with = "nbt_flag")]
        chest: bool,
    },
    #[dispatch(Shli)]
    StrongholdLibrary {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Tall", default, deserialize_with = "nbt_flag")]
        tall: bool,
    },
    #[dispatch(Shpr)]
    StrongholdPortalRoom {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Mob", default, deserialize_with = "nbt_flag")]
        mob: bool,
    },
    #[dispatch(Shfc)]
    StrongholdFillerCorridor {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "EntryDoor")]
        entry_door: SmallDoor,
        #[serde(rename = "Steps", default)]
        steps: i32,
    },
    #[dispatch(Wmp)]
    WoodlandMansion {
        #[serde(flatten)]
        header: GridTag,
        #[serde(rename = "TPX", default)]
        template_x: i32,
        #[serde(rename = "TPY", default)]
        template_y: i32,
        #[serde(rename = "TPZ", default)]
        template_z: i32,
        #[serde(rename = "Template", default)]
        template: String,
        #[serde(rename = "Rot", with = "rotation::legacy")]
        rotation: Rotation,
        #[serde(rename = "Mi", with = "mirror::legacy")]
        mirror: Mirror,
    },
}

/// The ocean ruin kind, stored as the upper-case name of the kind.
mod biome_type {
    use super::*;

    pub fn serialize<S: Serializer>(
        temp: &OceanTemperature,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match temp {
            OceanTemperature::Warm => "WARM",
            OceanTemperature::Cold => "COLD",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<OceanTemperature, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "WARM" => Ok(OceanTemperature::Warm),
            "COLD" => Ok(OceanTemperature::Cold),
            name => Err(D::Error::custom(format!("No value with id: {name}"))),
        }
    }
}

struct IntArray<'a>(&'a [i32; 6]);

impl Serialize for IntArray<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        nbt_int_array(self.0, serializer)
    }
}

/// A list of int arrays.
fn nbt_int_arrays<S: Serializer>(boxes: &[[i32; 6]], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(boxes.iter().map(IntArray))
}

/// What every stronghold piece writes after the common piece keys: its entry door.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorTag {
    #[serde(flatten)]
    pub header: GridTag,
    #[serde(rename = "EntryDoor")]
    pub entry_door: SmallDoor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StairsDownTag {
    #[serde(flatten)]
    pub header: GridTag,
    #[serde(rename = "EntryDoor")]
    pub entry_door: SmallDoor,
    #[serde(rename = "Source", default, deserialize_with = "nbt_flag")]
    pub source: bool,
}

/// The ocean monument piece: the grid fields plus the room graph that the vanilla format leaves
/// out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonumentBuildingTag {
    #[serde(flatten)]
    pub header: GridTag,
    #[serde(rename = "mcrs:rooms", default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<MonumentRoomTag>,
    #[serde(
        rename = "mcrs:children",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub children: Vec<MonumentChildTag>,
}

/// One room of the room graph that `mcrs:rooms` lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonumentRoomTag {
    pub index: i32,
    #[serde(serialize_with = "nbt_byte_array")]
    pub openings: [i8; 6],
    #[serde(serialize_with = "nbt_int_array")]
    pub connections: [i32; 6],
}

/// One room piece of the monument that `mcrs:children` lists. The key `room` is present for
/// every kind except `wing` and `penthouse`, and `design` for `simple` and `wing`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MonumentChildTag {
    Entry {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    Core {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    DoubleX {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    #[serde(rename = "double_xy")]
    DoubleXY {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    DoubleY {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    #[serde(rename = "double_yz")]
    DoubleYZ {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    DoubleZ {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    Simple {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
        design: i32,
    },
    SimpleTop {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        room: i32,
    },
    Wing {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
        design: i32,
    },
    Penthouse {
        #[serde(serialize_with = "nbt_int_array")]
        bounds: [i32; 6],
    },
}

/// The common piece keys without the id: what every grid piece writes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridTag {
    #[serde(rename = "BB", serialize_with = "nbt_int_array")]
    pub bounds: [i32; 6],
    #[serde(rename = "O", default)]
    pub orientation: i32,
    #[serde(rename = "GD", default)]
    pub gen_depth: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_nbt::tag::NbtTag;

    fn piece(id: &str, keys: Vec<(&str, NbtTag)>) -> NbtCompound {
        let mut piece = NbtCompound::new();
        piece.put_string("id", id.to_owned());
        piece.put("BB", NbtTag::IntArray(vec![0, 1, 2, 3, 4, 5]));
        for (key, value) in keys {
            piece.put(key, value);
        }
        piece
    }

    fn read(piece: NbtCompound) -> SavedPiece {
        mcrs_minecraft_nbt::from_tag(NbtTag::Compound(piece)).unwrap()
    }

    fn written(piece: &SavedPiece) -> NbtCompound {
        let mut bytes = Vec::new();
        mcrs_minecraft_nbt::to_bytes(piece, &mut bytes).unwrap();
        mcrs_minecraft_nbt::from_bytes(std::io::Cursor::new(&bytes[..])).unwrap()
    }

    #[test]
    fn a_crossing_direction_of_any_number_type_reads_modulo_four_and_writes_a_byte() {
        for (tag, expected) in [
            (NbtTag::Int(5), 1),
            (NbtTag::Int(-6), 2),
            (NbtTag::Long(7), 3),
            (NbtTag::Byte(2), 2),
            (NbtTag::Short(-4), 0),
        ] {
            let saved = read(piece("minecraft:mscrossing", vec![("D", tag)]));
            let SavedPiece::MineshaftCrossing { direction, .. } = &saved else {
                panic!("{saved:?}");
            };
            assert_eq!(*direction, expected);
            assert_eq!(
                written(&saved).get("D"),
                Some(&NbtTag::Byte(expected as i8))
            );
        }
    }

    #[test]
    fn a_ruined_portal_without_a_template_position_reads_zero_and_writes_every_key() {
        let mut properties = NbtCompound::new();
        for key in [
            "cold",
            "air_pocket",
            "overgrown",
            "vines",
            "replace_with_blackstone",
        ] {
            properties.put_byte(key, 0);
        }
        properties.put("mossiness", NbtTag::Float(0.5));
        let saved = read(piece(
            "minecraft:rupo",
            vec![
                ("Rotation", NbtTag::String("NONE".into())),
                ("Mirror", NbtTag::String("NONE".into())),
                (
                    "VerticalPlacement",
                    NbtTag::String("on_land_surface".into()),
                ),
                ("Properties", NbtTag::Compound(properties)),
            ],
        ));
        let out = written(&saved);
        for key in ["TPX", "TPY", "TPZ"] {
            assert_eq!(out.get(key), Some(&NbtTag::Int(0)), "{key}");
        }
        assert_eq!(out.get("Template"), Some(&NbtTag::String(String::new())));
    }

    #[test]
    fn an_inline_processor_list_or_feature_fails_the_load_naming_its_key() {
        let element = |key: &str, element_type: &str| {
            let mut element = NbtCompound::new();
            element.put_string("element_type", element_type.to_owned());
            element.put_string("projection", "rigid".to_owned());
            if key == "processors" {
                element.put_string("location", "minecraft:a".to_owned());
            }
            element.put_component(key, NbtCompound::new());
            element
        };
        for (key, element_type) in [
            ("processors", "minecraft:single_pool_element"),
            ("feature", "minecraft:feature_pool_element"),
        ] {
            let jigsaw = piece(
                "minecraft:jigsaw",
                vec![
                    ("PosX", NbtTag::Int(0)),
                    ("PosY", NbtTag::Int(0)),
                    ("PosZ", NbtTag::Int(0)),
                    ("ground_level_delta", NbtTag::Int(1)),
                    ("rotation", NbtTag::String("NONE".into())),
                    ("pool_element", NbtTag::Compound(element(key, element_type))),
                ],
            );
            let error =
                mcrs_minecraft_nbt::from_tag::<SavedPiece>(NbtTag::Compound(jigsaw)).unwrap_err();
            assert!(error.to_string().contains(key), "{error}");
        }
    }
}
