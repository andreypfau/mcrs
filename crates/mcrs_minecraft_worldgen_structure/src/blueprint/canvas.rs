use std::collections::{BTreeMap, HashMap, HashSet};

use bevy_math::IVec3;
use mcrs_minecraft_core::{Axis, Direction, Mirror, ResourceLocation, Rotation, VERSION};
use mcrs_minecraft_worldgen_feature::template::{
    PaletteState, Template, TemplateBlock, TemplateEntity, transform, zero_position_with_transform,
};

use super::data::{BlockData, chest, compound, layered};
use super::rules::{self, Settle, World, axis_index};
use super::{Cell, Fields, Pos, Turn, block, state};
use mcrs_minecraft_block::keys::Block;

/// The box a template fills, painted in order: a later call overwrites an
/// earlier one, and the rule layer runs once when the template is built.
pub struct Canvas {
    size: Pos,
    world: HashMap<Pos, Cell>,
    pad_void: Option<i32>,
    kept: Vec<([i32; 3], [i32; 3], PaletteState)>,
    snow: Vec<String>,
    bare: Vec<([i32; 3], [i32; 3])>,
    block_data: HashMap<Pos, BlockData>,
    entities: Vec<TemplateEntity>,
}

/// Two slopes climbing towards the middle of `span`, extruded over `length`
/// along `ridge`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gable {
    pub ridge: Axis,
    pub span: [i32; 2],
    pub length: [i32; 2],
    pub y: i32,
    pub levels: i32,
}

impl Gable {
    pub const fn new(ridge: Axis, span: [i32; 2], length: [i32; 2], y: i32, levels: i32) -> Self {
        Gable {
            ridge,
            span,
            length,
            y,
            levels,
        }
    }
}

fn boxed(min: [i32; 3], max: [i32; 3]) -> impl Iterator<Item = Pos> {
    (min[0]..=max[0]).flat_map(move |x| {
        (min[1]..=max[1]).flat_map(move |y| (min[2]..=max[2]).map(move |z| (x, y, z)))
    })
}

fn moved(pos: Pos, turn: Turn, shift: [i32; 3]) -> Pos {
    let at = transform(
        IVec3::new(pos.0, pos.1, pos.2),
        turn.mirror,
        turn.rotation,
        IVec3::ZERO,
    );
    (at.x + shift[0], at.y + shift[1], at.z + shift[2])
}

impl Canvas {
    /// A box of air.
    pub fn new(size: [i32; 3]) -> Self {
        let air = block(Block::Air.as_static_str());
        let last = [size[0] - 1, size[1] - 1, size[2] - 1];
        Canvas {
            size: (size[0], size[1], size[2]),
            world: boxed([0; 3], last).map(|pos| (pos, air.clone())).collect(),
            pad_void: None,
            kept: Vec::new(),
            snow: Vec::new(),
            bare: Vec::new(),
            block_data: HashMap::new(),
            entities: Vec::new(),
        }
    }

    pub fn size(&self) -> [i32; 3] {
        [self.size.0, self.size.1, self.size.2]
    }

    fn put(&mut self, pos: Pos, cell: Cell) {
        let slot = self
            .world
            .get_mut(&pos)
            .unwrap_or_else(|| panic!("a block is painted outside the box at {pos:?}"));
        *slot = cell;
    }

    pub fn place(&mut self, block: &Cell, x: i32, y: i32, z: i32) {
        self.put((x, y, z), block.clone());
    }

    pub fn solid(&mut self, block: &Cell, min: [i32; 3], max: [i32; 3]) {
        for pos in boxed(min, max) {
            self.put(pos, block.clone());
        }
    }

    /// Cells that are not part of the template at all.
    pub fn void(&mut self, min: [i32; 3], max: [i32; 3]) {
        self.solid(&Cell::Void, min, max);
    }

    /// The same column of blocks, bottom layer first, over a rectangle.
    pub fn columns(&mut self, layers: &[&Cell], min: [i32; 2], max: [i32; 2]) {
        for x in min[0]..=max[0] {
            for z in min[1]..=max[1] {
                for (y, layer) in layers.iter().enumerate() {
                    self.put((x, y as i32, z), (*layer).clone());
                }
            }
        }
    }

