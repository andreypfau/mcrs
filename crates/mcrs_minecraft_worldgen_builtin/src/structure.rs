macro_rules! kit {
    ($kit:ident; $($name:ident: $cell:expr,)*) => {
        struct Kit {
            $($name: Cell,)*
        }

        static $kit: std::sync::LazyLock<Kit> = std::sync::LazyLock::new(|| Kit {
            $($name: $cell,)*
        });
    };
}

/// The table of a village family whose villagers are of `kind`: under `both`
/// the pieces painted once for the living village and once for the abandoned
/// one, under `single` the rest. The villagers themselves are the same in
/// every family.
macro_rules! templates {
    (
        $family:literal $kind:literal;
        both { $($path:literal $size:tt $paint:expr;)* }
        single { $($one:literal $one_size:tt $single:expr;)* }
    ) => {
        const LIVING: Village = Village { family: $family, zombie: false };
        const ZOMBIE: Village = Village { family: $family, zombie: true };
        #[rustfmt::skip]
        const UNEMPLOYED: Fields = &[("VillagerData", Tag::Compound(&[("profession", Tag::String("minecraft:none")), ("level", Tag::Int(1)), ("type", Tag::String(concat!("minecraft:", $kind)))]))];
        #[rustfmt::skip]
        const NITWIT: Fields = &[("VillagerData", Tag::Compound(&[("profession", Tag::String("minecraft:nitwit")), ("level", Tag::Int(1)), ("type", Tag::String(concat!("minecraft:", $kind)))]))];

        pub const TEMPLATES: &[Entry] = &[
            $((concat!("village/", $family, "/", $path), $size, |c| ($paint)(c, LIVING)),)*
            $((concat!("village/", $family, "/zombie/", $path), $size, |c| ($paint)(c, ZOMBIE)),)*
            $((concat!("village/", $family, "/", $one), $one_size, $single),)*
            (concat!("village/", $family, "/villagers/baby"), [1, 2, 1], |c| baby(c, &[MOB, ANY_VILLAGER, VILLAGER, BABY, UNEMPLOYED])),
            (concat!("village/", $family, "/villagers/nitwit"), [1, 3, 1], |c| adult(c, &[MOB, ANY_VILLAGER, VILLAGER, ADULT, NITWIT])),
            (concat!("village/", $family, "/villagers/unemployed"), [1, 3, 1], |c| adult(c, &[MOB, ANY_VILLAGER, VILLAGER, ADULT, UNEMPLOYED])),
            (concat!("village/", $family, "/zombie/villagers/nitwit"), [1, 3, 1], |c| adult(c, &[MOB, ANY_VILLAGER, ZOMBIE_VILLAGER, ADULT, NITWIT])),
            (concat!("village/", $family, "/zombie/villagers/unemployed"), [1, 3, 1], |c| adult(c, &[MOB, ANY_VILLAGER, ZOMBIE_VILLAGER, ADULT, UNEMPLOYED])),
        ];
    };
}

mod village_common;
mod village_decays;
mod village_desert;
mod village_plains;
mod village_savanna;
mod village_snowy;
mod village_taiga;

use mcrs_minecraft_core::Axis::{X, Y, Z};
use mcrs_minecraft_core::Direction::{East, North, South, West};
use mcrs_minecraft_core::{Axis, Direction, ResourceLocation};
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_structure::blueprint::{
    Canvas, Cell, Fields, GROUND, Gable, Tag, Turn, VOID, block, corner_stairs, fence_joined, log,
    settled, stairs, wall_joined,
};

/// A template's name under `structure`, its size and the procedure that paints it.
type Entry = (&'static str, [i32; 3], fn(&mut Canvas));

const FAMILIES: [&[Entry]; 7] = [
    village_common::TEMPLATES,
    village_decays::TEMPLATES,
    village_desert::TEMPLATES,
    village_plains::TEMPLATES,
    village_savanna::TEMPLATES,
    village_snowy::TEMPLATES,
    village_taiga::TEMPLATES,
];

fn listed() -> impl Iterator<Item = &'static Entry> {
    FAMILIES.iter().flat_map(|family| family.iter())
}

pub fn keys() -> impl Iterator<Item = ResourceLocation> {
    listed().map(|(name, ..)| ResourceLocation::minecraft(name))
}

pub fn build(id: &ResourceLocation) -> Option<Template> {
    if id.namespace() != "minecraft" {
        return None;
    }
    let (_, size, paint) = listed().find(|(name, ..)| *name == id.path())?;
    let mut canvas = Canvas::new(*size);
    paint(&mut canvas);
    Some(canvas.template())
}

