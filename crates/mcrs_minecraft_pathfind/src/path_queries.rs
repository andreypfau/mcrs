use crate::path_type_table::PathState;
use crate::{PathInputs, PathScratch, PathTypeTable, SearchRegion};
use mcrs_minecraft_physics::{CollisionSection, CollisionWorld, SectionLookup};
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};

pub struct PathQueries<'a, 'w, W: CollisionWorld + 'w> {
    pub(crate) world: &'w W,
    pub(crate) types: &'a PathTypeTable,
    pub(crate) mob: &'a PathInputs,
    pub(crate) region: &'a SearchRegion,
    pub(crate) scratch: &'a mut PathScratch<W::Section<'w>>,
}

impl<'a, 'w, W: CollisionWorld> PathQueries<'a, 'w, W> {
    pub fn new(
        world: &'w W,
        types: &'a PathTypeTable,
        mob: &'a PathInputs,
        region: &'a SearchRegion,
        scratch: &'a mut PathScratch<W::Section<'w>>,
    ) -> Option<Self> {
        if !mob.width.is_finite()
            || mob.width <= 0.0
            || !mob.height.is_finite()
            || mob.height <= 0.0
            || mob.width + 1.0 >= i32::MAX as f32
            || mob.height + 1.0 >= i32::MAX as f32
            || region.min_y > region.max_y
        {
            return None;
        }
        scratch.clear();
        Some(Self {
            world,
            types,
            mob,
            region,
            scratch,
        })
    }

    pub(crate) fn load_section(&mut self, at: SectionPos) -> usize {
        if let Some(slot) = self
            .scratch
            .sections
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(pos, _)| *pos == at))
        {
            return slot;
        }
        let slot = self.scratch.next_section;
        self.scratch.next_section = (slot + 1) % 64;
        if let Some(entry) = self.scratch.sections.get_mut(slot) {
            *entry = Some((at, self.world.section(at)));
        }
        slot
    }

    pub(crate) fn state(&mut self, at: BlockPos) -> PathState {
        if !self.region.contains(at) {
            return PathState::AIR;
        }
        let slot = self.load_section(SectionPos::from(at));
        match self.scratch.sections.get(slot) {
            Some(Some((_, SectionLookup::Loaded(section)))) => {
                self.types.state(section.state(LocalPos::from(at)))
            }
            _ => PathState::AIR,
        }
    }
}