    /// The four walls of a rectangle, with `post` at the corners.
    pub fn walls(&mut self, wall: &Cell, post: &Cell, min: [i32; 3], max: [i32; 3]) {
        for (x, y, z) in boxed(min, max) {
            match (x == min[0] || x == max[0], z == min[2] || z == max[2]) {
                (true, true) => self.put((x, y, z), post.clone()),
                (false, false) => {}
                _ => self.put((x, y, z), wall.clone()),
            }
        }
    }

    /// Rings of stairs facing inward from `min` to `[x, z]` of `max`, each a
    /// layer higher and a block smaller, with `lining` just inside each ring.
    pub fn hip_roof(
        &mut self,
        stairs: &str,
        lining: Option<&Cell>,
        min: [i32; 3],
        max: [i32; 2],
        levels: i32,
    ) {
        use Direction::{East, North, South, West};
        for i in 0..levels {
            let (a, b, c, d) = (min[0] + i, min[2] + i, max[0] - i, max[1] - i);
            if a > c || b > d || (a == c && b == d) {
                break;
            }
            for x in a..=c {
                for z in b..=d {
                    let facing = match (x == a, x == c, z == b, z == d) {
                        (true, _, true, _) => South,
                        (_, true, true, _) => West,
                        (_, true, _, true) => North,
                        (true, ..) => East,
                        (_, true, ..) => West,
                        (_, _, true, _) => South,
                        (_, _, _, true) => North,
                        _ => {
                            if let Some(lining) = lining
                                && (x == a + 1 || x == c - 1 || z == b + 1 || z == d - 1)
                            {
                                self.put((x, min[1] + i, z), lining.clone());
                            }
                            continue;
                        }
                    };
                    self.put((x, min[1] + i, z), super::stairs(stairs, facing));
                }
            }
        }
    }

    fn gable(&mut self, gable: &Gable, low: &Cell, high: &Cell, inside: [Option<&Cell>; 2]) {
        let [lining, top] = inside;
        let at = |u: i32, y: i32, r: i32| {
            if gable.ridge == Axis::Z {
                (u, y, r)
            } else {
                (r, y, u)
            }
        };
        for i in 0..gable.levels {
            let (a, b, y) = (gable.span[0] + i, gable.span[1] - i, gable.y + i);
            if a > b {
                break;
            }
            for r in gable.length[0]..=gable.length[1] {
                if a == b {
                    if let Some(top) = top {
                        self.put(at(a, y, r), top.clone());
                    }
                    continue;
                }
                self.put(at(a, y, r), low.clone());
                self.put(at(b, y, r), high.clone());
                if let Some(lining) = lining
                    && a + 1 < b
                {
                    self.put(at(a + 1, y, r), lining.clone());
                    self.put(at(b - 1, y, r), lining.clone());
                }
            }
        }
    }

    /// A gable of stairs facing the ridge, `lining` just inside each slope and
    /// `top` along the middle of an odd span.
    pub fn gable_roof(
        &mut self,
        gable: Gable,
        stairs: &str,
        lining: Option<&Cell>,
        top: Option<&Cell>,
    ) {
        let (low, high) = if gable.ridge == Axis::Z {
            (Direction::East, Direction::West)
        } else {
            (Direction::South, Direction::North)
        };
        let (low, high) = (super::stairs(stairs, low), super::stairs(stairs, high));
        self.gable(&gable, &low, &high, [lining, top]);
    }

    /// The stepped shape of a gable built from one block.
    pub fn stepped_gable(
        &mut self,
        gable: Gable,
        block: &Cell,
        lining: Option<&Cell>,
        top: Option<&Cell>,
    ) {
        self.gable(&gable, block, block, [lining, top]);
    }

    /// A box of `block`, and the same box moved along `axis` to start at `to`
    /// with the block mirrored across that axis.
    pub fn mirrored(&mut self, block: &Cell, axis: Axis, min: [i32; 3], max: [i32; 3], to: i32) {
        let mirror = if axis == Axis::X {
            Mirror::FrontBack
        } else {
            Mirror::LeftRight
        };
        let other = block.turned(Turn::new(mirror, Rotation::None));
        let mut shift = [0; 3];
        shift[axis_index(axis)] = to - min[axis_index(axis)];
        self.solid(block, min, max);
        for (x, y, z) in boxed(min, max) {
            self.put((x + shift[0], y + shift[1], z + shift[2]), other.clone());
        }
    }