const AIR: &str = "minecraft:air";
const NOTHING: &str = "minecraft:structure_void";
const EMPTY: &str = "minecraft:empty";
const BOTTOM: &str = "minecraft:bottom";

const ANIMALS: &str = "minecraft:village/common/animals";
const BLUE_BED: &str = "minecraft:blue_bed";
const BUTCHER_ANIMALS: &str = "minecraft:village/common/butcher_animals";
const CATS: &str = "minecraft:village/common/cats";
const COBBLE: &str = "minecraft:cobblestone";
const COBBLE_STAIRS: &str = "minecraft:cobblestone_stairs";
const DIRT: &str = "minecraft:dirt";
const GRASS: &str = "minecraft:grass_block";
const IRON_GOLEM: &str = "minecraft:village/common/iron_golem";
const PATH: &str = "minecraft:dirt_path";
const RED_BED: &str = "minecraft:red_bed";
const SHEEP: &str = "minecraft:village/common/sheep";
const SPRUCE_DOOR: &str = "minecraft:spruce_door";
const SPRUCE_STAIRS: &str = "minecraft:spruce_stairs";
const WHITE_BED: &str = "minecraft:white_bed";

kit! {
    S;
    air: block(AIR),
    cobble: block(COBBLE),
    mossy: block("minecraft:mossy_cobblestone"),
    pane: settled("minecraft:glass_pane[waterlogged=false]"),
    cobble_wall: settled("minecraft:cobblestone_wall[waterlogged=false]"),
    torch: block("minecraft:torch"),
    wall_torch: settled("minecraft:wall_torch"),
    dirt: block(DIRT),
    grass: settled(GRASS),
    path: block(PATH),
    water: block("minecraft:water[level=0]"),
    farmland: block("minecraft:farmland[moisture=7]"),
    short_grass: block("minecraft:short_grass"),
    tall_grass: block("minecraft:tall_grass[half=lower]"),
    poppy: block("minecraft:poppy"),
    bookshelf: block("minecraft:bookshelf"),
    composter: block("minecraft:composter[level=0]"),
    crafting_table: block("minecraft:crafting_table"),
    spruce_planks: block("minecraft:spruce_planks"),
    spruce_slab_top: block("minecraft:spruce_slab[type=top,waterlogged=false]"),
    spruce_fence: settled("minecraft:spruce_fence[waterlogged=false]"),
}

fn wheat(age: i32) -> Cell {
    block(&format!("minecraft:wheat[age={age}]"))
}

fn path(c: &mut Canvas, along: Axis, middle: i32, range: [i32; 2]) {
    c.path_strip(&S.path, along, middle, range);
}

/// A village family, and whether the village is the abandoned one, whose
/// pieces draw from the zombie pools.
#[derive(Clone, Copy)]
struct Village {
    family: &'static str,
    zombie: bool,
}

impl Village {
    fn pool(self, kind: &str) -> String {
        let family = self.family;
        if self.zombie {
            format!("minecraft:village/{family}/zombie/{kind}")
        } else {
            format!("minecraft:village/{family}/{kind}")
        }
    }

    fn lit(self) -> bool {
        !self.zombie
    }
}

/// Leaves the air of the layers `ys` out of the template, but for `kept`.
fn open(c: &mut Canvas, ys: std::ops::RangeInclusive<i32>, kept: &[[i32; 3]]) {
    let air = block(AIR);
    let absent: Vec<[i32; 3]> = c
        .painted()
        .iter()
        .filter(|((_, y, _), cell)| ys.contains(y) && **cell == air)
        .map(|(&(x, y, z), _)| [x, y, z])
        .filter(|at| !kept.contains(at))
        .collect();
    for at in absent {
        c.void(at, at);
    }
}

fn spots(c: &mut Canvas, pool: &str, floor: &str, cells: &[[i32; 3]]) {
    for at in cells {
        c.spot(*at, pool, floor);
    }
}

fn villagers(c: &mut Canvas, v: Village, floor: &str, cells: &[[i32; 3]]) {
    spots(c, &v.pool("villagers"), floor, cells);
}

fn decorations(c: &mut Canvas, v: Village, soil: &str, cells: &[[i32; 2]]) {
    for [x, z] in cells {
        c.spot([*x, 0, *z], &v.pool("decor"), soil);
    }
}

fn street_ends(c: &mut Canvas, v: Village, ends: &[[i32; 2]]) {
    for [x, z] in ends {
        c.street_end([*x, 1, *z], &v.pool("streets"));
    }
}

fn street(c: &mut Canvas, v: Village, ends: &[[i32; 2]]) {
    c.jigsaw_layer(1);
    street_ends(c, v, ends);
}

