use glam::DVec3;
use glam::IVec3;
use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;

/// A cache that grows past this many sections starts again, so its size has a bound.
const CACHE_SECTIONS: usize = 512;

/// The sections that the entities of one batch of the pass read, each looked up once per batch.
/// A section out of the cache comes from the base world.
pub(crate) struct SectionCache<'a, W: CollisionWorld + 'a> {
    base: &'a W,
    sections: FxHashMap<IVec3, SectionLookup<W::Section<'a>>>,
}

pub(crate) enum CachedSection<'s, S> {
    Cache(&'s S),
    Base(S),
}

impl<'a, W: CollisionWorld> SectionCache<'a, W> {
    pub(crate) fn new(base: &'a W) -> Self {
        Self {
            base,
            sections: FxHashMap::default(),
        }
    }

    /// Adds the sections from 2 blocks around `position` on `x` and `z` and 4 blocks on `y`, the
    /// window that the rows pass reads around the rows of a tile.
    pub(crate) fn cover(&mut self, position: DVec3) {
        if self.sections.len() > CACHE_SECTIONS {
            self.sections.clear();
        }
        let low = SectionPos::from(position - DVec3::new(2.0, 4.0, 2.0)).0;
        let high = SectionPos::from(position + DVec3::new(2.0, 4.0, 2.0)).0;
        let base = self.base;
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                for x in low.x..=high.x {
                    let at = IVec3::new(x, y, z);
                    self.sections
                        .entry(at)
                        .or_insert_with(|| base.section(SectionPos(at)));
                }
            }
        }
    }
}

impl<'a, W: CollisionWorld> CollisionWorld for SectionCache<'a, W> {
    type Section<'s>
        = CachedSection<'s, W::Section<'a>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.sections.get(&at.0) {
            Some(SectionLookup::Loaded(section)) => {
                SectionLookup::Loaded(CachedSection::Cache(section))
            }
            Some(SectionLookup::Outside) => SectionLookup::Outside,
            Some(SectionLookup::Unloaded) => SectionLookup::Unloaded,
            None => match self.base.section(at) {
                SectionLookup::Loaded(section) => {
                    SectionLookup::Loaded(CachedSection::Base(section))
                }
                SectionLookup::Outside => SectionLookup::Outside,
                SectionLookup::Unloaded => SectionLookup::Unloaded,
            },
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for CachedSection<'_, S> {
    fn masks(&self) -> Option<&SectionMasks> {
        match self {
            Self::Cache(section) => section.masks(),
            Self::Base(section) => section.masks(),
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        match self {
            Self::Cache(section) => section.state(at),
            Self::Base(section) => section.state(at),
        }
    }
}