    fn copied(&mut self, cells: Vec<(Pos, Cell)>, turn: Turn, shift: [i32; 3]) {
        for (pos, cell) in cells {
            let to = moved(pos, turn, shift);
            if let Some(slot) = self.world.get_mut(&to) {
                *slot = cell.turned(turn);
            }
        }
    }

    /// Every block of the box as it stands, turned and moved by `shift`; void
    /// cells are not copied.
    pub fn copy(&mut self, min: [i32; 3], max: [i32; 3], turn: Turn, shift: [i32; 3]) {
        let cells = boxed(min, max)
            .filter_map(|pos| Some((pos, self.world.get(&pos)?.clone())))
            .filter(|(_, cell)| *cell != Cell::Void)
            .collect();
        self.copied(cells, turn, shift);
    }

    /// Every column of the rectangle as it stands, turned and moved; a column
    /// that is void from bottom to top is not copied.
    pub fn copy_columns(&mut self, min: [i32; 2], max: [i32; 2], turn: Turn, shift: [i32; 2]) {
        let top = self.size.1 - 1;
        let cells = boxed([min[0], 0, min[1]], [max[0], top, max[1]])
            .filter(|(x, _, z)| {
                (0..=top).any(|y| self.world.get(&(*x, y, *z)) != Some(&Cell::Void))
            })
            .filter_map(|pos| Some((pos, self.world.get(&pos)?.clone())))
            .collect();
        self.copied(cells, turn, [shift[0], 0, shift[1]]);
    }

    /// Starts over from what `paint` leaves on a canvas of `size`, turned and
    /// set against the origin corner; cells it does not reach are void.
    pub fn variant_of(&mut self, size: [i32; 3], turn: Turn, paint: impl FnOnce(&mut Canvas)) {
        let mut original = Canvas::new(size);
        paint(&mut original);
        let corner =
            zero_position_with_transform(IVec3::ZERO, turn.mirror, turn.rotation, size[0], size[2]);
        for cell in self.world.values_mut() {
            *cell = Cell::Void;
        }
        self.copied(
            original.world.into_iter().collect(),
            turn,
            [corner.x, corner.y, corner.z],
        );
    }

    /// Replaces one block by another wherever it stands, keeping its properties.
    pub fn rename(&mut self, from: &str, to: &str) {
        let to = ResourceLocation::read(to).expect("a block id");
        for cell in self.world.values_mut() {
            if let Cell::Exact(state) | Cell::Settled(state) = cell
                && state.id.as_str() == from
            {
                state.id = to.clone();
            }
        }
    }

    /// In the lowest `height` layers, a cell under air or void is void.
    pub fn pad_void(&mut self, height: i32) {
        self.pad_void = Some(height);
    }

    /// A block the pad rule must leave in place.
    pub fn keep(&mut self, block_state: &str, min: [i32; 3], max: [i32; 3]) {
        self.kept.push((min, max, state(block_state)));
    }

    /// Thin snow on every cell open to the sky that stands on one of these
    /// supports: a block name, `soil` for grass and dirt, `BOTTOM` for the
    /// lowest layer.
    pub fn snow_on(&mut self, supports: &[&str]) {
        self.snow = supports.iter().map(|s| (*s).to_owned()).collect();
    }

    pub fn no_snow(&mut self, min: [i32; 3], max: [i32; 3]) {
        self.bare.push((min, max));
    }

    /// The data of a jigsaw whose target is its own name. A vertical jigsaw
    /// rolls, a horizontal one stays aligned.
    pub fn jigsaw(&mut self, at: [i32; 3], name: &str, pool: &str, final_state: &str) {
        self.jigsaw_data(at, name, pool, final_state, false);
    }

    /// A vertical jigsaw that stays aligned.
    pub fn jigsaw_aligned(&mut self, at: [i32; 3], name: &str, pool: &str, final_state: &str) {
        self.jigsaw_data(at, name, pool, final_state, true);
    }

    fn jigsaw_data(
        &mut self,
        at: [i32; 3],
        name: &str,
        pool: &str,
        final_state: &str,
        aligned: bool,
    ) {
        let data = BlockData::Jigsaw {
            name: name.to_owned(),
            pool: pool.to_owned(),
            final_state: final_state.to_owned(),
            aligned,
        };
        self.block_data.insert((at[0], at[1], at[2]), data);
    }