fn houses(c: &mut Canvas, v: Village, side: Direction, line: i32, range: [i32; 2]) {
    c.house_sockets(side, line, range, &v.pool("houses"));
}

fn chest(c: &mut Canvas, at: [i32; 3], facing: Direction, loot: &str) {
    let chest = block(&format!(
        "minecraft:chest[facing={},type=single,waterlogged=false]",
        facing.name()
    ));
    c.place(&chest, at[0], at[1], at[2]);
    c.chest(at, &format!("minecraft:chests/village/{loot}"));
}

fn wall_torch(facing: Direction) -> Cell {
    block(&format!("minecraft:wall_torch[facing={}]", facing.name()))
}

/// `cells[0]` where x and z sum to an even number, `cells[1]` elsewhere.
fn checker(c: &mut Canvas, cells: [&Cell; 2], y: i32, min: [i32; 2], max: [i32; 2]) {
    for x in min[0]..=max[0] {
        for z in min[1]..=max[1] {
            c.place(cells[((x + z) % 2) as usize], x, y, z);
        }
    }
}

fn villager_marker(c: &mut Canvas, pos: [f64; 3], block_pos: [i32; 3], villager: &[Fields]) {
    c.socket_facing([0, 0, 0], "down_south", BOTTOM, EMPTY, NOTHING);
    c.entity(pos, block_pos, villager);
}

fn baby(c: &mut Canvas, villager: &[Fields]) {
    let at = [0.36605308557818717, 1.0, 1.0697962117287148];
    villager_marker(c, at, [0, 1, 1], villager);
}

fn adult(c: &mut Canvas, villager: &[Fields]) {
    let at = [0.7204986190855891, 1.0, 0.631455289898156];
    villager_marker(c, at, [0, 1, 0], villager);
}

#[rustfmt::skip]
mod mob {
    use super::{Fields, Tag};

    /// What every mob of the village templates is saved with.
    pub const MOB: Fields = &[("AbsorptionAmount", Tag::Float(0.0)), ("Air", Tag::Short(300)), ("ArmorItems", Tag::List(&[Tag::Compound(&[]), Tag::Compound(&[]), Tag::Compound(&[]), Tag::Compound(&[])])), ("CanPickUpLoot", Tag::Byte(0)), ("DeathTime", Tag::Short(0)), ("Dimension", Tag::Int(0)), ("FallFlying", Tag::Byte(0)), ("Fire", Tag::Short(-1)), ("HandItems", Tag::List(&[Tag::Compound(&[]), Tag::Compound(&[])])), ("Health", Tag::Float(20.0)), ("HurtByTimestamp", Tag::Int(0)), ("HurtTime", Tag::Short(0)), ("Invulnerable", Tag::Byte(0)), ("LeftHanded", Tag::Byte(0)), ("Motion", Tag::List(&[Tag::Double(0.0), Tag::Double(-0.0784000015258789), Tag::Double(0.0)])), ("OnGround", Tag::Byte(1)), ("PersistenceRequired", Tag::Byte(1)), ("PortalCooldown", Tag::Int(0)), ("attributes", Tag::List(&[])), ("fall_distance", Tag::Double(0.0))];
    pub const ANY_VILLAGER: Fields = &[("Pos", Tag::List(&[Tag::Double(-178.2795013809144), Tag::Double(5.0), Tag::Double(184.63145528989816)])), ("Rotation", Tag::List(&[Tag::Float(48.821632), Tag::Float(0.0)])), ("UUID", Tag::IntArray(&[1383272762, 272124144, -1415224788, -1032494613])), ("Age", Tag::Int(0)), ("CanPickUpLoot", Tag::Byte(1)), ("ForcedAge", Tag::Int(0)), ("Gossips", Tag::List(&[])), ("Inventory", Tag::List(&[])), ("Xp", Tag::Int(0))];
    pub const VILLAGER: Fields = &[("id", Tag::String("minecraft:villager")), ("PersistenceRequired", Tag::Byte(0))];
    pub const ZOMBIE_VILLAGER: Fields = &[("id", Tag::String("minecraft:zombie_villager"))];
    pub const ADULT: Fields = &[("food_level", Tag::Byte(0)), ("Leashed", Tag::Byte(0)), ("lastRestock", Tag::Long(0))];
    pub const BABY: Fields = &[("FoodLevel", Tag::Byte(0)), ("LastRestock", Tag::Long(0)), ("UUID", Tag::IntArray(&[895037032, -995736946, -1319125422, -1224629455])), ("Age", Tag::Int(-21359)), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(-25.827711)])), ("Pos", Tag::List(&[Tag::Double(-1740.6339469144218), Tag::Double(5.0), Tag::Double(496.0697962117287)]))];
}
use mob::*;
