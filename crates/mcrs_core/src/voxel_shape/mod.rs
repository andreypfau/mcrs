//! `VoxelShape` and `ShapeRegistry` — the v1 geometric primitive consumed by
//! the block light table and by vanilla block retrofits.
//!
//! Why this lives in `mcrs_core` rather than `mcrs_minecraft_lighting` or `mcrs_vanilla`:
//! both downstream crates need to reference `&'static VoxelShape`, so the type
//! must live in the upstream-most crate to preserve the workspace dep arrow.
//!
//! v1 exposes only the surface the lighting BFS needs:
//! `empty`, `block`, `is_empty`, `occludes_full_block`, `face_shape`,
//! `face_occludes`. v2 operations (collision sweep, raycast/clip, mesh iter)
//! are out of scope.

pub mod block;
pub mod discrete;
pub mod empty;

use bevy_math::Vec3;

use self::block::block_shape;
use self::discrete::DiscreteShape;
use self::empty::empty_shape;

/// Six cardinal axis-directions used for face projection.
///
/// A separate `Direction` lives in `mcrs_minecraft::direction` for game-axis
/// semantics; pulling that into `mcrs_core` would invert the workspace dep
/// arrow, so the lighting/shape geometry uses this independent copy. A `From`
/// impl bridging the two can land later if needed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    #[inline]
    pub const fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::North => 2,
            Direction::South => 3,
            Direction::West => 4,
            Direction::East => 5,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// Internal representation of a `VoxelShape`. v1 only ever stores the
/// `Empty` / `Block` variants in practice; `SingleAabb` and `Discrete`
/// exist so the conditional-shape slow path can land later without
/// rewriting the repr.
#[derive(Debug)]
pub enum ShapeRepr {
    Empty,
    Block,
    SingleAabb(Aabb),
    Discrete(DiscreteShape),
}

/// Opaque voxel shape. Construction-time caches (`face_cache`,
/// `occludes_full_block`, `bounds`) are populated up front — no interior
/// mutability, per the project's concurrency convention.
#[derive(Debug)]
pub struct VoxelShape {
    pub repr: ShapeRepr,
    pub bounds: Aabb,
    pub occludes_full_block: bool,
    pub face_cache: [&'static VoxelShape; 6],
}

impl VoxelShape {
    #[inline]
    pub fn empty() -> &'static VoxelShape {
        empty_shape()
    }

    #[inline]
    pub fn block() -> &'static VoxelShape {
        block_shape()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self.repr, ShapeRepr::Empty)
    }

    #[inline]
    pub fn occludes_full_block(&self) -> bool {
        self.occludes_full_block
    }

    #[inline]
    pub fn face_shape(&self, dir: Direction) -> &'static VoxelShape {
        self.face_cache[dir.index()]
    }

    /// Analog of vanilla `Shapes.faceShapeOccludes(a, b)`: returns true when
    /// `self`'s face on `dir` merged with `other`'s face on the opposite
    /// direction covers the entire unit face.
    ///
    /// v1 coverage matrix (the only cases the lighting BFS exercises while
    /// the conditionally-opaque flag is false):
    /// - (Block, Block, _) → true
    /// - (Empty, _, _) or (_, Empty, _) → false
    /// - (Block, _, _) where the other side is a full-face shape → true
    /// - all other combinations → false until the discrete merge lands
    pub fn face_occludes(&self, other: &VoxelShape, _dir: Direction) -> bool {
        match (&self.repr, &other.repr) {
            (ShapeRepr::Empty, _) | (_, ShapeRepr::Empty) => false,
            (ShapeRepr::Block, ShapeRepr::Block) => true,
            (ShapeRepr::Block, _) | (_, ShapeRepr::Block) => {
                // Conservative: if either side is a full unit cube, the face
                // is fully covered regardless of what the other side projects.
                true
            }
            // TODO: real bitset-discrete face merge lands with the
            // conditional-shape slow path covering slabs/stairs/walls.
            _ => false,
        }
    }
}

