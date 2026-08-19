//! Suballocation of the geometry buffers.
//!
//! A buffer cannot be resized, so the three arenas are created at their full size once and handed
//! out in blocks from here. Blocks come in size classes a power of two apart, which is what makes
//! a block's buddy its own offset with one bit flipped: a request splits the smallest free block
//! that will hold it and keeps splitting down, and giving one back joins it with its buddy again
//! while that buddy is free. Nothing is ever moved, there is no compacting pass, and free room
//! never ends up stranded in the wrong size.
//!
//! The price is up to twice the room a block really needs, which the report prints so it can be
//! seen rather than guessed at.
//!
//! Units are whatever the arena stores — greedy quads, model quads, culling groups — so the byte
//! size of one never appears here.

use std::collections::HashSet;

/// One handed-out run of an arena.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub offset: usize,
    /// What the block occupies, which is its size class rather than what was asked for.
    size: usize,
    /// What was asked for, so what is really in the arena can be told from what rounding costs.
    asked: usize,
}

impl Block {
    /// A stream a region holds nothing in still has a block; it just takes no room.
    pub const EMPTY: Self = Self {
        offset: 0,
        size: 0,
        asked: 0,
    };
}

pub struct Arena {
    capacity: usize,
    /// Free blocks by size class. A set rather than a list because giving a block back asks
    /// whether one particular offset — its buddy — is free.
    free: Vec<HashSet<usize>>,
    held: usize,
    asked: usize,
}

