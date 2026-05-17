//! Internal block-light BFS state.
//!
//! The per-cell queue entry is a packed `u64` built by `pack_bfs_entry`
//! and consumed in place by the BFS bodies. It is intentionally distinct
//! from `components::Wavefront(u32)`, which is the cross-chunk egress
//! representation pushed onto `BlockEgress` at face boundaries. The
//! per-cell entry carries Y plus a 6-bit direction bitset and a 3-bit
//! flag field, none of which `Wavefront` needs.

use mcrs_core::voxel_shape::{Direction, VoxelShape};
use mcrs_minecraft_block::palette::BlockPalette;

use crate::components::{BlockEgress, BlockLightWorkspace, SkyEgress, SkyLightWorkspace, Wavefront};
use crate::geom::chunk_xyz_to_face_cell;
use crate::storage::LightStorage;
use crate::table::{flag_bits, BlockLightTable};

pub(crate) const FLAG_HAS_SIDED_TRANSPARENT_BLOCKS: u8 = 1 << 0;
// Promoted from pub(crate) to pub so external snapshot / property
// harnesses can pack initial BFS entries without reaching into the crate
// via cfg-gated shims. Production behavior is unchanged; this is API
// surface only.
pub const FLAG_RECHECK_LEVEL: u8 = 1 << 1;
pub const FLAG_WRITE_LEVEL: u8 = 1 << 2;
pub const ALL_DIRECTIONS_BITSET: u8 = 0b111111;

#[inline]
pub const fn pack_bfs_entry(
    x: u8,
    z: u8,
    y: u8,
    level: u8,
    direction_bitset: u8,
    flags: u8,
) -> u64 {
    debug_assert!(x < 16);
    debug_assert!(z < 16);
    debug_assert!(y < 16);
    debug_assert!(level < 16);
    debug_assert!(direction_bitset < 64);
    debug_assert!(flags < 8);
    (x as u64 & 0xF)
        | ((z as u64 & 0xF) << 4)
        | ((y as u64 & 0xFFF) << 8)
        | ((level as u64 & 0xF) << 20)
        | ((direction_bitset as u64 & 0x3F) << 24)
        | ((flags as u64 & 0x7) << 30)
}

