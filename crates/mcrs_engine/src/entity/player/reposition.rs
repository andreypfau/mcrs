use bevy_ecs::prelude::{Component, Resource};
use bevy_math::*;

use crate::world::block::BlockPos;
use crate::world::chunk;

/// Client-coordinate repositioning (Spout-style).
///
/// Semantics:
/// - "convert_*" maps **server/world** coordinates -> **client** coordinates by applying `offset_blocks`.
/// - "unconvert_*" maps **client** coordinates -> **server/world** coordinates by removing `offset_blocks`.
///
/// `offset_blocks` is expressed in **blocks** (not chunks). Vertical repositioning works by shifting
/// the client window while keeping the server world unbounded.
#[derive(Copy, Clone, Debug, Default, Component, Reflect)]
pub struct Reposition {
    offset_blocks: IVec3,
}

impl Reposition {
    #[inline]
    pub fn offset_blocks(&self) -> IVec3 {
        self.offset_blocks
    }

    #[inline]
    pub fn set_offset_blocks(&mut self, offset_blocks: IVec3) {
        self.offset_blocks = offset_blocks;
    }

    #[inline]
    pub fn offset_y_blocks(&self) -> i32 {
        self.offset_blocks.y
    }

    #[inline]
    pub fn set_offset_y_blocks(&mut self, y: i32) {
        self.offset_blocks.y = y;
    }

    #[inline]
    pub fn convert_x(&self, x: f64) -> f64 {
        x + self.offset_blocks.x as f64
    }

    #[inline]
    pub fn convert_y(&self, y: f64) -> f64 {
        y + self.offset_blocks.y as f64
    }

    #[inline]
    pub fn convert_z(&self, z: f64) -> f64 {
        z + self.offset_blocks.z as f64
    }

    #[inline]
    pub fn unconvert_x(&self, x: f64) -> f64 {
        x - self.offset_blocks.x as f64
    }

    #[inline]
    pub fn unconvert_y(&self, y: f64) -> f64 {
        y - self.offset_blocks.y as f64
    }

    #[inline]
    pub fn unconvert_z(&self, z: f64) -> f64 {
        z - self.offset_blocks.z as f64
    }

    #[inline]
    pub fn convert_dvec3(&self, pos: DVec3) -> DVec3 {
        pos + DVec3::new(
            self.offset_blocks.x as f64,
            self.offset_blocks.y as f64,
            self.offset_blocks.z as f64,
        )
    }

    #[inline]
    pub fn unconvert_dvec3(&self, pos: DVec3) -> DVec3 {
        pos - DVec3::new(
            self.offset_blocks.x as f64,
            self.offset_blocks.y as f64,
            self.offset_blocks.z as f64,
        )
    }

    #[inline]
    pub fn convert_block_pos(&self, pos: BlockPos) -> BlockPos {
        BlockPos::new(
            pos.x + self.offset_blocks.x,
            pos.y + self.offset_blocks.y,
            pos.z + self.offset_blocks.z,
        )
    }

    #[inline]
    pub fn unconvert_block_pos(&self, pos: BlockPos) -> BlockPos {
        BlockPos::new(
            pos.x - self.offset_blocks.x,
            pos.y - self.offset_blocks.y,
            pos.z - self.offset_blocks.z,
        )
    }

    // --- Chunk-coordinate conversions (ChunkPos uses chunk indices, not block coords)

    #[inline]
    pub fn convert_chunk_x(&self, x: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((x as i64) << bits) + (self.offset_blocks.x as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn convert_chunk_y(&self, y: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((y as i64) << bits) + (self.offset_blocks.y as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn convert_chunk_z(&self, z: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((z as i64) << bits) + (self.offset_blocks.z as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn unconvert_chunk_x(&self, x: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((x as i64) << bits) - (self.offset_blocks.x as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn unconvert_chunk_y(&self, y: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((y as i64) << bits) - (self.offset_blocks.y as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn unconvert_chunk_z(&self, z: i32) -> i32 {
        let bits = chunk::BLOCKS::BITS as i64;
        let v = ((z as i64) << bits) - (self.offset_blocks.z as i64);
        (v >> bits) as i32
    }

    #[inline]
    pub fn convert_chunk_pos(&self, pos: chunk::ChunkPos) -> chunk::ChunkPos {
        chunk::ChunkPos::new(
            self.convert_chunk_x(pos.x),
            self.convert_chunk_y(pos.y),
            self.convert_chunk_z(pos.z),
        )
    }

    #[inline]
    pub fn unconvert_chunk_pos(&self, pos: chunk::ChunkPos) -> chunk::ChunkPos {
        chunk::ChunkPos::new(
            self.unconvert_chunk_x(pos.x),
            self.unconvert_chunk_y(pos.y),
            self.unconvert_chunk_z(pos.z),
        )
    }

    /// Spout-style vertical windowing:
    /// If `convert_y(y)` escapes [min_y, max_y), adjust offset in `step_y` blocks to bring it back.
    ///
    /// Returns true if `offset_blocks` changed.
    pub fn ensure_visible_y_window(
        &mut self,
        y_blocks: i32,
        min_y: i32,
        max_y: i32,
        step_y: i32,
    ) -> bool {
        // client Y for current player position
        let mut c_y = y_blocks + self.offset_blocks.y;

        if c_y >= max_y || c_y < min_y {
            let mid = (max_y + min_y) >> 1;
            let steps = (c_y - mid) / step_y;

            self.offset_blocks.y -= steps * step_y;
            c_y = y_blocks + self.offset_blocks.y;

            if c_y >= max_y {
                self.offset_blocks.y -= step_y;
            } else if c_y < min_y {
                self.offset_blocks.y += step_y;
            }
            return true;
        }

        false
    }
}

#[derive(Copy, Clone, Debug, Resource)]
pub struct RepositionConfig {
    pub min_y: i32,
    pub max_y: i32,
    pub step_y: i32,
}

impl Default for RepositionConfig {
    fn default() -> Self {
        Self {
            min_y: 0,
            max_y: 256,
            step_y: 160,
        }
    }
}
