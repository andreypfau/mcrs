use bevy_math::IVec3;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
use mcrs_voxel_storage::VoxelId;

/// `VinesFeature.place`: the first face of `Direction.values()` order, minus
/// down, whose neighbour can hold a vine. It draws nothing at all.
///
/// `vine` is one state per face over `Direction::all()`; the down slot is never
/// read, since a vine has no bottom face.
pub fn place_vines<W: WorldGenVolume>(vine: &[VoxelId; 6], volume: &mut W, at: IVec3) -> bool {
    if !volume.is_air(at) {
        return false;
    }
    for (index, direction) in Direction::all().into_iter().enumerate() {
        if direction == Direction::Down {
            continue;
        }
        // `MultifaceBlock.canAttachTo` as the full-collision-cube flag, so a
        // vine refuses a stair or a slab the reference would hang from.
        let neighbour = direction.relative(at, 1);
        if volume.holds(&volume.world().sturdy_up, neighbour) {
            volume.set(at, vine[index]);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {

    use mcrs_minecraft_worldgen::feature::placer::single_state;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const AT: IVec3 = IVec3::new(0, 70, 0);

    /// Stone is what a vine can hang from.
    fn cave(mut volume: FakeVolume) -> FakeVolume {
        volume.world.sturdy_up = single_state(STONE);
        volume
    }

    fn config() -> [VoxelId; 6] {
        [
            VoxelId(20),
            VoxelId(21),
            VoxelId(22),
            VoxelId(23),
            VoxelId(24),
            VoxelId(25),
        ]
    }

    /// Up comes before the horizontals in `Direction.values()`, so a ceiling
    /// wins over a wall even when both are there.
    #[test]
    fn the_first_face_of_the_reference_order_takes_the_vine() {
        let cfg = config();
        let mut volume = cave(FakeVolume::with([
            ((AT.x, AT.y + 1, AT.z), STONE),
            ((AT.x, AT.y, AT.z - 1), STONE),
        ]));

        assert!(place_vines(&cfg, &mut volume, AT));
        assert_eq!(volume.writes, vec![((AT.x, AT.y, AT.z), cfg[1])]);
    }

    /// The face below is skipped, so a vine standing on stone alone places
    /// nothing.
    #[test]
    fn a_neighbour_below_is_no_reason_to_place() {
        let cfg = config();
        let mut volume = cave(FakeVolume::with([((AT.x, AT.y - 1, AT.z), STONE)]));

        assert!(!place_vines(&cfg, &mut volume, AT));
        assert!(volume.writes.is_empty());
    }

    /// North is the first horizontal of `Direction.values()`, ahead of south,
    /// west and east.
    #[test]
    fn the_walls_are_tried_north_south_west_east() {
        let cfg = config();
        let mut volume = cave(FakeVolume::with([
            ((AT.x, AT.y, AT.z + 1), STONE),
            ((AT.x, AT.y, AT.z - 1), STONE),
        ]));

        assert!(place_vines(&cfg, &mut volume, AT));
        assert_eq!(volume.writes, vec![((AT.x, AT.y, AT.z), cfg[2])]);
    }

    /// An occupied cell is refused before any neighbour is read.
    #[test]
    fn a_cell_that_is_not_empty_is_refused() {
        let cfg = config();
        let mut volume =
            FakeVolume::with([((AT.x, AT.y, AT.z), STONE), ((AT.x, AT.y + 1, AT.z), STONE)]);

        assert!(!place_vines(&cfg, &mut volume, AT));
        assert!(volume.writes.is_empty());
    }
}
