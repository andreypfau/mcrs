use crate::ceillog2;
use mcrs_voxel::{LocalPos, SectionPos};
use rustc_hash::FxHashMap;
use std::hash::Hash;

pub type AbstractCube<T, const DIM: usize> = [[[T; DIM]; DIM]; DIM];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeterogeneousPaletteData<V: Hash + Eq + Copy, const DIM: usize> {
    pub cube: AbstractCube<V, DIM>,
    pub palette: Vec<V>,
    pub counts: Vec<u16>,
    index: FxHashMap<V, usize>,
}

impl<V: Hash + Eq + Copy, const DIM: usize> HeterogeneousPaletteData<V, DIM> {
    fn get(&self, x: usize, y: usize, z: usize) -> Option<V> {
        self.cube.get(y)?.get(z)?.get(x).copied()
    }

    fn cell_mut(&mut self, x: usize, y: usize, z: usize) -> Option<&mut V> {
        self.cube.get_mut(y)?.get_mut(z)?.get_mut(x)
    }

    #[inline]
    fn palette_index(&self, value: &V) -> Option<usize> {
        self.index.get(value).copied()
    }

    fn release(&mut self, value: &V, index: usize) -> bool {
        let Some(count) = self.counts.get_mut(index) else {
            return false;
        };
        *count -= 1;
        if *count != 0 {
            return false;
        }
        self.index.remove(value);
        self.palette.swap_remove(index);
        self.counts.swap_remove(index);
        if let Some(&moved) = self.palette.get(index) {
            self.index.insert(moved, index);
        }
        true
    }

    fn acquire(&mut self, value: V) -> usize {
        match self.palette_index(&value) {
            Some(index) => {
                self.acquire_at(index);
                index
            }
            None => {
                let index = self.palette.len();
                self.palette.push(value);
                self.counts.push(1);
                self.index.insert(value, index);
                index
            }
        }
    }

    fn acquire_at(&mut self, index: usize) {
        if let Some(count) = self.counts.get_mut(index) {
            *count += 1;
        }
    }

    fn set(&mut self, x: usize, y: usize, z: usize, value: V) -> Option<V> {
        let original = std::mem::replace(self.cell_mut(x, y, z)?, value);
        if let Some(index) = self.palette_index(&original) {
            self.release(&original, index);
        }
        self.acquire(value);
        Some(original)
    }

