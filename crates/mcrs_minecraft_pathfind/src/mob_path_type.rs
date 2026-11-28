use crate::malus_table::TYPES;
use crate::{MobFlags, PathQueries};
use mcrs_minecraft_physics::{CollisionWorld, PathType};
use mcrs_voxel::BlockPos;

impl<W: CollisionWorld> PathQueries<'_, '_, W> {
    pub fn mob_path_type(&mut self, at: BlockPos) -> PathType {
        if let Some(&kind) = self.scratch.mob_types.get(&at) {
            return kind;
        }
        let kind = self.compute_mob_type(at);
        self.scratch.mob_types.insert(at, kind);
        kind
    }
    fn compute_mob_type(&mut self, at: BlockPos) -> PathType {
        let width = (self.mob.width + 1.0).floor() as i32;
        let height = (self.mob.height + 1.0).floor() as i32;
        let mut set = 0_u32;
        for dx in 0..width {
            for dy in 0..height {
                for dz in 0..width {
                    let mut kind = self.block_path_type(at.offset(dx, dy, dz));
                    if kind == PathType::DoorWoodClosed
                        && self
                            .mob
                            .flags
                            .contains(MobFlags::OPEN_DOORS.union(MobFlags::PASS_DOORS))
                    {
                        kind = PathType::WalkableDoor;
                    }
                    if kind == PathType::DoorOpen && !self.mob.flags.contains(MobFlags::PASS_DOORS)
                    {
                        kind = PathType::Blocked;
                    }
                    if kind == PathType::Rail
                        && self.block_path_type(self.mob.block_position) != PathType::Rail
                        && self.block_path_type(self.mob.block_position.below()) != PathType::Rail
                    {
                        kind = PathType::UnpassableRail;
                    }
                    set |= 1 << kind as u32;
                }
            }
        }
        if set.count_ones() == 1 {
            return TYPES
                .get(set.trailing_zeros() as usize)
                .copied()
                .unwrap_or(PathType::Blocked);
        }
        for kind in [PathType::Fence, PathType::UnpassableRail] {
            if set & (1 << kind as u32) != 0 {
                return kind;
            }
        }
        let mut highest = PathType::Blocked;
        let mut malus = self.mob.malus.get(highest);
        for kind in TYPES {
            if set & (1 << kind as u32) == 0 {
                continue;
            }
            let value = self.mob.malus.get(kind);
            if value < 0.0 {
                return kind;
            }
            if value >= malus {
                highest = kind;
                malus = value;
            }
        }
        let centre = self.block_path_type(at);
        if width > 1
            && self.mob.malus.get(centre) < malus
            && self.mob.malus.get(PathType::BigMobsCloseToDanger) < malus
        {
            return PathType::BigMobsCloseToDanger;
        }
        if width == 1 && centre == PathType::Open && highest != PathType::Open && malus == 0.0 {
            return PathType::Open;
        }
        highest
    }
}