    pub fn chest(&mut self, at: [i32; 3], loot_table: &str) {
        self.block_data
            .insert((at[0], at[1], at[2]), chest(loot_table));
    }

    pub fn block_entity(&mut self, at: [i32; 3], data: Fields) {
        let data = BlockData::Fixed(compound(data));
        self.block_data.insert((at[0], at[1], at[2]), data);
    }

    /// An entity described in layers: what its kind is saved with, then how
    /// this one differs.
    pub fn entity(&mut self, pos: [f64; 3], block_pos: [i32; 3], nbt: &[Fields]) {
        self.entities.push(TemplateEntity {
            nbt: layered(nbt),
            block_pos,
            pos,
        });
    }

    /// What has been painted so far, before the rule layer.
    pub fn painted(&self) -> &HashMap<Pos, Cell> {
        &self.world
    }

    /// Every block of the finished template.
    pub fn cells(&self) -> BTreeMap<Pos, PaletteState> {
        let mut settled: HashSet<Pos> = HashSet::new();
        let mut world: World = HashMap::new();
        for (pos, cell) in &self.world {
            let state = match cell {
                Cell::Void => continue,
                Cell::Exact(state) => state.clone(),
                Cell::Settled(state) => {
                    settled.insert(*pos);
                    state.clone()
                }
                Cell::Ground => {
                    settled.insert(*pos);
                    rules::ground()
                }
            };
            world.insert(*pos, state);
        }

        let mut sources: Vec<(i32, Pos)> = world
            .iter()
            .filter_map(|(pos, state)| rules::partner(state, *pos).map(|(order, ..)| (order, *pos)))
            .collect();
        sources.sort();
        for (_, pos) in sources {
            if let Some((_, to, state)) = rules::partner(&world[&pos], pos) {
                world.insert(to, state);
                settled.remove(&to);
            }
        }
        if let Some(height) = self.pad_void {
            for cell in rules::pad_void(&world, self.size, height) {
                world.remove(&cell);
                settled.remove(&cell);
            }
        }
        for (min, max, state) in &self.kept {
            for cell in boxed(*min, *max) {
                world.insert(cell, state.clone());
                settled.remove(&cell);
            }
        }
        if !self.snow.is_empty() {
            let bare: HashSet<Pos> = self
                .bare
                .iter()
                .flat_map(|(min, max)| boxed(*min, *max))
                .collect();
            for (pos, class) in rules::snow_supports(&world) {
                if self.snow.contains(&class) && !bare.contains(&pos) {
                    world.insert(pos, rules::thin_snow());
                    settled.remove(&pos);
                }
            }
        }

        let stripped: World = world
            .iter()
            .map(|(pos, state)| {
                let state = if settled.contains(pos) {
                    state.clone()
                } else {
                    rules::strip(state)
                };
                (*pos, state)
            })
            .collect();
        let mut settle = Settle::new(&stripped, self.size);
        world
            .into_iter()
            .map(|(pos, state)| {
                let state = if settled.contains(&pos) {
                    settle.full(pos).unwrap_or(state)
                } else {
                    state
                };
                (pos, state)
            })
            .collect()
    }

    pub fn template(&self) -> Template {
        let mut palette: Vec<PaletteState> = Vec::new();
        let mut indices: HashMap<PaletteState, i32> = HashMap::new();
        let blocks = self
            .cells()
            .into_iter()
            .map(|(pos, state)| {
                let vertical = rules::property(&state, "orientation")
                    .is_some_and(|o| o.starts_with("up") || o.starts_with("down"));
                let next = palette.len() as i32;
                let index = *indices.entry(state).or_insert_with_key(|state| {
                    palette.push(state.clone());
                    next
                });
                TemplateBlock {
                    nbt: self.block_data.get(&pos).map(|data| data.to_nbt(vertical)),
                    pos: [pos.0, pos.1, pos.2],
                    state: index,
                }
            })
            .collect();
        let (x, y, z) = self.size;
        Template {
            size: [x, y, z],
            entities: self.entities.clone(),
            blocks,
            palette: Some(palette),
            palettes: None,
            data_version: VERSION.world_version,
        }
    }
}