    fn sole_value(&self) -> Option<V> {
        match self.palette.as_slice() {
            &[only] => Some(only),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PalettedContainer<V: Hash + Eq + Copy, const DIM: usize> {
    Homogeneous(V),
    Heterogeneous(Box<HeterogeneousPaletteData<V, DIM>>),
}

impl<V: Hash + Eq + Copy, const DIM: usize> PalettedContainer<V, DIM> {
    pub const SIZE: usize = DIM;
    pub const VOLUME: usize = DIM * DIM * DIM;

    #[expect(
        clippy::boxed_local,
        reason = "callers fill the cube in place on the heap, not on the stack"
    )]
    fn from_split_cube(
        cube: Box<AbstractCube<V, DIM>>,
        original: V,
        value: V,
        value_count: usize,
        value_is_first: bool,
    ) -> Self {
        let original_count = Self::VOLUME - value_count;
        let (palette, counts) = if value_is_first {
            (
                vec![value, original],
                vec![value_count as u16, original_count as u16],
            )
        } else {
            (
                vec![original, value],
                vec![original_count as u16, value_count as u16],
            )
        };
        let index: FxHashMap<V, usize> = palette.iter().enumerate().map(|(i, v)| (*v, i)).collect();
        Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
            cube: *cube,
            palette,
            counts,
            index,
        }))
    }

    #[expect(
        clippy::boxed_local,
        reason = "callers fill the cube in place on the heap, not on the stack"
    )]
    pub fn from_cube(cube: Box<AbstractCube<V, DIM>>) -> Self {
        let mut palette: Vec<V> = Vec::new();
        let mut counts: Vec<u16> = Vec::new();
        let mut index: FxHashMap<V, usize> = FxHashMap::default();

        let mut last: Option<(V, usize)> = None;
        for row in cube.as_flattened() {
            if let Some((prev, idx)) = last
                && *row == [prev; DIM]
            {
                if let Some(count) = counts.get_mut(idx) {
                    *count += DIM as u16;
                }
                continue;
            }
            for val in row {
                if let Some((prev, idx)) = last
                    && prev == *val
                {
                    if let Some(count) = counts.get_mut(idx) {
                        *count += 1;
                    }
                    continue;
                }
                let idx = match index.get(val) {
                    Some(&idx) => {
                        if let Some(count) = counts.get_mut(idx) {
                            *count += 1;
                        }
                        idx
                    }
                    None => {
                        let idx = palette.len();
                        index.insert(*val, idx);
                        palette.push(*val);
                        counts.push(1);
                        idx
                    }
                };
                last = Some((*val, idx));
            }
        }

        if let &[only] = palette.as_slice() {
            return Self::Homogeneous(only);
        }
        Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
            cube: *cube,
            palette,
            counts,
            index,
        }))
    }

    pub fn from_cells(cells: &[V]) -> Option<Self> {
        let (rows, []) = cells.as_chunks::<DIM>() else {
            return None;
        };
        let (planes, []) = rows.as_chunks::<DIM>() else {
            return None;
        };
        let ([cube], []) = planes.as_chunks::<DIM>() else {
            return None;
        };
        Some(Self::from_cube(Box::new(*cube)))
    }

    pub fn to_palette_and_packed_data(&self, bits_per_entry: u8) -> (Box<[V]>, Box<[i64]>) {
        match self {
            Self::Homogeneous(registry_id) => (Box::new([*registry_id]), Box::new([])),
            Self::Heterogeneous(data) => {
                debug_assert!(bits_per_entry as u32 >= ceillog2(data.counts.len()));
                debug_assert!(bits_per_entry <= 15);

                let cells = data.cube.as_flattened().as_flattened();
                let mut last: Option<(V, u32)> = None;
                let packed = crate::pack_from(bits_per_entry as u32, cells, |key| {
                    if let Some((value, index)) = last
                        && value == *key
                    {
                        return index;
                    }
                    let index = data.palette_index(key).map_or(0, |index| index as u32);
                    last = Some((*key, index));
                    index
                });

                (data.palette.clone().into_boxed_slice(), packed)
            }
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<V> {
        match self {
            Self::Homogeneous(value) => (x < DIM && y < DIM && z < DIM).then_some(*value),
            Self::Heterogeneous(data) => data.get(x, y, z),
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: V) -> Option<V> {
        match self {
            Self::Homogeneous(original) => {
                let original = *original;
                if value == original {
                    return (x < DIM && y < DIM && z < DIM).then_some(original);
                }
                let mut cube = Box::new([[[original; DIM]; DIM]; DIM]);
                *cube.get_mut(y)?.get_mut(z)?.get_mut(x)? = value;
                *self = Self::from_split_cube(cube, original, value, 1, x == 0 && y == 0 && z == 0);
                Some(original)
            }
            Self::Heterogeneous(data) => {
                let original = data.set(x, y, z, value)?;
                if let Some(only) = data.sole_value() {
                    *self = Self::Homogeneous(only);
                }
                Some(original)
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the lower and upper bound of the box on each axis, then the value to fill it with"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "the palette bookkeeping of each cell stays inline"
    )]
    pub fn fill_box(
        &mut self,
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
        z0: usize,
        z1: usize,
        value: V,
    ) {
        let (x1, y1, z1) = (x1.min(DIM), y1.min(DIM), z1.min(DIM));
        if x0 >= x1 || y0 >= y1 || z0 >= z1 {
            return;
        }
        match self {
            Self::Homogeneous(original) => {
                let original = *original;
                if value == original {
                    return;
                }
                let filled = (x1 - x0) * (y1 - y0) * (z1 - z0);
                if filled == Self::VOLUME {
                    *self = Self::Homogeneous(value);
                    return;
                }
                let mut cube = Box::new([[[original; DIM]; DIM]; DIM]);
                for plane in cube.get_mut(y0..y1).into_iter().flatten() {
                    for row in plane.get_mut(z0..z1).into_iter().flatten() {
                        if let Some(cells) = row.get_mut(x0..x1) {
                            cells.fill(value);
                        }
                    }
                }
                *self = Self::from_split_cube(
                    cube,
                    original,
                    value,
                    filled,
                    x0 == 0 && y0 == 0 && z0 == 0,
                );
            }
            Self::Heterogeneous(data) => {
                let mut orig_memo: Option<(V, usize)> = None;
                let mut value_idx: Option<usize> = None;
                for y in y0..y1 {
                    for z in z0..z1 {
                        for x in x0..x1 {
                            let Some(cell) = data.cell_mut(x, y, z) else {
                                continue;
                            };
                            let original = std::mem::replace(cell, value);
                            if original == value {
                                if let Some(index) = data.palette_index(&original) {
                                    data.release(&original, index);
                                }
                                data.acquire(value);
                                orig_memo = None;
                                value_idx = None;
                                continue;
                            }
                            let original_index = match orig_memo {
                                Some((o, idx)) if o == original => Some(idx),
                                _ => data.palette_index(&original),
                            };
                            if let Some(original_index) = original_index {
                                orig_memo = Some((original, original_index));
                                if data.release(&original, original_index) {
                                    orig_memo = None;
                                    value_idx = None;
                                }
                            }
                            match value_idx {
                                Some(vi) => data.acquire_at(vi),
                                None => value_idx = Some(data.acquire(value)),
                            }
                        }
                    }
                }
                if let Some(only) = data.sole_value() {
                    *self = Self::Homogeneous(only);
                }
            }
        }
    }

    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(V),
    {
        match self {
            Self::Homogeneous(registry_id) => {
                for _ in 0..Self::VOLUME {
                    f(*registry_id);
                }
            }
            Self::Heterogeneous(data) => {
                data.cube
                    .as_flattened()
                    .as_flattened()
                    .iter()
                    .for_each(|value| {
                        f(*value);
                    });
            }
        }
    }

    pub fn map<W: Hash + Eq + Copy, F: Fn(V) -> W>(&self, f: F) -> PalettedContainer<W, DIM> {
        match self {
            Self::Homogeneous(value) => PalettedContainer::Homogeneous(f(*value)),
            Self::Heterogeneous(data) => {
                let palette: Vec<W> = data.palette.iter().map(|value| f(*value)).collect();
                let index: FxHashMap<W, usize> =
                    palette.iter().enumerate().map(|(i, v)| (*v, i)).collect();
                let cube = Box::new(data.cube.map(|plane| plane.map(|row| row.map(&f))));
                if index.len() != palette.len() {
                    return PalettedContainer::from_cube(cube);
                }
                PalettedContainer::Heterogeneous(Box::new(HeterogeneousPaletteData {
                    cube: *cube,
                    palette,
                    counts: data.counts.clone(),
                    index,
                }))
            }
        }
    }
}

impl<V: Hash + Eq + Copy> PalettedContainer<V, { SectionPos::SIZE }> {
    pub fn get_local(&self, pos: LocalPos) -> V {
        match self {
            Self::Homogeneous(value) => *value,
            Self::Heterogeneous(data) => {
                let [[[corner, ..], ..], ..] = &data.cube;
                data.get(pos.x().into(), pos.y().into(), pos.z().into())
                    .unwrap_or(*corner)
            }
        }
    }

    pub fn set_local(&mut self, pos: LocalPos, value: V) -> V {
        self.set(pos.x().into(), pos.y().into(), pos.z().into(), value)
            .unwrap_or(value)
    }
}

impl<V: Default + Hash + Eq + Copy, const DIM: usize> Default for PalettedContainer<V, DIM> {
    fn default() -> Self {
        Self::Homogeneous(V::default())
    }
}