/// Pool of `&'static VoxelShape` references produced by freeze-time interning.
/// Indices 0 and 1 are reserved for the `Empty` and `Block` singletons.
///
/// `intern` leaks owned shapes via `Box::leak`. The bound is ~30 unique shapes
/// across the full vanilla retrofit (~6 KB total leak), all paid once at
/// freeze time, so the leak is acceptable per RESEARCH "Don't Hand-Roll".
#[derive(Default)]
pub struct ShapeRegistry {
    entries: Vec<&'static VoxelShape>,
}

impl ShapeRegistry {
    pub fn new() -> Self {
        Self {
            entries: vec![VoxelShape::empty(), VoxelShape::block()],
        }
    }

    pub fn intern(&mut self, shape: VoxelShape) -> &'static VoxelShape {
        if let Some(existing) = self.entries.iter().find(|entry| shapes_equal(entry, &shape)) {
            return *existing;
        }
        let leaked: &'static VoxelShape = Box::leak(Box::new(shape));
        self.entries.push(leaked);
        leaked
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no user-interned shapes have been added. The two reserved
    /// entries `Empty` and `Block` preloaded by `new` do not count.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.len() <= 2
    }

    #[inline]
    pub fn entries(&self) -> &[&'static VoxelShape] {
        &self.entries
    }
}

fn shapes_equal(a: &VoxelShape, b: &VoxelShape) -> bool {
    if a.bounds != b.bounds || a.occludes_full_block != b.occludes_full_block {
        return false;
    }
    match (&a.repr, &b.repr) {
        (ShapeRepr::Empty, ShapeRepr::Empty) => true,
        (ShapeRepr::Block, ShapeRepr::Block) => true,
        (ShapeRepr::SingleAabb(la), ShapeRepr::SingleAabb(rb)) => la == rb,
        (ShapeRepr::Discrete(la), ShapeRepr::Discrete(rb)) => {
            la.bounds == rb.bounds && la.resolution == rb.resolution && la.bits == rb.bits
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_pointer_stable_static() {
        let a = VoxelShape::empty();
        let b = VoxelShape::empty();
        assert!(std::ptr::eq(a, b));
        assert!(a.is_empty());
        assert!(!a.occludes_full_block());
    }

    #[test]
    fn block_returns_pointer_stable_static() {
        let a = VoxelShape::block();
        let b = VoxelShape::block();
        assert!(std::ptr::eq(a, b));
        assert!(!a.is_empty());
        assert!(a.occludes_full_block());
    }

    #[test]
    fn block_face_shape_is_self_for_all_six_faces() {
        let b = VoxelShape::block();
        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            assert!(std::ptr::eq(b.face_shape(dir), b), "face_shape({:?}) != block()", dir);
        }
    }

    #[test]
    fn empty_face_shape_is_self_for_all_six_faces() {
        let e = VoxelShape::empty();
        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            assert!(std::ptr::eq(e.face_shape(dir), e));
        }
    }

    #[test]
    fn face_occludes_block_block_returns_true() {
        let b = VoxelShape::block();
        assert!(b.face_occludes(b, Direction::Up));
        assert!(b.face_occludes(b, Direction::Down));
        assert!(b.face_occludes(b, Direction::North));
    }

    #[test]
    fn face_occludes_block_empty_returns_false() {
        let b = VoxelShape::block();
        let e = VoxelShape::empty();
        assert!(!b.face_occludes(e, Direction::Up));
        assert!(!e.face_occludes(b, Direction::Up));
        assert!(!e.face_occludes(e, Direction::Up));
    }

    #[test]
    fn shape_registry_new_reserves_empty_and_block() {
        let reg = ShapeRegistry::new();
        assert_eq!(reg.len(), 2);
        assert!(std::ptr::eq(reg.entries()[0], VoxelShape::empty()));
        assert!(std::ptr::eq(reg.entries()[1], VoxelShape::block()));
    }

    #[test]
    fn direction_opposite_is_involutive() {
        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            assert_eq!(dir.opposite().opposite(), dir);
        }
    }

    #[test]
    fn singletons_referenced_through_static_globals_are_pointer_equal_to_accessors() {
        assert!(std::ptr::eq(&super::empty::EMPTY, VoxelShape::empty()));
        assert!(std::ptr::eq(&super::block::BLOCK, VoxelShape::block()));
    }
}