impl Arena {
    pub fn new(capacity: usize) -> Self {
        let classes = class_of(capacity.max(1)) + 1;
        let mut free: Vec<HashSet<usize>> = (0..classes).map(|_| HashSet::new()).collect();
        // Carved into the blocks the capacity's binary representation names, largest first. Every
        // block then starts at a multiple of its own size, which is what the buddy arithmetic
        // needs, and an arena that is not a power of two costs nothing to allow.
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
            asked: 0,
        }
    }

    /// A run of `units`, or `None` when no free block is large enough.
    pub fn alloc(&mut self, units: usize) -> Option<Block> {
        if units == 0 {
            return Some(Block::EMPTY);
        }
        let want = class_of(units);
        let mut class = (want..self.free.len()).find(|class| !self.free[*class].is_empty())?;
        let offset = *self.free[class].iter().next().expect("the class is not empty");
        self.free[class].remove(&offset);
        // Split down to the class asked for, keeping the lower half and freeing the upper one.
        while class > want {
            class -= 1;
            self.free[class].insert(offset + (1 << class));
        }
        self.held += 1 << want;
        self.asked += units;
        Some(Block {
            offset,
            size: 1 << want,
            asked: units,
        })
    }

    pub fn free(&mut self, block: Block) {
        if block.size == 0 {
            return;
        }
        self.held -= block.size;
        self.asked -= block.asked;
        let mut class = class_of(block.size);
        let mut offset = block.offset;
        while class + 1 < self.free.len() {
            let buddy = offset ^ (1 << class);
            let merged = offset.min(buddy);
            // The bound is what keeps two blocks from different carves apart: the pair only counts
            // as one block of the arena when it lies inside it whole.
            if merged + (2 << class) > self.capacity || !self.free[class].remove(&buddy) {
                break;
            }
            offset = merged;
            class += 1;
        }
        self.free[class].insert(offset);
    }

    /// Units the arena has handed out, rounding included. This is what decides whether the next
    /// region fits.
    pub fn held(&self) -> usize {
        self.held
    }

    /// Units really in use, so the cost of rounding is the difference between the two.
    pub fn asked(&self) -> usize {
        self.asked
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// The class a request of this many units lands in, which is also the log of the block size.
fn class_of(units: usize) -> usize {
    units.next_power_of_two().trailing_zeros() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &Block, b: &Block) -> bool {
        a.offset < b.offset + b.size && b.offset < a.offset + a.size
    }

    #[test]
    fn a_request_takes_the_class_above_it() {
        let mut arena = Arena::new(1024);
        let block = arena.alloc(33).unwrap();
        assert_eq!(block.size, 64, "33 units round up to a class of 64");
        assert_eq!(arena.asked(), 33);
        assert_eq!(arena.held(), 64, "and the arena is charged for the rounding");
    }

    #[test]
    fn no_two_live_blocks_overlap() {
        let mut arena = Arena::new(4096);
        let live: Vec<Block> = [700usize, 3, 200, 64, 1, 33, 129, 500]
            .iter()
            .map(|units| arena.alloc(*units).unwrap())
            .collect();
        for (index, block) in live.iter().enumerate() {
            assert!(block.offset + block.size <= arena.capacity());
            for other in &live[index + 1..] {
                assert!(!overlap(block, other), "{block:?} overlaps {other:?}");
            }
        }
    }

    /// The whole point of giving a block back: the room returns and the next region of the same
    /// shape lands in it rather than at the far end of an arena that only ever grows.
    #[test]
    fn a_freed_block_is_handed_out_again() {
        let mut arena = Arena::new(1024);
        let first = arena.alloc(100).unwrap();
        let second = arena.alloc(100).unwrap();
        assert!(!overlap(&first, &second));

        arena.free(first);
        assert_eq!(arena.held(), second.size, "only the block still out is charged");
        let third = arena.alloc(100).unwrap();
        assert_eq!(third.offset, first.offset, "the freed block, not a fresh one");
        assert_eq!(arena.held(), second.size + third.size);
    }

    /// What splitting and merging are for. Room freed by many small blocks has to serve one large
    /// request afterwards, or a view that reshapes what it holds runs the arena into the ground
    /// while most of it stands empty.
    #[test]
    fn room_freed_in_small_blocks_serves_a_large_request() {
        let mut arena = Arena::new(1024);
        let small: Vec<Block> = (0..16).map(|_| arena.alloc(64).unwrap()).collect();
        assert_eq!(arena.held(), 1024, "the arena is full");
        assert!(arena.alloc(1).is_none());

        for block in small {
            arena.free(block);
        }
        assert_eq!(arena.held(), 0);
        let whole = arena.alloc(1024).unwrap();
        assert_eq!((whole.offset, whole.size), (0, 1024), "the arena came back in one piece");
    }

    #[test]
    fn an_arena_with_no_room_refuses_rather_than_overlapping() {
        let mut arena = Arena::new(256);
        let first = arena.alloc(200).unwrap();
        assert_eq!(first.size, 256, "200 rounds up to the whole arena");
        assert!(arena.alloc(1).is_none());
        arena.free(first);
        assert!(arena.alloc(1).is_some(), "and the room comes back");
    }

    /// An arena whose size is not a power of two is carved into several, and a block may not merge
    /// with a buddy that belongs to a different carve.
    #[test]
    fn an_arena_that_is_not_a_power_of_two_hands_out_all_of_itself() {
        let mut arena = Arena::new(12);
        let live: Vec<Block> = (0..12).map(|_| arena.alloc(1).unwrap()).collect();
        assert_eq!(arena.held(), 12);
        assert!(arena.alloc(1).is_none());
        for (index, block) in live.iter().enumerate() {
            assert!(block.offset < 12);
            for other in &live[index + 1..] {
                assert!(!overlap(block, other));
            }
        }
        for block in live {
            arena.free(block);
        }
        assert_eq!(arena.alloc(8).unwrap().size, 8, "the larger carve is whole again");
    }

    /// A stream a region holds nothing in still asks for a block, and must not be charged for one
    /// or handed an offset anything else could be using.
    #[test]
    fn an_empty_request_costs_nothing() {
        let mut arena = Arena::new(64);
        let empty = arena.alloc(0).unwrap();
        assert_eq!(arena.held(), 0);
        arena.free(empty);
        assert_eq!(arena.held(), 0);
        assert_eq!(arena.alloc(64).unwrap().offset, 0, "the arena is untouched");
    }
}
