use glam::IVec3;
use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

/// The most sections on `y` that one tile looks up in advance.
const HEIGHT: i32 = 8;

/// The sections around one tile, looked up once for the task of the tile; a section out of the
/// window comes from the base world. The rows of a tile read the same few sections many times.
pub(crate) struct TileWorld<'a, W: CollisionWorld + 'a> {
    base: &'a W,
    min: SectionPos,
    size: [i32; 3],
    sections: Vec<SectionLookup<W::Section<'a>>>,
}

/// A section of a [`TileWorld`].
pub enum TileSection<'s, S> {
    Window(&'s S),
    Base(S),
}

impl<'a, W: CollisionWorld> TileWorld<'a, W> {
    /// The sections from `min` over `size` sections on each axis, at most [`HEIGHT`] on `y`.
    pub fn new(base: &'a W, min: SectionPos, size: [i32; 3]) -> Self {
        let [width, height, depth] = size;
        let size = [width, height.clamp(0, HEIGHT), depth];
        let mut sections = Vec::new();
        for y in 0..height.clamp(0, HEIGHT) {
            for z in 0..depth {
                for x in 0..width {
                    sections.push(base.section(SectionPos(min.0 + IVec3::new(x, y, z))));
                }
            }
        }
        Self {
            base,
            min,
            size,
            sections,
        }
    }

    fn index(&self, at: SectionPos) -> Option<usize> {
        let [x, y, z] = at.0.wrapping_sub(self.min.0).to_array();
        let [width, height, depth] = self.size;
        if !(0..width).contains(&x) || !(0..height).contains(&y) || !(0..depth).contains(&z) {
            return None;
        }
        usize::try_from((y * depth + z) * width + x).ok()
    }
}

impl<'a, W: CollisionWorld> CollisionWorld for TileWorld<'a, W> {
    type Section<'s>
        = TileSection<'s, W::Section<'a>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.index(at).and_then(|index| self.sections.get(index)) {
            Some(SectionLookup::Loaded(section)) => {
                SectionLookup::Loaded(TileSection::Window(section))
            }
            Some(SectionLookup::Outside) => SectionLookup::Outside,
            Some(SectionLookup::Unloaded) => SectionLookup::Unloaded,
            None => match self.base.section(at) {
                SectionLookup::Loaded(section) => SectionLookup::Loaded(TileSection::Base(section)),
                SectionLookup::Outside => SectionLookup::Outside,
                SectionLookup::Unloaded => SectionLookup::Unloaded,
            },
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for TileSection<'_, S> {
    fn masks(&self) -> Option<&SectionMasks> {
        match self {
            Self::Window(section) => section.masks(),
            Self::Base(section) => section.masks(),
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        match self {
            Self::Window(section) => section.state(at),
            Self::Base(section) => section.state(at),
        }
    }
}
