use crate::PathQueries;
use mcrs_minecraft_physics::{CollisionWorld, PathType};
use mcrs_voxel::BlockPos;

impl<W: CollisionWorld> PathQueries<'_, '_, W> {
    pub fn block_path_type(&mut self, at: BlockPos) -> PathType {
        if let Some(&kind) = self.scratch.block_types.get(&at) {
            return kind;
        }
        let kind = self.compute_block_type(at);
        self.scratch.block_types.insert(at, kind);
        kind
    }
    fn compute_block_type(&mut self, at: BlockPos) -> PathType {
        let kind = self.state(at).kind;
        if kind != PathType::Open || at.y == self.region.min_y {
            return kind;
        }
        if let Some(kind) = PathType::over(self.state(at.below()).kind) {
            return kind;
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if (dx != 0 || dz != 0)
                        && let Some(kind) = self.state(at.offset(dx, dy, dz)).kind.as_neighbour()
                    {
                        return kind;
                    }
                }
            }
        }
        PathType::Walkable
    }
}
