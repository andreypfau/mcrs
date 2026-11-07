//! The batch run gives the poses of the scalar run, bit for bit.

use mcrs_minecraft_entity_animation::{
    BatchLanes, EntityAnimation, EntityQuery, Pose, PoseBatch, PoseBinding, QuerySource, Rig,
};
use mcrs_molang::{ScalarVm, Value, string_hash};

use crate::pack;

#[derive(Clone, Copy)]
struct Lane {
    index: usize,
}

impl QuerySource for Lane {
    fn query(&self, query: EntityQuery) -> Option<Value> {
        let index = self.index;
        let number = |value: f32| Some(Value::Number(value));
        match query {
            EntityQuery::ModifiedDistanceMoved => number(index as f32 * 0.37),
            EntityQuery::ModifiedMoveSpeed => number((index % 5) as f32 * 0.1),
            EntityQuery::TargetXRotation => number((index % 60) as f32 - 30.0),
            EntityQuery::TargetYRotation => number((index % 90) as f32 - 45.0),
            EntityQuery::IsInWater => number(f32::from(u8::from(index.is_multiple_of(3)))),
            EntityQuery::Pose => {
                let pose = ["standing", "croaking", "long_jumping", "using_tongue"][index % 4];
                Some(Value::String(string_hash(pose)))
            }
            _ => None,
        }
    }
}

fn compare(rig: &Rig, geometry: &str) {
    let geometries = pack::geometries(geometry);
    let binding = PoseBinding::new(rig, &geometries[0]);
    let count = 300;
    let sources: Vec<Lane> = (0..count).map(|index| Lane { index }).collect();
    let mut scalar_entities = vec![EntityAnimation::new(rig); count];
    let mut batch_entities = scalar_entities.clone();
    let mut scalar_poses = vec![Pose::new(rig); count];
    let mut batch_poses = scalar_poses.clone();
    let mut vm = ScalarVm::new();
    let mut batch: PoseBatch<64> = PoseBatch::new(rig);
    for frame in 0..4 {
        let clocks: Vec<f32> = (0..count)
            .map(|index| frame as f32 * 7.5 + index as f32)
            .collect();
        for (((entity, source), pose), clock) in scalar_entities
            .iter_mut()
            .zip(&sources)
            .zip(&mut scalar_poses)
            .zip(&clocks)
        {
            entity.update(rig, &mut vm, *clock, source);
            entity.pose(rig, &mut vm, &binding, pose);
        }
        batch.run(
            rig,
            &binding,
            BatchLanes {
                entities: &mut batch_entities,
                clocks: &clocks,
                sources: &sources,
                poses: &mut batch_poses,
            },
        );
        assert!(
            scalar_poses == batch_poses,
            "{}: frame {frame} differs",
            rig.identifier
        );
    }
}

#[test]
fn a_batch_run_gives_the_poses_of_a_scalar_run() {
    compare(&pack::rig("pig"), "pig");
    compare(&pack::rig("frog"), "frog");
}
