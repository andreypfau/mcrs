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

const WINDOW_SECTIONS: usize = 8;

pub(crate) struct SectionCache<'a, W: CollisionWorld + 'a> {
    base: &'a W,
    low: IVec3,
    high: IVec3,
    sections: [SectionLookup<W::Section<'a>>; WINDOW_SECTIONS],
}

pub(crate) enum CachedSection<'s, S> {
    Cache(&'s S),
    Base(S),
}

impl<'a, W: CollisionWorld> SectionCache<'a, W> {
    pub(crate) fn cover(base: &'a W, position: DVec3) -> Self {
        let low = SectionPos::from(position - DVec3::new(2.0, 4.0, 2.0)).0;
        let high = SectionPos::from(position + DVec3::new(2.0, 4.0, 2.0))
            .0
            .min(low.saturating_add(IVec3::ONE));
        let mut cache = Self {
            base,
            low,
            high,
            sections: [const { SectionLookup::Unloaded }; WINDOW_SECTIONS],
        };
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                for x in low.x..=high.x {
                    let at = IVec3::new(x, y, z);
                    let Some(index) = cache.index(at) else {
                        continue;
                    };
                    if let Some(slot) = cache.sections.get_mut(index) {
                        *slot = base.section(SectionPos(at));
                    }
                }
            }
        }
        cache
    }

    fn index(&self, at: IVec3) -> Option<usize> {
        if !(at.cmpge(self.low).all() && at.cmple(self.high).all()) {
            return None;
        }
        let offset = (at - self.low).as_uvec3();
        usize::try_from(offset.x + 2 * offset.z + 4 * offset.y).ok()
    }
}

impl<'a, W: CollisionWorld> CollisionWorld for SectionCache<'a, W> {
    type Section<'s>
        = CachedSection<'s, W::Section<'a>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        let cached = self.index(at.0).and_then(|index| self.sections.get(index));
        match cached {
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
