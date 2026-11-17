use glam::DVec3;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PoseInputs;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::select_pose;
use mcrs_minecraft_pose::Pose;
use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug)]
enum Ceiling {
    None,
    TopSlabAbove,
    StoneAbove,
    StoneAtFeet,
}

fn inputs() -> PoseInputs {
    PoseInputs {
        current: Pose::Standing,
        entity: TestFixture::query_entity(DVec3::new(0.5, 1.0, 0.5)),
        scale: 1.0,
        sleeping: false,
        swimming: false,
        fall_flying: false,
        spin_attack: false,
        shift: false,
        flight: false,
        spectator: false,
        passenger: false,
    }
}

struct Case {
    name: &'static str,
    ceiling: Ceiling,
    inputs: PoseInputs,
    expected: Pose,
}

fn check(cases: &[Case]) {
    for case in cases {
        let mut fixture = TestFixture::new();
        let block = match case.ceiling {
            Ceiling::None => None,
            Ceiling::TopSlabAbove => Some((2, fixture.blocks.top_slab)),
            Ceiling::StoneAbove => Some((2, fixture.blocks.stone)),
            Ceiling::StoneAtFeet => Some((1, fixture.blocks.stone)),
        };
        if let Some((y, state)) = block {
            fixture.world.set(BlockPos::new(0, y, 0), state);
        }
        let view = View::new(&fixture.world, &NoBodies, &fixture.tables);
        let pose = select_pose(view, &case.inputs, &mut fixture.scratch);
        assert_eq!(pose, case.expected, "{}", case.name);
    }
}

#[test]
fn desired_pose_follows_the_order_of_the_list() {
    let base = inputs();
    let case = |name, inputs, expected| Case {
        name,
        ceiling: Ceiling::None,
        inputs,
        expected,
    };
    check(&[
        case("standing", base, Pose::Standing),
        case(
            "shift",
            PoseInputs {
                shift: true,
                ..base
            },
            Pose::Crouching,
        ),
        case(
            "shift in flight",
            PoseInputs {
                shift: true,
                flight: true,
                ..base
            },
            Pose::Standing,
        ),
        case(
            "sleeping before swimming",
            PoseInputs {
                sleeping: true,
                swimming: true,
                ..base
            },
            Pose::Sleeping,
        ),
        case(
            "swimming before fall flying",
            PoseInputs {
                swimming: true,
                fall_flying: true,
                ..base
            },
            Pose::Swimming,
        ),
        case(
            "fall flying before spin attack",
            PoseInputs {
                fall_flying: true,
                spin_attack: true,
                ..base
            },
            Pose::FallFlying,
        ),
        case(
            "spin attack before crouching",
            PoseInputs {
                spin_attack: true,
                shift: true,
                ..base
            },
            Pose::SpinAttack,
        ),
    ]);
}

#[test]
fn a_pose_without_room_gives_the_next_pose_with_room() {
    let base = inputs();
    let case = |name, ceiling, inputs, expected| Case {
        name,
        ceiling,
        inputs,
        expected,
    };
    check(&[
        case(
            "no room to stand",
            Ceiling::TopSlabAbove,
            base,
            Pose::Crouching,
        ),
        case(
            "no room to crouch",
            Ceiling::StoneAbove,
            base,
            Pose::Swimming,
        ),
        case(
            "fall flying under a low ceiling",
            Ceiling::StoneAbove,
            PoseInputs {
                fall_flying: true,
                ..base
            },
            Pose::FallFlying,
        ),
        case(
            "passenger",
            Ceiling::StoneAbove,
            PoseInputs {
                passenger: true,
                ..base
            },
            Pose::Standing,
        ),
        case(
            "spectator",
            Ceiling::StoneAbove,
            PoseInputs {
                spectator: true,
                ..base
            },
            Pose::Standing,
        ),
        case(
            "no room to swim keeps the pose",
            Ceiling::StoneAtFeet,
            PoseInputs {
                current: Pose::Sleeping,
                spectator: true,
                ..base
            },
            Pose::Sleeping,
        ),
        case(
            "half scale fits under the slab",
            Ceiling::TopSlabAbove,
            PoseInputs { scale: 0.5, ..base },
            Pose::Standing,
        ),
    ]);
}
