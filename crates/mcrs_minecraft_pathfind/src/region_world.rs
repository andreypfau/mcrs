use crate::{GroundEvaluator, SearchRegion, SearchScratch};
use mcrs_minecraft_physics::{
    Aabb, CollisionSection, CollisionWorld, NoBodies, SectionLookup, SectionMasks, View,
    WorldBorder,
};
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};

struct RegionWorld<'a, 'w, W: CollisionWorld + 'w> {
    world: &'w W,
    region: &'a SearchRegion,
    scratch: &'a SearchScratch<W::Section<'w>>,
}
enum SectionValue<'a, S> {
    Borrowed(&'a S),
    Owned(S),
}
struct RegionSection<'a, S> {
    value: SectionValue<'a, S>,
    at: SectionPos,
    region: &'a SearchRegion,
}
impl<S: CollisionSection> CollisionSection for RegionSection<'_, S> {
    fn masks(&self) -> Option<&SectionMasks> {
        match &self.value {
            SectionValue::Borrowed(section) => section.masks(),
            SectionValue::Owned(section) => section.masks(),
        }
    }
    fn state(&self, at: LocalPos) -> BlockStateId {
        let position =
            self.at
                .min_block()
                .offset(i32::from(at.x()), i32::from(at.y()), i32::from(at.z()));
        if !self.region.contains(position) {
            return BlockStateId::AIR;
        }
        match &self.value {
            SectionValue::Borrowed(section) => section.state(at),
            SectionValue::Owned(section) => section.state(at),
        }
    }
}
impl<'w, W: CollisionWorld> CollisionWorld for RegionWorld<'_, 'w, W> {
    type Section<'a>
        = RegionSection<'a, W::Section<'w>>
    where
        Self: 'a;
    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        if !self.region.contains_section(at) {
            return SectionLookup::Unloaded;
        }
        let cached = self
            .scratch
            .paths
            .sections
            .iter()
            .flatten()
            .find(|(pos, _)| *pos == at);
        let value = match cached {
            Some((_, SectionLookup::Loaded(section))) => SectionValue::Borrowed(section),
            Some(_) => return SectionLookup::Unloaded,
            None => match self.world.section(at) {
                SectionLookup::Loaded(section) => SectionValue::Owned(section),
                _ => return SectionLookup::Unloaded,
            },
        };
        SectionLookup::Loaded(RegionSection {
            value,
            at,
            region: self.region,
        })
    }
    fn border(&self) -> Option<&WorldBorder> {
        self.world.border()
    }
}
impl<W: CollisionWorld> GroundEvaluator<'_, '_, W> {
    pub(crate) fn collides(&mut self, bounds: Aabb) -> bool {
        let key = [
            bounds.min.x.to_bits(),
            bounds.min.y.to_bits(),
            bounds.min.z.to_bits(),
            bounds.max.x.to_bits(),
            bounds.max.y.to_bits(),
            bounds.max.z.to_bits(),
        ];
        if let Some(&hit) = self.scratch.collisions.get(&key) {
            return hit;
        }
        let low = BlockPos::from(bounds.min - glam::DVec3::splat(1.000_000_1));
        let high = BlockPos::from(bounds.max + glam::DVec3::splat(1.000_000_1));
        for x in (low.x >> 4)..=(high.x >> 4) {
            for y in (low.y >> 4)..=(high.y >> 4) {
                for z in (low.z >> 4)..=(high.z >> 4) {
                    self.queries().load_section(SectionPos::new(x, y, z));
                }
            }
        }
        let mut entities = core::mem::take(&mut self.scratch.entities);
        let world = RegionWorld {
            world: self.world,
            region: self.region,
            scratch: self.scratch,
        };
        let hit = mcrs_minecraft_physics::intersects(
            View::new(&world, &NoBodies, self.tables),
            &self.mob.entity,
            bounds,
            &mut entities,
        );
        self.scratch.entities = entities;
        self.scratch.collisions.insert(key, hit);
        hit
    }
}