#[inline]
pub(crate) const fn unpack_bfs_entry_x(entry: u64) -> u8 {
    (entry & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_z(entry: u64) -> u8 {
    ((entry >> 4) & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_y(entry: u64) -> u16 {
    ((entry >> 8) & 0xFFF) as u16
}

#[inline]
pub(crate) const fn unpack_bfs_entry_level(entry: u64) -> u8 {
    ((entry >> 20) & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_dir_bitset(entry: u64) -> u8 {
    ((entry >> 24) & 0x3F) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_flags(entry: u64) -> u8 {
    ((entry >> 30) & 0x7) as u8
}

#[inline]
pub(crate) const fn normal_of(d: Direction) -> (i8, i8, i8) {
    match d {
        Direction::Down => (0, -1, 0),
        Direction::Up => (0, 1, 0),
        Direction::North => (0, 0, -1),
        Direction::South => (0, 0, 1),
        Direction::West => (-1, 0, 0),
        Direction::East => (1, 0, 0),
    }
}

const fn build_directions_except_opposite() -> [u8; 6] {
    let all = 0b111111u8;
    let mut t = [0u8; 6];
    let mut i = 0;
    while i < 6 {
        let opp = i ^ 1;
        t[i] = all & !(1u8 << opp);
        i += 1;
    }
    t
}

pub(crate) const DIRECTIONS_EXCEPT_OPPOSITE: [u8; 6] = build_directions_except_opposite();

static ARR_00: [Direction; 0] = [];
static ARR_01: [Direction; 1] = [Direction::Down];
static ARR_02: [Direction; 1] = [Direction::Up];
static ARR_03: [Direction; 2] = [Direction::Down, Direction::Up];
static ARR_04: [Direction; 1] = [Direction::North];
static ARR_05: [Direction; 2] = [Direction::Down, Direction::North];
static ARR_06: [Direction; 2] = [Direction::Up, Direction::North];
static ARR_07: [Direction; 3] = [Direction::Down, Direction::Up, Direction::North];
static ARR_08: [Direction; 1] = [Direction::South];
static ARR_09: [Direction; 2] = [Direction::Down, Direction::South];
static ARR_10: [Direction; 2] = [Direction::Up, Direction::South];
static ARR_11: [Direction; 3] = [Direction::Down, Direction::Up, Direction::South];
static ARR_12: [Direction; 2] = [Direction::North, Direction::South];
static ARR_13: [Direction; 3] = [Direction::Down, Direction::North, Direction::South];
static ARR_14: [Direction; 3] = [Direction::Up, Direction::North, Direction::South];
static ARR_15: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
];
static ARR_16: [Direction; 1] = [Direction::West];
static ARR_17: [Direction; 2] = [Direction::Down, Direction::West];
static ARR_18: [Direction; 2] = [Direction::Up, Direction::West];
static ARR_19: [Direction; 3] = [Direction::Down, Direction::Up, Direction::West];
static ARR_20: [Direction; 2] = [Direction::North, Direction::West];
static ARR_21: [Direction; 3] = [Direction::Down, Direction::North, Direction::West];
static ARR_22: [Direction; 3] = [Direction::Up, Direction::North, Direction::West];
static ARR_23: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::West,
];
static ARR_24: [Direction; 2] = [Direction::South, Direction::West];
static ARR_25: [Direction; 3] = [Direction::Down, Direction::South, Direction::West];
static ARR_26: [Direction; 3] = [Direction::Up, Direction::South, Direction::West];
static ARR_27: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::West,
];
static ARR_28: [Direction; 3] = [Direction::North, Direction::South, Direction::West];
static ARR_29: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_30: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_31: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_32: [Direction; 1] = [Direction::East];
static ARR_33: [Direction; 2] = [Direction::Down, Direction::East];
static ARR_34: [Direction; 2] = [Direction::Up, Direction::East];
static ARR_35: [Direction; 3] = [Direction::Down, Direction::Up, Direction::East];
static ARR_36: [Direction; 2] = [Direction::North, Direction::East];
static ARR_37: [Direction; 3] = [Direction::Down, Direction::North, Direction::East];
static ARR_38: [Direction; 3] = [Direction::Up, Direction::North, Direction::East];
static ARR_39: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::East,
];
static ARR_40: [Direction; 2] = [Direction::South, Direction::East];
static ARR_41: [Direction; 3] = [Direction::Down, Direction::South, Direction::East];
static ARR_42: [Direction; 3] = [Direction::Up, Direction::South, Direction::East];
static ARR_43: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::East,
];
static ARR_44: [Direction; 3] = [Direction::North, Direction::South, Direction::East];
static ARR_45: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_46: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_47: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_48: [Direction; 2] = [Direction::West, Direction::East];
static ARR_49: [Direction; 3] = [Direction::Down, Direction::West, Direction::East];
static ARR_50: [Direction; 3] = [Direction::Up, Direction::West, Direction::East];
static ARR_51: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::West,
    Direction::East,
];
static ARR_52: [Direction; 3] = [Direction::North, Direction::West, Direction::East];
static ARR_53: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_54: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_55: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_56: [Direction; 3] = [Direction::South, Direction::West, Direction::East];
static ARR_57: [Direction; 4] = [
    Direction::Down,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_58: [Direction; 4] = [
    Direction::Up,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_59: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_60: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_61: [Direction; 5] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_62: [Direction; 5] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_63: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub(crate) const DIRECTIONS_FROM_BITSET: [&[Direction]; 64] = [
    &ARR_00, &ARR_01, &ARR_02, &ARR_03, &ARR_04, &ARR_05, &ARR_06, &ARR_07, &ARR_08, &ARR_09,
    &ARR_10, &ARR_11, &ARR_12, &ARR_13, &ARR_14, &ARR_15, &ARR_16, &ARR_17, &ARR_18, &ARR_19,
    &ARR_20, &ARR_21, &ARR_22, &ARR_23, &ARR_24, &ARR_25, &ARR_26, &ARR_27, &ARR_28, &ARR_29,
    &ARR_30, &ARR_31, &ARR_32, &ARR_33, &ARR_34, &ARR_35, &ARR_36, &ARR_37, &ARR_38, &ARR_39,
    &ARR_40, &ARR_41, &ARR_42, &ARR_43, &ARR_44, &ARR_45, &ARR_46, &ARR_47, &ARR_48, &ARR_49,
    &ARR_50, &ARR_51, &ARR_52, &ARR_53, &ARR_54, &ARR_55, &ARR_56, &ARR_57, &ARR_58, &ARR_59,
    &ARR_60, &ARR_61, &ARR_62, &ARR_63,
];

/// Compile-time wave selector passed to `propagate_core` by the four
/// wrappers. Reusing `FLAG_WRITE_LEVEL` / `FLAG_RECHECK_LEVEL` as the
/// discriminator values keeps the bit pattern unique without introducing
/// a parallel constant namespace; the values intentionally match the
/// per-entry flag bits the wave is most associated with at the source
/// (increase seeds use `WRITE_LEVEL`; decrease cells re-emit via
/// `RECHECK_LEVEL`). The const-generic dispatch produces dead-code
/// elimination of the unused branch in each monomorphization.
pub(crate) const WAVE_INCREASE: u8 = FLAG_WRITE_LEVEL;
pub(crate) const WAVE_DECREASE: u8 = FLAG_RECHECK_LEVEL;

/// Per-channel hook surface for the shared BFS body.
///
/// Monomorphized once per channel marker; every method is annotated
/// `#[inline(always)]` on both impls so LLVM erases the trait boundary
/// during codegen. The two impls live in this module immediately below
/// and the BFS body in `propagate_core` calls into them; no dynamic
/// dispatch is involved.
pub(crate) trait BfsChannel {
    type Workspace;
    type Egress;

    fn increase_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64>;
    fn decrease_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64>;
    fn push_egress(egress: &mut Self::Egress, wavefront: Wavefront);

    /// Increase-wave target level for the neighbour cell in direction `d`.
    /// Block channel returns `propagated_level.saturating_sub(opacity.max(1))`.
    /// Sky channel returns 15 for the Down + level==15 + PROPAGATES_SKYLIGHT_DOWN
    /// vertical free-fall case and the same block formula otherwise.
    fn increase_target_level(
        propagated_level: u8,
        d: Direction,
        opacity: u8,
        dst_flags: u8,
    ) -> u8;

    /// Decrease-wave emission re-emit. Block channel returns
    /// `Some(emission)` for emitter cells encountered en-route so the
    /// increase pass restores their emission via `FLAG_WRITE_LEVEL`. Sky
    /// channel returns `None` unconditionally (no per-cell emission).
    fn emission_for(table: &BlockLightTable, dst_state: mcrs_protocol::BlockStateId) -> Option<u8>;
}

pub(crate) struct BlockBfs;
pub(crate) struct SkyBfs;

impl BfsChannel for BlockBfs {
    type Workspace = BlockLightWorkspace;
    type Egress = BlockEgress;

    #[inline(always)]
    fn increase_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64> {
        &mut workspace.increase_queue
    }

    #[inline(always)]
    fn decrease_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64> {
        &mut workspace.decrease_queue
    }

    #[inline(always)]
    fn push_egress(egress: &mut Self::Egress, wavefront: Wavefront) {
        egress.0.push(wavefront);
    }

    #[inline(always)]
    fn increase_target_level(
        propagated_level: u8,
        _d: Direction,
        opacity: u8,
        _dst_flags: u8,
    ) -> u8 {
        propagated_level.saturating_sub(opacity.max(1))
    }

    #[inline(always)]
    fn emission_for(
        table: &BlockLightTable,
        dst_state: mcrs_protocol::BlockStateId,
    ) -> Option<u8> {
        let emitted = table.emission_for(dst_state);
        if emitted != 0 {
            Some(emitted)
        } else {
            None
        }
    }
}

impl BfsChannel for SkyBfs {
    type Workspace = SkyLightWorkspace;
    type Egress = SkyEgress;

    #[inline(always)]
    fn increase_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64> {
        &mut workspace.increase_queue
    }

    #[inline(always)]
    fn decrease_queue(workspace: &mut Self::Workspace) -> &mut Vec<u64> {
        &mut workspace.decrease_queue
    }

    #[inline(always)]
    fn push_egress(egress: &mut Self::Egress, wavefront: Wavefront) {
        egress.0.push(wavefront);
    }

    #[inline(always)]
    fn increase_target_level(
        propagated_level: u8,
        d: Direction,
        opacity: u8,
        dst_flags: u8,
    ) -> u8 {
        if d == Direction::Down
            && propagated_level == 15
            && (dst_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
        {
            15
        } else {
            propagated_level.saturating_sub(opacity.max(1))
        }
    }

    #[inline(always)]
    fn emission_for(
        _table: &BlockLightTable,
        _dst_state: mcrs_protocol::BlockStateId,
    ) -> Option<u8> {
        None
    }
}

/// Shared per-chunk BFS outer loop. Const-generic `FLAGS` selects the
/// wave (`WAVE_INCREASE` or `WAVE_DECREASE`); the channel `C` selects
/// which queue is drained, where pushes land, how target levels are
/// computed, and whether decrease re-emits emission. Every wrapper passes
/// a literal `FLAGS`, so each branch on `FLAGS` constant-folds and the
/// unused arm is dead-code-eliminated per monomorphization.
///
/// Slow-path branch fires when either side has `IS_CONDITIONALLY_OPAQUE`
/// set; in that case the source's `VoxelShape` is consulted via
/// `face_occludes`. The fast path uses `VoxelShape::empty()` as the
/// source shape, which never occludes.
#[inline]
pub(crate) fn propagate_core<C: BfsChannel, const FLAGS: u8>(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &mut LightStorage,
    workspace: &mut C::Workspace,
    egress: &mut C::Egress,
) {
    let mut queue_read_index: usize = 0;
    loop {
        let entry = {
            let queue = if FLAGS == WAVE_INCREASE {
                C::increase_queue(workspace)
            } else {
                C::decrease_queue(workspace)
            };
            if queue_read_index >= queue.len() {
                break;
            }
            let e = queue[queue_read_index];
            queue_read_index += 1;
            e
        };

        let x = unpack_bfs_entry_x(entry);
        let z = unpack_bfs_entry_z(entry);
        let y_full = unpack_bfs_entry_y(entry);
        let propagated_level = unpack_bfs_entry_level(entry);
        let check_dir_bitset = unpack_bfs_entry_dir_bitset(entry);
        let entry_flags = unpack_bfs_entry_flags(entry);
        let y_local = (y_full as usize) & 0xF;

        debug_assert!(
            !(entry_flags & FLAG_RECHECK_LEVEL != 0 && entry_flags & FLAG_WRITE_LEVEL != 0),
            "RECHECK_LEVEL and WRITE_LEVEL must not co-occur on a single entry"
        );

        if FLAGS == WAVE_INCREASE {
            if entry_flags & FLAG_RECHECK_LEVEL != 0 {
                if light.get(x as usize, y_local, z as usize) != propagated_level {
                    continue;
                }
            } else if entry_flags & FLAG_WRITE_LEVEL != 0 {
                // Ingress and seed entries use WRITE_LEVEL to establish
                // the cell's level. The max-guard prevents a reflection
                // from a brighter source from overwriting a stronger
                // pre-existing value: each cross-chunk round trip loses
                // one Manhattan step, and an unconditional overwrite at
                // the source face would monotonically drain the source.
                let current = light.get(x as usize, y_local, z as usize);
                if propagated_level <= current {
                    continue;
                }
                light.set(x as usize, y_local, z as usize, propagated_level);
            }
        }

        let src_state = palette.get((x as i32, y_local as i32, z as i32));
        let src_flags = table.flags_for(src_state);
        let src_conditional = (src_flags & flag_bits::IS_CONDITIONALLY_OPAQUE) != 0;
        let from_shape: &'static VoxelShape = if src_conditional {
            table.occlusion_for(src_state)
        } else {
            VoxelShape::empty()
        };

        for &d in DIRECTIONS_FROM_BITSET[check_dir_bitset as usize] {
            let (dx, dy, dz) = normal_of(d);
            let off_x = x as i8 + dx;
            let off_y = y_local as i8 + dy;
            let off_z = z as i8 + dz;

            if !(0..=15).contains(&off_x)
                || !(0..=15).contains(&off_y)
                || !(0..=15).contains(&off_z)
            {
                let (cx, cz) = chunk_xyz_to_face_cell(d, off_x, off_y, off_z);
                C::push_egress(
                    egress,
                    Wavefront::new(d.index() as u8, cx, cz, propagated_level),
                );
                continue;
            }

            let off_x_u = off_x as usize;
            let off_y_u = off_y as usize;
            let off_z_u = off_z as usize;

            if FLAGS == WAVE_INCREASE {
                let current_level = light.get(off_x_u, off_y_u, off_z_u);
                let cheap_out_threshold = if d == Direction::Down && propagated_level == 15 {
                    propagated_level
                } else {
                    propagated_level.saturating_sub(1)
                };
                if current_level >= cheap_out_threshold {
                    continue;
                }

                let dst_state = palette.get((off_x as i32, off_y as i32, off_z as i32));
                let dst_flags = table.flags_for(dst_state);
                let mut emit_flags: u8 = 0;
                if (src_flags | dst_flags) & flag_bits::IS_CONDITIONALLY_OPAQUE != 0 {
                    let culling_face = table.occlusion_for(dst_state).face_shape(d.opposite());
                    if from_shape.face_occludes(culling_face, d) {
                        continue;
                    }
                    emit_flags |= FLAG_HAS_SIDED_TRANSPARENT_BLOCKS;
                }

                let opacity = table.dampening_for(dst_state);
                let target_level =
                    C::increase_target_level(propagated_level, d, opacity, dst_flags);
                if target_level <= current_level {
                    continue;
                }

                light.set(off_x_u, off_y_u, off_z_u, target_level);

                if target_level > 1 {
                    C::increase_queue(workspace).push(pack_bfs_entry(
                        off_x as u8,
                        off_z as u8,
                        off_y as u8,
                        target_level,
                        DIRECTIONS_EXCEPT_OPPOSITE[d.index()],
                        emit_flags,
                    ));
                }
            } else {
                // WAVE_DECREASE
                let light_level = light.get(off_x_u, off_y_u, off_z_u);
                if light_level == 0 {
                    continue;
                }

                let dst_state = palette.get((off_x as i32, off_y as i32, off_z as i32));
                let dst_flags = table.flags_for(dst_state);
                let mut emit_flags: u8 = 0;
                if (src_flags | dst_flags) & flag_bits::IS_CONDITIONALLY_OPAQUE != 0 {
                    let culling_face = table.occlusion_for(dst_state).face_shape(d.opposite());
                    if from_shape.face_occludes(culling_face, d) {
                        continue;
                    }
                    emit_flags |= FLAG_HAS_SIDED_TRANSPARENT_BLOCKS;
                }

                let opacity = table.dampening_for(dst_state);
                let target_level = propagated_level.saturating_sub(opacity.max(1));

                if light_level > target_level {
                    C::increase_queue(workspace).push(pack_bfs_entry(
                        off_x as u8,
                        off_z as u8,
                        off_y as u8,
                        light_level,
                        ALL_DIRECTIONS_BITSET,
                        emit_flags | FLAG_RECHECK_LEVEL,
                    ));
                    continue;
                }

                if let Some(emitted) = C::emission_for(table, dst_state) {
                    C::increase_queue(workspace).push(pack_bfs_entry(
                        off_x as u8,
                        off_z as u8,
                        off_y as u8,
                        emitted,
                        ALL_DIRECTIONS_BITSET,
                        emit_flags | FLAG_WRITE_LEVEL,
                    ));
                }

                light.set(off_x_u, off_y_u, off_z_u, 0);

                if target_level > 0 {
                    C::decrease_queue(workspace).push(pack_bfs_entry(
                        off_x as u8,
                        off_z as u8,
                        off_y as u8,
                        target_level,
                        DIRECTIONS_EXCEPT_OPPOSITE[d.index()],
                        emit_flags,
                    ));
                }
            }
        }
    }

    if FLAGS == WAVE_INCREASE {
        C::increase_queue(workspace).clear();
    } else {
        C::decrease_queue(workspace).clear();
    }
}

/// Block-light increase BFS over one chunk.
///
/// Drains `workspace.increase_queue` to empty. Cells whose stored level is
/// raised by this pass are written via `LightStorage::set`. Steps that fall
/// off the 0..=15 cube are converted into `Wavefront(u32)` entries on
/// `egress.0` for the cross-chunk distribute pass; the source chunk
/// itself never re-enqueues a boundary cell.
///
/// Slow-path branch fires when either side has `IS_CONDITIONALLY_OPAQUE`
/// set; in that case the source's `VoxelShape` is consulted via
/// `face_occludes`. The fast path uses `VoxelShape::empty()` as the
/// source shape, which never occludes.
pub fn propagate_increase(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &mut LightStorage,
    workspace: &mut BlockLightWorkspace,
    egress: &mut BlockEgress,
) {
    propagate_core::<BlockBfs, { WAVE_INCREASE }>(table, palette, light, workspace, egress);
}

/// Block-light decrease BFS over one chunk.
///
/// Drains `workspace.decrease_queue` to empty. Cells whose stored level is
/// dominated solely by the removed source path are zeroed via
/// `LightStorage::set(_, 0)`. Cells whose stored level exceeds the decrease
/// pass's path-derived target are NOT touched — instead they are requeued
/// onto `workspace.increase_queue` with `FLAG_RECHECK_LEVEL` so the
/// subsequent increase pass re-propagates from them after re-reading the
/// stored level. Emitter cells encountered en-route are requeued with
/// `FLAG_WRITE_LEVEL` so the increase pass restores their emission.
///
/// Slow-path branch fires when either side has `IS_CONDITIONALLY_OPAQUE`
/// set; in that case the source's `VoxelShape` is consulted via
/// `face_occludes`.
///
/// The function never calls `propagate_increase`. The two passes are
/// separated at the system-set level so a deferred barrier sits between
/// them, allowing other systems to observe the intermediate state.
pub fn propagate_decrease(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &mut LightStorage,
    workspace: &mut BlockLightWorkspace,
    egress: &mut BlockEgress,
) {
    propagate_core::<BlockBfs, { WAVE_DECREASE }>(table, palette, light, workspace, egress);
}

/// Sky-light increase BFS over one chunk.
///
/// Mirrors `propagate_increase` with two differences. First, the workspace
/// and egress are the sky-side types. Second, when the step direction is
/// `Direction::Down`, the source level is exactly 15, and the destination
/// cell's `PROPAGATES_SKYLIGHT_DOWN` flag is set, the new level is 15
/// (vertical free-fall through air). All other steps fall through to the
/// unified `parent - max(1, dampening)` attenuation shared with block-light.
pub fn propagate_increase_sky(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &mut LightStorage,
    workspace: &mut SkyLightWorkspace,
    egress: &mut SkyEgress,
) {
    propagate_core::<SkyBfs, { WAVE_INCREASE }>(table, palette, light, workspace, egress);
}

/// Sky-light decrease BFS over one chunk.
///
/// Mirrors `propagate_decrease` with two differences. First, the workspace
/// and egress are the sky-side types. Second, the destination-emission
/// re-emit branch is omitted entirely — sky-light has no per-cell
/// emission, so the only re-queue path is `FLAG_RECHECK_LEVEL` when the
/// neighbour's stored level exceeds the propagated target.
pub fn propagate_decrease_sky(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &mut LightStorage,
    workspace: &mut SkyLightWorkspace,
    egress: &mut SkyEgress,
) {
    propagate_core::<SkyBfs, { WAVE_DECREASE }>(table, palette, light, workspace, egress);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nibble::NibbleArray;
    use mcrs_protocol::BlockStateId;

    const ALL_DIRECTIONS: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    #[test]
    fn bfs_entry_pack_unpack_round_trip() {
        for &x in &[0u8, 7, 15] {
            for &z in &[0u8, 7, 15] {
                for &y in &[0u8, 7, 15] {
                    for &level in &[0u8, 7, 15] {
                        for &dir_bitset in &[0u8, 0b000001, 0b111110, 0b111111] {
                            for &flags in &[0u8, 1, 2, 4, 7] {
                                let packed = pack_bfs_entry(x, z, y, level, dir_bitset, flags);
                                assert_eq!(unpack_bfs_entry_x(packed), x, "x mismatch");
                                assert_eq!(unpack_bfs_entry_z(packed), z, "z mismatch");
                                assert_eq!(unpack_bfs_entry_y(packed) as u8, y, "y mismatch");
                                assert_eq!(
                                    unpack_bfs_entry_level(packed),
                                    level,
                                    "level mismatch"
                                );
                                assert_eq!(
                                    unpack_bfs_entry_dir_bitset(packed),
                                    dir_bitset,
                                    "dir_bitset mismatch"
                                );
                                assert_eq!(
                                    unpack_bfs_entry_flags(packed),
                                    flags,
                                    "flags mismatch"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn bfs_entry_reserved_bits_are_zero() {
        let packed = pack_bfs_entry(15, 15, 15, 15, 0b111111, 0b111);
        assert_eq!(packed >> 33, 0, "reserved bits 33..=63 must be zero");
    }

    #[test]
    fn bfs_directions_from_bitset_complete() {
        let slice = DIRECTIONS_FROM_BITSET[0b111111];
        assert_eq!(slice.len(), 6);
        for d in ALL_DIRECTIONS {
            assert!(
                slice.iter().filter(|&&x| x == d).count() == 1,
                "{:?} missing or duplicated",
                d
            );
        }
    }

    #[test]
    fn bfs_directions_from_bitset_empty() {
        let slice = DIRECTIONS_FROM_BITSET[0];
        assert!(slice.is_empty());
    }

    #[test]
    fn bfs_directions_from_bitset_single() {
        for d in ALL_DIRECTIONS {
            let bit = 1u8 << d.index();
            let slice = DIRECTIONS_FROM_BITSET[bit as usize];
            assert_eq!(slice.len(), 1, "len mismatch for {:?}", d);
            assert_eq!(slice[0], d);
        }
    }

    #[test]
    fn bfs_directions_except_opposite_clears_back_bit() {
        for d in ALL_DIRECTIONS {
            let bitset = DIRECTIONS_EXCEPT_OPPOSITE[d.index()];
            let opp_bit = 1u8 << d.opposite().index();
            assert_eq!(
                bitset & opp_bit,
                0,
                "opposite bit not cleared for {:?}",
                d
            );
            assert_eq!(
                bitset.count_ones(),
                5,
                "expected exactly five bits set for {:?}",
                d
            );
        }
    }

    struct TableSpec {
        emission: u8,
        dampening: u8,
        occlusion: &'static VoxelShape,
        flags: u8,
    }

    fn build_table(specs: &[(u16, TableSpec)]) -> BlockLightTable {
        let max_state = specs.iter().map(|(id, _)| *id).max().unwrap_or(0);
        let size = (max_state as usize) + 1;
        let mut emission = vec![0u8; size].into_boxed_slice();
        let mut dampening = vec![0u8; size].into_boxed_slice();
        let mut occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); size].into_boxed_slice();
        let mut flags = vec![0u8; size].into_boxed_slice();
        for (id, spec) in specs {
            let idx = *id as usize;
            emission[idx] = spec.emission;
            dampening[idx] = spec.dampening;
            occlusion[idx] = spec.occlusion;
            flags[idx] = spec.flags;
        }
        BlockLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    fn manhattan(a: (i32, i32, i32), b: (i32, i32, i32)) -> u8 {
        ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs()) as u8
    }

    fn fill_palette_with_air(palette: &mut BlockPalette) {
        palette.fill(BlockStateId(0));
    }

    /// Construct an empty `LightStorage::Mixed` directly. Seeding the source
    /// cell via `LightStorage::Null::set` produces `Uniform(emission)`, which
    /// causes `light.get` to report `emission` for every cell and the BFS to
    /// early-exit before propagating (see 03-RESEARCH.md Pitfall #6).
    fn zero_light_storage() -> LightStorage {
        LightStorage::Mixed(Box::new(NibbleArray::zeros()))
    }

    fn air_spec() -> TableSpec {
        TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: 0,
        }
    }

    fn torch_spec() -> TableSpec {
        TableSpec {
            emission: 14,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: 0,
        }
    }

    #[test]
    fn bfs_increase_single_emitter_all_air() {
        let table = build_table(&[(0, air_spec()), (0x1000, torch_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((8, 8, 8), BlockStateId(0x1000));

        let mut light = zero_light_storage();
        light.set(8, 8, 8, 14);

        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .increase_queue
            .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));

        propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert!(workspace.increase_queue.is_empty());
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let dist = manhattan((x, y, z), (8, 8, 8));
                    let expected = if dist == 0 { 14 } else { 14u8.saturating_sub(dist) };
                    let actual = light.get(x as usize, y as usize, z as usize);
                    assert_eq!(
                        actual, expected,
                        "cell ({}, {}, {}) at dist {}: got {} expected {}",
                        x, y, z, dist, actual, expected
                    );
                }
            }
        }
    }

    #[test]
    fn bfs_increase_attenuates_by_dampening() {
        const SLAB_HIGH: u16 = 2;
        const SLAB_ZERO: u16 = 3;
        let slab_high_spec = TableSpec {
            emission: 0,
            dampening: 2,
            occlusion: VoxelShape::empty(),
            flags: 0,
        };
        let slab_zero_spec = TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: 0,
        };

        // First scenario: dampening=2 produces target = 14 - max(1, 2) = 12 at
        // the slab cell, then 11 at the cell beyond.
        {
            let table = build_table(&[
                (0, air_spec()),
                (0x1000, torch_spec()),
                (SLAB_HIGH, slab_high_spec),
            ]);
            let mut palette = BlockPalette::default();
            fill_palette_with_air(&mut palette);
            palette.set((8, 8, 8), BlockStateId(0x1000));
            palette.set((8, 8, 9), BlockStateId(SLAB_HIGH));

            let mut light = zero_light_storage();
            light.set(8, 8, 8, 14);
            let mut workspace = BlockLightWorkspace::default();
            let mut egress = BlockEgress::default();
            workspace
                .increase_queue
                .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));
            propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

            assert_eq!(light.get(8, 8, 9), 14u8.saturating_sub(2), "slab cell");
            assert_eq!(
                light.get(8, 8, 10),
                14u8.saturating_sub(2).saturating_sub(1),
                "cell beyond slab"
            );
        }

        // Control: dampening=0 still attenuates by max(1, 0) = 1.
        {
            let table = build_table(&[
                (0, air_spec()),
                (0x1000, torch_spec()),
                (SLAB_ZERO, slab_zero_spec),
            ]);
            let mut palette = BlockPalette::default();
            fill_palette_with_air(&mut palette);
            palette.set((8, 8, 8), BlockStateId(0x1000));
            palette.set((8, 8, 9), BlockStateId(SLAB_ZERO));

            let mut light = zero_light_storage();
            light.set(8, 8, 8, 14);
            let mut workspace = BlockLightWorkspace::default();
            let mut egress = BlockEgress::default();
            workspace
                .increase_queue
                .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));
            propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

            assert_eq!(light.get(8, 8, 9), 14u8.saturating_sub(1));
            assert_eq!(light.get(8, 8, 10), 14u8.saturating_sub(2));
        }
    }

    #[test]
    fn bfs_increase_pushes_face_egress() {
        let table = build_table(&[(0, air_spec()), (0x1000, torch_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((15, 8, 8), BlockStateId(0x1000));

        let mut light = zero_light_storage();
        light.set(15, 8, 8, 14);
        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .increase_queue
            .push(pack_bfs_entry(15, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));

        propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert!(workspace.increase_queue.is_empty());
        // The source cell's own +X step must produce one egress wavefront
        // carrying the source level. The cross-chunk distribute pass is
        // responsible for the cross-chunk attenuation; the BFS just records
        // the pre-step level. Other cells on the x=15 plane that get reached
        // by the BFS also push East egress entries at lower levels — those
        // are not checked here.
        let expected_face = Direction::East.index() as u8;
        let found = egress.0.iter().any(|w| {
            w.face() == expected_face && w.cell_x() == 8 && w.cell_z() == 8 && w.level() == 14
        });
        assert!(
            found,
            "missing East egress wavefront (face=East, cell_x=8, cell_z=8, level=14); egress={:?}",
            egress.0
        );
    }

    #[test]
    fn bfs_increase_early_exit_dedup() {
        // One-seed reference run.
        let table = build_table(&[(0, air_spec()), (0x1000, torch_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((8, 8, 8), BlockStateId(0x1000));

        let mut light_one = zero_light_storage();
        light_one.set(8, 8, 8, 14);
        let mut workspace_one = BlockLightWorkspace::default();
        let mut egress_one = BlockEgress::default();
        workspace_one
            .increase_queue
            .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));
        propagate_increase(
            &table,
            &palette,
            &mut light_one,
            &mut workspace_one,
            &mut egress_one,
        );

        // Two-seed run: the second seed's neighbours all hit the early-exit.
        let mut light_two = zero_light_storage();
        light_two.set(8, 8, 8, 14);
        let mut workspace_two = BlockLightWorkspace::default();
        let mut egress_two = BlockEgress::default();
        workspace_two
            .increase_queue
            .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));
        workspace_two
            .increase_queue
            .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));
        propagate_increase(
            &table,
            &palette,
            &mut light_two,
            &mut workspace_two,
            &mut egress_two,
        );

        assert!(workspace_two.increase_queue.is_empty());
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(
                        light_one.get(x, y, z),
                        light_two.get(x, y, z),
                        "field mismatch at ({}, {}, {})",
                        x,
                        y,
                        z
                    );
                }
            }
        }
    }

    #[test]
    fn bfs_recheck_level_stale_skip_discards() {
        let table = build_table(&[(0, air_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);

        let mut light = zero_light_storage();
        light.set(0, 0, 0, 10);
        // Capture all neighbour values to assert nothing changed.
        let baseline: Vec<u8> = (0..6)
            .map(|i| {
                let d = ALL_DIRECTIONS[i];
                let (dx, dy, dz) = normal_of(d);
                let (nx, ny, nz) = (dx, dy, dz);
                if nx < 0 || ny < 0 || nz < 0 {
                    0
                } else {
                    light.get(nx as usize, ny as usize, nz as usize)
                }
            })
            .collect();

        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .increase_queue
            .push(pack_bfs_entry(0, 0, 0, 5, 0, FLAG_RECHECK_LEVEL));

        propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert_eq!(light.get(0, 0, 0), 10, "stale recheck must not touch cell");
        assert!(workspace.increase_queue.is_empty());
        assert!(egress.0.is_empty());
        for (i, d) in ALL_DIRECTIONS.iter().enumerate() {
            let (dx, dy, dz) = normal_of(*d);
            if dx < 0 || dy < 0 || dz < 0 {
                continue;
            }
            assert_eq!(
                light.get(dx as usize, dy as usize, dz as usize),
                baseline[i],
                "neighbour mutated for {:?}",
                d
            );
        }
    }

    #[test]
    fn bfs_increase_slow_path_face_occluded() {
        // Source state is a conditionally-opaque, full-cube emitter; the
        // destination has the same shape. The Block/Block face_occludes pair
        // returns true, so the BFS must NOT propagate light into dst.
        let src_spec = TableSpec {
            emission: 14,
            dampening: 0,
            occlusion: VoxelShape::block(),
            flags: flag_bits::IS_CONDITIONALLY_OPAQUE,
        };
        let dst_spec = TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::block(),
            flags: flag_bits::IS_CONDITIONALLY_OPAQUE,
        };
        let table = build_table(&[
            (0, air_spec()),
            (5, src_spec),
            (6, dst_spec),
            (0x1000, torch_spec()),
        ]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((5, 5, 5), BlockStateId(5));
        palette.set((5, 5, 6), BlockStateId(6));

        let mut light = zero_light_storage();
        light.set(5, 5, 5, 14);
        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        // Only walk in the +Z (South) direction so the test isolates the
        // src→dst face check; the bitset is 1 << South.index().
        let south_only_bitset = 1u8 << Direction::South.index();
        workspace
            .increase_queue
            .push(pack_bfs_entry(5, 5, 5, 14, south_only_bitset, 0));

        propagate_increase(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert_eq!(
            light.get(5, 5, 6),
            0,
            "slow path must block light through Block/Block face"
        );
    }

    /// Populate the L1-attenuated field for a single emitter at (ex,ey,ez)
    /// with emission `e`, in an all-air chunk, into a Mixed storage.
    fn seed_l1_field(light: &mut LightStorage, ex: i32, ey: i32, ez: i32, e: u8) {
        for y in 0..16i32 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let dist = ((x - ex).abs() + (y - ey).abs() + (z - ez).abs()) as u8;
                    let lvl = e.saturating_sub(dist);
                    if lvl > 0 {
                        light.set(x as usize, y as usize, z as usize, lvl);
                    }
                }
            }
        }
    }

    #[test]
    fn bfs_decrease_clears_emitter_field() {
        let table = build_table(&[(0, air_spec()), (0x1000, torch_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((8, 8, 8), BlockStateId(0x1000));

        // Pre-seed the L1-attenuated field as if the torch had been lit.
        let mut light = zero_light_storage();
        seed_l1_field(&mut light, 8, 8, 8, 14);

        // Zero out the torch cell — the cell itself stays unaffected by its
        // own decrease seed, so seeding it 0 reflects the post-removal state.
        light.set(8, 8, 8, 0);

        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .decrease_queue
            .push(pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));

        propagate_decrease(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert!(
            workspace.decrease_queue.is_empty(),
            "decrease_queue must drain to empty"
        );
        assert!(
            workspace.increase_queue.is_empty(),
            "no other emitter to requeue in all-air-single-torch scenario"
        );
        // Every cell whose only source was the now-removed torch reads 0.
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(
                        light.get(x, y, z),
                        0,
                        "cell ({}, {}, {}) should be dark",
                        x,
                        y,
                        z
                    );
                }
            }
        }
    }

    #[test]
    fn bfs_decrease_requeues_higher_stored() {
        let table = build_table(&[(0, air_spec()), (0x1000, torch_spec())]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        // Surviving emitter at (12, 8, 8); the removed one was at (4, 8, 8).
        palette.set((12, 8, 8), BlockStateId(0x1000));

        // Pre-seed the cells along (x, 8, 8) for x in 4..=12 with the
        // max-of-both-emitters L1 field. Outside this line the field is
        // zero — the BFS terminates on those `light_level == 0` cells.
        let mut light = zero_light_storage();
        for x in 4..=12i32 {
            let lvl_a = 14u8.saturating_sub((x - 4).unsigned_abs() as u8);
            let lvl_b = 14u8.saturating_sub((x - 12).unsigned_abs() as u8);
            let lvl = lvl_a.max(lvl_b);
            if lvl > 0 {
                light.set(x as usize, 8, 8, lvl);
            }
        }

        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .decrease_queue
            .push(pack_bfs_entry(4, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));

        propagate_decrease(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert!(workspace.decrease_queue.is_empty(), "decrease drains");
        assert!(
            !workspace.increase_queue.is_empty(),
            "expected at least one requeue into increase_queue"
        );
        // At least one entry carries FLAG_RECHECK_LEVEL.
        let recheck_count = workspace
            .increase_queue
            .iter()
            .filter(|&&e| unpack_bfs_entry_flags(e) & FLAG_RECHECK_LEVEL != 0)
            .count();
        assert!(
            recheck_count >= 1,
            "expected at least one FLAG_RECHECK_LEVEL entry"
        );
        // The surviving emitter cell must not be cleared by the decrease pass.
        assert_eq!(
            light.get(12, 8, 8),
            14,
            "surviving emitter cell must keep its stored level"
        );
    }

    #[test]
    fn bfs_decrease_emitter_cell_gets_write_level_flag() {
        // Removed emitter at (5, 8, 8) emission 14; surviving emitter at
        // (6, 8, 8) emission 7. The decrease walks east from (5, 8, 8) and
        // visits the surviving emitter cell, which must be requeued with
        // FLAG_WRITE_LEVEL (not FLAG_RECHECK_LEVEL) so the increase pass
        // restores its emission.
        const TORCH_HI: u16 = 0x1000;
        const TORCH_LO: u16 = 0x1001;
        let torch_lo_spec = TableSpec {
            emission: 7,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: 0,
        };
        let table = build_table(&[
            (0, air_spec()),
            (TORCH_HI, torch_spec()),
            (TORCH_LO, torch_lo_spec),
        ]);
        let mut palette = BlockPalette::default();
        fill_palette_with_air(&mut palette);
        palette.set((5, 8, 8), BlockStateId(TORCH_HI));
        palette.set((6, 8, 8), BlockStateId(TORCH_LO));

        let mut light = zero_light_storage();
        // (5, 8, 8) is the removed source — post-removal we treat it as 0.
        // (6, 8, 8) has its own emission of 7 plus the contribution from the
        // removed torch (14 - 1 = 13); the max is 13.
        light.set(6, 8, 8, 13);

        let mut workspace = BlockLightWorkspace::default();
        let mut egress = BlockEgress::default();
        workspace
            .decrease_queue
            .push(pack_bfs_entry(5, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0));

        propagate_decrease(&table, &palette, &mut light, &mut workspace, &mut egress);

        let write_level_entries: Vec<u64> = workspace
            .increase_queue
            .iter()
            .copied()
            .filter(|&e| unpack_bfs_entry_flags(e) & FLAG_WRITE_LEVEL != 0)
            .collect();
        assert!(
            !write_level_entries.is_empty(),
            "expected at least one FLAG_WRITE_LEVEL entry for emitter encountered"
        );
        let entry = write_level_entries[0];
        assert_eq!(
            unpack_bfs_entry_level(entry),
            7,
            "WRITE_LEVEL entry carries dst emission"
        );
        assert_eq!(
            unpack_bfs_entry_flags(entry) & FLAG_RECHECK_LEVEL,
            0,
            "WRITE_LEVEL entry must NOT also carry FLAG_RECHECK_LEVEL"
        );
    }

    // -------- sky-light BFS tests --------

    const SYNTH_AIR_ID: u16 = 0;
    const SYNTH_WATER_ID: u16 = 0x1002;
    const SYNTH_OPAQUE_SRC_ID: u16 = 0x20;
    const SYNTH_OPAQUE_DST_ID: u16 = 0x21;

    fn air_sky_spec() -> TableSpec {
        TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        }
    }

    fn water_sky_spec() -> TableSpec {
        TableSpec {
            emission: 0,
            dampening: 1,
            occlusion: VoxelShape::empty(),
            flags: flag_bits::IS_NOT_AIR,
        }
    }

    fn build_sky_air_table() -> BlockLightTable {
        build_table(&[(SYNTH_AIR_ID, air_sky_spec())])
    }

    #[test]
    fn bfs_sky_increase_vertical_drop_through_air() {
        let table = build_sky_air_table();
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));

        let mut light = zero_light_storage();
        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();

        workspace
            .increase_queue
            .push(pack_bfs_entry(8, 8, 15, 15, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL));

        propagate_increase_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        for y in 0..16usize {
            assert_eq!(
                light.get(8, y, 8),
                15,
                "expected vertical drop to keep level 15 at (8, {}, 8)",
                y
            );
        }
    }

    #[test]
    fn bfs_sky_increase_down_through_pre_attenuated_column() {
        // Reproduces the chunk-border off-by-one regression: when a column
        // already holds level 14 (because cross-chunk horizontal wavefronts at
        // attenuated level 14 arrived first and wrote those cells), a
        // subsequent Down propagation from a level-15 cell above must still
        // upgrade the cells underneath to 15, not skip them. The PROPAGATES_
        // SKYLIGHT_DOWN flag preserves level 15 unattenuated through air.
        let table = build_sky_air_table();
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));

        let mut light = zero_light_storage();
        for y in 0..15usize {
            light.set(15, y, 15, 14);
        }

        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();

        workspace.increase_queue.push(pack_bfs_entry(
            15,
            15,
            15,
            15,
            ALL_DIRECTIONS_BITSET,
            FLAG_WRITE_LEVEL,
        ));

        propagate_increase_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        for y in 0..16usize {
            assert_eq!(
                light.get(15, y, 15),
                15,
                "Down propagation must upgrade pre-attenuated cell at (15, {}, 15) from 14 to 15",
                y
            );
        }
    }

    #[test]
    fn bfs_sky_increase_attenuates_at_water() {
        let table = build_table(&[
            (SYNTH_AIR_ID, air_sky_spec()),
            (SYNTH_WATER_ID, water_sky_spec()),
        ]);
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));
        palette.set((8, 10, 8), BlockStateId(SYNTH_WATER_ID));

        let mut light = zero_light_storage();
        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();

        workspace
            .increase_queue
            .push(pack_bfs_entry(8, 8, 15, 15, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL));

        propagate_increase_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert_eq!(light.get(8, 15, 8), 15, "top of column stays 15");
        assert_eq!(light.get(8, 11, 8), 15, "air above water stays 15");
        assert_eq!(light.get(8, 10, 8), 14, "water cell drops to 14");
        assert_eq!(light.get(8, 9, 8), 13, "cell below water attenuates by 1");
        assert_eq!(light.get(8, 8, 8), 12, "two cells below water reads 12");
    }

    #[test]
    fn bfs_sky_increase_horizontal_attenuates_by_one() {
        // Seed an east-going wavefront at level 14 with the east-only direction
        // bitset so the BFS must use the unified parent - max(1, dampening) rule
        // on the horizontal step.
        let table = build_sky_air_table();
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));

        let mut light = zero_light_storage();
        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();

        let east_only_bitset = 1u8 << Direction::East.index();
        light.set(5, 5, 5, 14);
        workspace.increase_queue.push(pack_bfs_entry(
            5,
            5,
            5,
            14,
            east_only_bitset,
            0,
        ));

        propagate_increase_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert_eq!(
            light.get(6, 5, 5),
            13,
            "first east step from level 14 attenuates by max(1, 0) = 1"
        );
    }

    #[test]
    fn bfs_sky_increase_slow_path_face_occluded() {
        // Conditionally-opaque source + destination with full-cube occlusion
        // shapes — Block/Block face_occludes returns true, BFS must not propagate.
        let src_spec = TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::block(),
            flags: flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        };
        let dst_spec = TableSpec {
            emission: 0,
            dampening: 0,
            occlusion: VoxelShape::block(),
            flags: flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        };
        let table = build_table(&[
            (SYNTH_AIR_ID, air_sky_spec()),
            (SYNTH_OPAQUE_SRC_ID, src_spec),
            (SYNTH_OPAQUE_DST_ID, dst_spec),
        ]);
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));
        palette.set((5, 5, 5), BlockStateId(SYNTH_OPAQUE_SRC_ID));
        palette.set((5, 5, 6), BlockStateId(SYNTH_OPAQUE_DST_ID));

        let mut light = zero_light_storage();
        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();

        light.set(5, 5, 5, 14);
        let south_only_bitset = 1u8 << Direction::South.index();
        workspace
            .increase_queue
            .push(pack_bfs_entry(5, 5, 5, 14, south_only_bitset, 0));

        propagate_increase_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        assert_eq!(
            light.get(5, 5, 6),
            0,
            "slow path must block sky-light through Block/Block face"
        );
    }

    #[test]
    fn bfs_sky_decrease_requeues_higher_stored() {
        let table = build_sky_air_table();
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));

        let mut light = zero_light_storage();
        // Neighbour cell holds a higher level than the decrease propagation —
        // expect the BFS to requeue it with FLAG_RECHECK_LEVEL.
        light.set(5, 8, 8, 12);

        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();
        // Push a decrease entry from (4, 8, 8) at level 6 only walking east.
        let east_only_bitset = 1u8 << Direction::East.index();
        workspace
            .decrease_queue
            .push(pack_bfs_entry(4, 8, 8, 6, east_only_bitset, 0));

        propagate_decrease_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        let recheck_count = workspace
            .increase_queue
            .iter()
            .filter(|&&e| unpack_bfs_entry_flags(e) & FLAG_RECHECK_LEVEL != 0)
            .count();
        assert!(
            recheck_count >= 1,
            "expected at least one FLAG_RECHECK_LEVEL entry from higher-stored neighbour"
        );
        // The higher-stored neighbour must NOT have been cleared.
        assert_eq!(
            light.get(5, 8, 8),
            12,
            "higher-stored neighbour must not be cleared by decrease pass"
        );
    }

    #[test]
    fn bfs_sky_decrease_no_write_level_reemit() {
        // Sky has no per-cell emission. Even if the destination state has a
        // table-recorded emission (which shouldn't happen for real sky data
        // but we synthesize it here to prove the branch is omitted), the sky
        // decrease pass must NOT emit any FLAG_WRITE_LEVEL requeue entries.
        let pseudo_emitter_spec = TableSpec {
            emission: 7,
            dampening: 0,
            occlusion: VoxelShape::empty(),
            flags: flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        };
        let table = build_table(&[
            (SYNTH_AIR_ID, air_sky_spec()),
            (SYNTH_OPAQUE_SRC_ID, pseudo_emitter_spec),
        ]);
        let mut palette = BlockPalette::default();
        palette.fill(BlockStateId(SYNTH_AIR_ID));
        // Place the pseudo-emitter at (6, 8, 8) — the cell visited by the
        // east-walking decrease pass.
        palette.set((6, 8, 8), BlockStateId(SYNTH_OPAQUE_SRC_ID));

        let mut light = zero_light_storage();
        // (6, 8, 8) holds level 6 — equal to the target (14 - 1 - dampening),
        // so the recheck branch does not fire and we exercise the post-write
        // region where the block path would emit a WRITE_LEVEL entry.
        light.set(6, 8, 8, 6);

        let mut workspace = SkyLightWorkspace::default();
        let mut egress = SkyEgress::default();
        let east_only_bitset = 1u8 << Direction::East.index();
        workspace
            .decrease_queue
            .push(pack_bfs_entry(5, 8, 8, 14, east_only_bitset, 0));

        propagate_decrease_sky(&table, &palette, &mut light, &mut workspace, &mut egress);

        for &entry in workspace.increase_queue.iter() {
            assert_eq!(
                unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
                0,
                "sky decrease must never emit FLAG_WRITE_LEVEL (no per-cell emission)"
            );
        }
    }

}
