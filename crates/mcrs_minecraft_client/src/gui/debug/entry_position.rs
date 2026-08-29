use bevy::prelude::*;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_voxel_math::{BlockPos, ChunkPos, Direction};
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

use super::{DebugEntryGroup, DebugScreenDisplayer};
use crate::player::Player;
use crate::sky::PlayerDimension;

pub const GROUP: DebugEntryGroup = ResourceLocation::new_static("minecraft:position");

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    camera: Single<&PhysicsTransform, With<Player>>,
    dimension: Res<PlayerDimension>,
) {
    let position = camera.translation;
    let feet = BlockPos::from(position);
    let chunk = ChunkPos::from(feet);
    let direction = Direction::from_y_rot(camera.rotation.yaw());
    let facing = match direction {
        Direction::North => "Towards negative Z",
        Direction::South => "Towards positive Z",
        Direction::West => "Towards negative X",
        Direction::East => "Towards positive X",
        Direction::Down | Direction::Up => "Invalid",
    };

    displayer.add_to_group(
        GROUP,
        [
            format!(
                "XYZ: {:.3} / {:.5} / {:.3}",
                position.x, position.y, position.z
            ),
            format!("Block: {} {} {}", feet.x, feet.y, feet.z),
            format!(
                "Chunk: {} {} {} [{} {} in r.{}.{}.mca]",
                chunk.x,
                chunk.y,
                chunk.z,
                chunk.x & 31,
                chunk.z & 31,
                chunk.x >> 5,
                chunk.z >> 5,
            ),
            format!(
                "Facing: {direction} ({facing}) ({:.1} / {:.1})",
                camera.rotation.yaw(),
                camera.rotation.pitch(),
            ),
            // Only a server force-loads chunks, and this client has none.
            format!("{} FC: 0", dimension.0),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::DVec3;
    use mcrs_voxel_world::entity::physics::Rotation;

    #[test]
    fn the_group_reads_the_way_vanilla_prints_it() {
        let mut world = World::new();
        world.init_resource::<DebugScreenDisplayer>();
        world.insert_resource(PlayerDimension("minecraft:overworld".to_owned()));
        world.spawn((
            Player,
            PhysicsTransform::from_translation(DVec3::new(100.5, 71.0, -33.25))
                .with_rotation(Rotation::new(45.0, -10.0)),
        ));
        world.run_system_once(display).unwrap();

        let (left, right) = world.resource::<DebugScreenDisplayer>().columns();
        assert_eq!(
            left,
            [
                "XYZ: 100.500 / 71.00000 / -33.250",
                "Block: 100 71 -34",
                "Chunk: 6 4 -3 [6 29 in r.0.-1.mca]",
                "Facing: west (Towards negative X) (45.0 / -10.0)",
                "minecraft:overworld FC: 0",
                "",
            ]
        );
        assert!(right.is_empty());
    }
}
