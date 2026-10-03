//! What a block's neighbours decide about it: stair shapes, fence, pane and
//! wall connections, the facing of a block hung on a wall, the grass over
//! soil, and the second half of a door, a bed or a tall plant.

use std::collections::{BTreeMap, HashMap, HashSet};

use mcrs_minecraft_core::{Axis, Direction, ResourceLocation};
use mcrs_minecraft_worldgen_feature::template::PaletteState;

use super::Pos;
use super::turn::direction;

pub type World = HashMap<Pos, PaletteState>;

const GROUND: &str = "ground";

pub fn block(path: &str) -> PaletteState {
    PaletteState {
        id: ResourceLocation::minecraft(path),
        properties: None,
    }
}

pub fn ground() -> PaletteState {
    block(GROUND)
}

pub fn is_air(state: &PaletteState) -> bool {
    state.id.path() == "air"
}

pub fn property<'a>(state: &'a PaletteState, key: &str) -> Option<&'a str> {
    state.properties.as_ref()?.get(key).map(String::as_str)
}

pub fn with(state: &PaletteState, key: &str, value: &str) -> PaletteState {
    let mut state = state.clone();
    state
        .properties
        .get_or_insert_default()
        .insert(key.to_owned(), value.to_owned());
    state
}

fn keeping(state: &PaletteState, keep: impl Fn(&str) -> bool) -> PaletteState {
    let kept: BTreeMap<String, String> = state
        .properties
        .iter()
        .flatten()
        .filter(|(key, _)| keep(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    PaletteState {
        id: state.id.clone(),
        properties: (!kept.is_empty()).then_some(kept),
    }
}

pub fn offset(pos: Pos, by: Pos) -> Pos {
    (pos.0 + by.0, pos.1 + by.1, pos.2 + by.2)
}

pub fn toward(pos: Pos, direction: Direction) -> Pos {
    let step = direction.normal();
    offset(pos, (step.x, step.y, step.z))
}

fn above(pos: Pos) -> Pos {
    toward(pos, Direction::Up)
}

pub fn facing(state: &PaletteState) -> Option<Direction> {
    direction(property(state, "facing")?)
}

fn name(state: &PaletteState) -> &str {
    state.id.path()
}

fn is_stairs(state: &PaletteState) -> bool {
    name(state).ends_with("_stairs")
}

fn is_wall(state: &PaletteState) -> bool {
    name(state).ends_with("_wall")
}

fn is_pane(state: &PaletteState) -> bool {
    name(state).ends_with("glass_pane") || name(state) == "iron_bars"
}

pub fn is_attached(name: &str) -> bool {
    matches!(name, "wall_torch" | "ladder")
        || ["_wall_banner", "_wall_sign", "_trapdoor"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

pub fn is_double_plant(name: &str) -> bool {
    matches!(
        name,
        "tall_grass" | "large_fern" | "sunflower" | "lilac" | "rose_bush" | "peony"
    )
}

// chisle: name lists stand in for the collision shapes, because
// `BlockDefinitions` is only built through the asset server. Reading the full
// cube and sturdy face flags from the block definitions lifts it.
fn is_cube(state: &PaletteState) -> bool {
    const PARTIAL_SUFFIX: [&str; 17] = [
        "_door",
        "_fence",
        "_fence_gate",
        "_pressure_plate",
        "_sapling",
        "_slab",
        "_stairs",
        "_bed",
        "_carpet",
        "_banner",
        "_wall",
        "_button",
        "_trapdoor",
        "_stem",
        "_sign",
        "glass_pane",
        "_torch",
    ];
    const PARTIAL: [&str; 31] = [
        "air",
        "cave_air",
        "bell",
        "brewing_stand",
        "cactus",
        "campfire",
        "chest",
        "composter",
        "dandelion",
        "dead_bush",
        "dirt_path",
        "farmland",
        "fern",
        "grindstone",
        "iron_bars",
        "ladder",
        "lantern",
        "large_fern",
        "lava",
        "lectern",
        "oxeye_daisy",
        "poppy",
        "sea_pickle",
        "short_grass",
        "snow",
        "stonecutter",
        "tall_grass",
        "torch",
        "water",
        "water_cauldron",
        "wheat",
    ];
    let name = name(state);
    if name.ends_with("_slab") {
        return property(state, "type") == Some("double");
    }
    if name == "snow" {
        return property(state, "layers") == Some("8");
    }
    if name.starts_with("potted_") {
        return false;
    }
    !(PARTIAL.contains(&name) || PARTIAL_SUFFIX.iter().any(|suffix| name.ends_with(suffix)))
}

fn side_sturdy(state: Option<&PaletteState>, side: Direction) -> bool {
    let Some(state) = state else { return false };
    if is_cube(state) {
        return !matches!(name(state), "pumpkin" | "melon" | "oak_leaves");
    }
    name(state) == "composter" || (is_stairs(state) && facing(state) == Some(side))
}

fn top_sturdy(state: &PaletteState) -> bool {
    is_cube(state)
        || (name(state).ends_with("_slab") && property(state, "type") == Some("top"))
        || (is_stairs(state) && property(state, "half") == Some("top"))
}

fn gate_connects(state: &PaletteState, side: Direction) -> bool {
    name(state).ends_with("_fence_gate")
        && facing(state).is_some_and(|facing| facing.axis() != side.axis())
}

fn counter_clockwise(side: Direction) -> Direction {
    side.clockwise().opposite()
}

fn stair_state(grid: &World, pos: Pos) -> PaletteState {
    let state = &grid[&pos];
    let facing = facing(state).expect("stairs face a side");
    let half = property(state, "half");
    let neighbour = |side: Direction| {
        grid.get(&toward(pos, side))
            .filter(|other| is_stairs(other) && property(other, "half") == half)
            .and_then(self::facing)
    };
    let can_take = |side: Direction| neighbour(side) != Some(facing);
    let turns = |other: &Direction| other.axis() != facing.axis();
    let shape = if let Some(front) = neighbour(facing).filter(turns)
        && can_take(front.opposite())
    {
        if front == counter_clockwise(facing) {
            "outer_left"
        } else {
            "outer_right"
        }
    } else if let Some(back) = neighbour(facing.opposite()).filter(turns)
        && can_take(back)
    {
        if back == counter_clockwise(facing) {
            "inner_left"
        } else {
            "inner_right"
        }
    } else {
        "straight"
    };
    with(state, "shape", shape)
}

/// Completes stripped states from their neighbours in `grid`.
pub struct Settle<'a> {
    grid: &'a World,
    size: Pos,
    memo: HashMap<Pos, Option<PaletteState>>,
}

impl<'a> Settle<'a> {
    pub fn new(grid: &'a World, size: Pos) -> Self {
        Settle {
            grid,
            size,
            memo: HashMap::new(),
        }
    }

    /// `None` where there is no block or the neighbours leave it undecided.
    pub fn full(&mut self, pos: Pos) -> Option<PaletteState> {
        if let Some(done) = self.memo.get(&pos) {
            return done.clone();
        }
        let stripped = self.grid.get(&pos)?;
        self.memo.insert(pos, Some(stripped.clone()));
        let settled = self.settle(pos, stripped);
        self.memo.insert(pos, settled.clone());
        settled
    }

    fn snowy(&self, pos: Pos) -> &'static str {
        let above = self.grid.get(&above(pos));
        if above.is_some_and(|a| matches!(name(a), "snow" | "snow_block")) {
            "true"
        } else {
            "false"
        }
    }

    fn jigsaw(&self, pos: Pos, state: &PaletteState) -> PaletteState {
        let faces: Vec<Direction> = [
            (Direction::West, pos.0 == 0),
            (Direction::East, pos.0 == self.size.0 - 1),
            (Direction::North, pos.2 == 0),
            (Direction::South, pos.2 == self.size.2 - 1),
        ]
        .into_iter()
        .filter(|(_, on_face)| *on_face)
        .map(|(side, _)| side)
        .collect();
        let orientation = match faces[..] {
            [side] => format!("{}_up", side.name()),
            _ => "up_north".to_owned(),
        };
        with(state, "orientation", &orientation)
    }

    fn wall(&mut self, pos: Pos, state: &PaletteState) -> PaletteState {
        let grid = self.grid;
        let above = grid.get(&above(pos));
        let above_full = match above {
            Some(a) if is_wall(a) || is_pane(a) => self.full(self::above(pos)),
            other => other.cloned(),
        };
        let above_is = |key: &str, value: &str| {
            above_full.as_ref().and_then(|a| property(a, key)) == Some(value)
        };
        let covered = above.is_some_and(is_cube);
        let wall_above = above.is_some_and(is_wall);
        let pane_above = above.is_some_and(is_pane);
        let waterlogged = property(state, "waterlogged").unwrap_or("false");
        let mut out = with(&keeping(state, |_| false), "waterlogged", waterlogged);
        let mut heights: Vec<(Direction, &str)> = Vec::new();
        for side in Direction::HORIZONTAL {
            let other = grid.get(&toward(pos, side));
            let connects = other.is_some_and(|o| {
                is_wall(o)
                    || side_sturdy(other, side.opposite())
                    || is_pane(o)
                    || gate_connects(o, side)
            });
            let height = if !connects {
                "none"
            } else if covered
                || (wall_above && above_is(side.name(), "tall"))
                || (pane_above && above_is(side.name(), "true"))
            {
                "tall"
            } else {
                "low"
            };
            out = with(&out, side.name(), height);
            heights.push((side, height));
        }
        let is = |side: Direction, height: &str| heights.contains(&(side, height));
        let (north, south) = (Direction::North, Direction::South);
        let (east, west) = (Direction::East, Direction::West);
        let none = |side| is(side, "none");
        let tall = |side| is(side, "tall");
        let up = if (wall_above && above_is("up", "true"))
            || heights.iter().all(|(_, height)| *height == "none")
            || none(north) != none(south)
            || none(east) != none(west)
        {
            true
        } else if (tall(north) && tall(south)) || (tall(east) && tall(west)) {
            false
        } else {
            let top = above.map_or("", name);
            top == "torch"
                || ["_sign", "_banner", "_pressure_plate", "_fence", "_wall"]
                    .iter()
                    .any(|suffix| top.ends_with(suffix))
                || covered
                || pane_above
        };
        with(&out, "up", if up { "true" } else { "false" })
    }

    fn settle(&mut self, pos: Pos, state: &PaletteState) -> Option<PaletteState> {
        let grid = self.grid;
        let bare = name(state);
        if is_stairs(state) {
            return Some(stair_state(grid, pos));
        }
        if bare == "jigsaw" {
            return Some(self.jigsaw(pos, state));
        }
        if bare == GROUND {
            let buried = grid
                .get(&above(pos))
                .is_some_and(|a| is_cube(a) || matches!(name(a), "water" | "lava"));
            if buried {
                return Some(block("dirt"));
            }
            return Some(with(&block("grass_block"), "snowy", self.snowy(pos)));
        }
        if bare == "grass_block" {
            return Some(with(state, "snowy", self.snowy(pos)));
        }
        if is_wall(state) {
            return Some(self.wall(pos, state));
        }
        let pane = is_pane(state);
        if pane || bare.ends_with("_fence") {
            let mut out = state.clone();
            for side in Direction::HORIZONTAL {
                let other = grid.get(&toward(pos, side));
                let connects = other.is_some_and(|o| {
                    side_sturdy(other, side.opposite())
                        || (pane && (is_pane(o) || is_wall(o)))
                        || (!pane && (name(o).ends_with("_fence") || gate_connects(o, side)))
                });
                out = with(&out, side.name(), if connects { "true" } else { "false" });
            }
            return Some(out);
        }
        if is_attached(bare) {
            let supports: Vec<Direction> = Direction::HORIZONTAL
                .into_iter()
                .filter(|side| side_sturdy(grid.get(&toward(pos, *side)), side.opposite()))
                .collect();
            let [support] = supports[..] else {
                return None;
            };
            return Some(with(state, "facing", support.opposite().name()));
        }
        Some(state.clone())
    }
}

/// The state without the properties its neighbours decide.
pub fn strip(state: &PaletteState) -> PaletteState {
    let bare = name(state);
    if is_stairs(state) {
        keeping(state, |key| {
            matches!(key, "facing" | "half" | "waterlogged")
        })
    } else if is_wall(state) || bare.ends_with("_fence") || is_pane(state) {
        keeping(state, |key| key == "waterlogged")
    } else if bare == "grass_block" || bare == "jigsaw" {
        keeping(state, |_| false)
    } else if is_attached(bare) {
        keeping(state, |key| key != "facing")
    } else {
        state.clone()
    }
}

/// The half a lower door, a bed foot or a lower tall plant implies: the order
/// to visit it in, where the partner goes and what it is.
pub fn partner(state: &PaletteState, pos: Pos) -> Option<(i32, Pos, PaletteState)> {
    let bare = name(state);
    if (bare.ends_with("_door") || is_double_plant(bare))
        && property(state, "half") == Some("lower")
    {
        return Some((pos.1, above(pos), with(state, "half", "upper")));
    }
    if bare.ends_with("_bed") && property(state, "part") == Some("foot") {
        let towards = facing(state)?;
        let step = towards.normal();
        let order = pos.0 * step.x + pos.2 * step.z;
        return Some((order, toward(pos, towards), with(state, "part", "head")));
    }
    None
}

pub fn pad_void(world: &World, size: Pos, height: i32) -> HashSet<Pos> {
    let mut out = HashSet::new();
    for x in 0..size.0 {
        for z in 0..size.2 {
            for y in (0..height).rev() {
                let over = (x, y + 1, z);
                if world.get(&over).is_none_or(is_air) || out.contains(&over) {
                    out.insert((x, y, z));
                }
            }
        }
    }
    out
}

fn is_thin_snow(state: &PaletteState) -> bool {
    name(state) == "snow" && property(state, "layers") == Some("1")
}

pub fn thin_snow() -> PaletteState {
    with(&block("snow"), "layers", "1")
}

/// Every air or thin snow cell open to the sky, with the kind of block it
/// stands on.
pub fn snow_supports(world: &World) -> Vec<(Pos, String)> {
    let mut out = Vec::new();
    for (&pos, state) in world {
        if !is_air(state) && !is_thin_snow(state) {
            continue;
        }
        if pos.1 == 0 {
            out.push((pos, "BOTTOM".to_owned()));
            continue;
        }
        let Some(below) = world.get(&toward(pos, Direction::Down)) else {
            continue;
        };
        let support = name(below);
        if !top_sturdy(below) || matches!(support, "packed_ice" | "ice" | "barrier") {
            continue;
        }
        let open = (pos.1 + 1..)
            .map_while(|y| world.get(&(pos.0, y, pos.2)))
            .all(is_air);
        if open {
            let class = match support {
                "grass_block" | "dirt" | GROUND => "soil",
                other => other,
            };
            out.push((pos, class.to_owned()));
        }
    }
    out
}

pub fn stairs(block: &str, facing: Direction) -> PaletteState {
    let stairs = PaletteState {
        id: ResourceLocation::parse(block).expect("a block id"),
        properties: None,
    };
    let stairs = with(&stairs, "facing", facing.name());
    with(&with(&stairs, "half", "bottom"), "waterlogged", "false")
}

pub fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}
