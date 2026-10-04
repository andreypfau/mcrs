use std::collections::BTreeSet;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub offset: usize,
    size: usize,
}

impl Block {
    pub const EMPTY: Self = Self { offset: 0, size: 0 };

    /// The units the block really holds, which the buddy rounding leaves above what was asked.
    pub fn capacity(&self) -> usize {
        self.size
    }
}

pub struct Arena {
    capacity: usize,
    free: Vec<BTreeSet<usize>>,
    held: usize,
}

impl Arena {
    pub fn new(capacity: usize) -> Self {
        let classes = class_of(capacity.max(1)) + 1;
        let mut free: Vec<BTreeSet<usize>> = (0..classes).map(|_| BTreeSet::new()).collect();
        let mut offset = 0;
        for class in (0..classes).rev() {
            if capacity & (1 << class) != 0 {
                free[class].insert(offset);
                offset += 1 << class;
            }
        }
        Self {
            capacity,
            free,
            held: 0,
        }
    }

    pub fn alloc(&mut self, units: usize) -> Option<Block> {
        if units == 0 {
            return Some(Block::EMPTY);
        }
        let want = class_of(units);
        // The lowest free block of any class that fits, so what is handed out packs towards
        // offset zero: the GPU buffer behind the arena only grows to the highest block in use,
        // and a capacity that is not a power of two keeps its smallest blocks at the top.
        let (mut class, offset) = (want..self.free.len())
            .filter_map(|class| Some((class, *self.free[class].first()?)))
            .min_by_key(|&(_, offset)| offset)?;
        self.free[class].remove(&offset);
        while class > want {
            class -= 1;
            self.free[class].insert(offset + (1 << class));
        }
        self.held += 1 << want;
        Some(Block {
            offset,
            size: 1 << want,
        })
    }

    pub fn free(&mut self, block: Block) {
        if block.size == 0 {
            return;
        }
        self.held -= block.size;
        let mut class = class_of(block.size);
        let mut offset = block.offset;
        while class + 1 < self.free.len() {
            let buddy = offset ^ (1 << class);
            let merged = offset.min(buddy);
            if merged + (2 << class) > self.capacity || !self.free[class].remove(&buddy) {
                break;
            }
            offset = merged;
            class += 1;
        }
        self.free[class].insert(offset);
    }

    pub fn held(&self) -> usize {
        self.held
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

fn class_of(units: usize) -> usize {
    units.next_power_of_two().trailing_zeros() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &Block, b: &Block) -> bool {
        a.offset < b.offset + b.size && b.offset < a.offset + a.size
    }

    fn assert_disjoint(arena: &Arena, live: &[Block]) {
        for (index, block) in live.iter().enumerate() {
            assert!(block.offset + block.size <= arena.capacity());
            for other in &live[index + 1..] {
                assert!(!overlap(block, other), "{block:?} overlaps {other:?}");
            }
        }
    }

    #[test]
    fn allocation_rounds_up_packs_low_and_never_overlaps() {
        let mut uneven = Arena::new(1000);
        let ends: Vec<usize> = (0..8)
            .map(|_| {
                let block = uneven.alloc(1).unwrap();
                block.offset + block.size
            })
            .collect();
        assert_eq!(ends, (1..=8).collect::<Vec<_>>());

        let mut arena = Arena::new(4096);
        let live: Vec<Block> = [700usize, 3, 200, 64, 1, 33, 129, 500]
            .iter()
            .map(|units| arena.alloc(*units).unwrap())
            .collect();
        assert_eq!(live[5].size, 64, "33 units round up to a class of 64");
        assert_eq!(
            arena.held(),
            live.iter().map(|block| block.size).sum::<usize>(),
            "the arena is charged for the rounding"
        );
        assert_disjoint(&arena, &live);

        let mut twelve = Arena::new(12);
        let live: Vec<Block> = (0..12).map(|_| twelve.alloc(1).unwrap()).collect();
        assert_eq!(twelve.held(), 12);
        assert!(twelve.alloc(1).is_none());
        assert_disjoint(&twelve, &live);
        for block in live {
            twelve.free(block);
        }
        assert_eq!(
            twelve.alloc(8).unwrap().size,
            8,
            "the larger carve is whole again"
        );
    }

    #[test]
    fn freed_room_is_reused_and_coalesces() {
        let mut arena = Arena::new(1024);
        let first = arena.alloc(100).unwrap();
        let second = arena.alloc(100).unwrap();
        arena.free(first);
        assert_eq!(
            arena.held(),
            second.size,
            "only the block still out is charged"
        );
        let third = arena.alloc(100).unwrap();
        assert_eq!(
            third.offset, first.offset,
            "the freed block, not a fresh one"
        );
        arena.free(second);
        arena.free(third);

        let small: Vec<Block> = (0..16).map(|_| arena.alloc(64).unwrap()).collect();
        assert_eq!(arena.held(), 1024, "the arena is full");
        assert!(arena.alloc(1).is_none());
        for block in small {
            arena.free(block);
        }
        assert_eq!(arena.held(), 0);

        let empty = arena.alloc(0).unwrap();
        arena.free(empty);
        assert_eq!(arena.held(), 0, "an empty request costs nothing");

        let whole = arena.alloc(1024).unwrap();
        assert_eq!(
            (whole.offset, whole.size),
            (0, 1024),
            "the arena came back in one piece"
        );
    }
}
