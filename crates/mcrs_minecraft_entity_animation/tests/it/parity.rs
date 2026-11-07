//! Compares the pose of the pig and the frog with each case of the pose artefact, within the
//! pose tolerance: 1e-3 radian, 1e-3 model unit and 1e-4 of scale.

use glam::Vec3;
use mcrs_minecraft_entity_animation::{
    EntityAnimation, EntityQuery, Pose, PoseBinding, QuerySource, Rig,
};
use mcrs_minecraft_entity_model::BakedGeometry;
use mcrs_molang::{ScalarVm, Value, string_hash};

use crate::artefact::{Inputs, PoseArtefact, PoseCase};
use crate::pack;

const ROTATION_TOLERANCE: f64 = 1e-3;
const POSITION_TOLERANCE: f64 = 1e-3;
const SCALE_TOLERANCE: f64 = 1e-4;

/// The clips whose start tick the case gives, with the value of `query.pose` while they play.
const STARTED_CLIPS: [(&str, Option<&str>); 4] = [
    ("jump", Some("long_jumping")),
    ("croak", Some("croaking")),
    ("tongue", Some("using_tongue")),
    ("idle_water", None),
];

/// The walk clips of the frog and the factor of their `anim_time_update`.
const WALK_CLIPS: [(&str, f32); 2] = [("walk", 0.075), ("swim", 0.05)];

struct CaseQueries<'a> {
    inputs: &'a Inputs,
}

impl QuerySource for CaseQueries<'_> {
    fn query(&self, query: EntityQuery) -> Option<Value> {
        let inputs = self.inputs;
        let number = |value: f64| Some(Value::Number(value as f32));
        match query {
            EntityQuery::ModifiedDistanceMoved => number(inputs.walk_position),
            EntityQuery::ModifiedMoveSpeed => number(inputs.walk_speed),
            EntityQuery::TargetXRotation => number(inputs.head_pitch_degrees),
            EntityQuery::TargetYRotation => number(inputs.head_yaw_degrees),
            EntityQuery::LifeTime => number(inputs.age_ticks / 20.0),
            EntityQuery::IsInWater => number(if inputs.swimming { 1.0 } else { 0.0 }),
            EntityQuery::Pose => {
                let pose = STARTED_CLIPS
                    .iter()
                    .find(|(clip, pose)| {
                        pose.is_some()
                            && inputs
                                .clip_start_ticks
                                .get(*clip)
                                .copied()
                                .flatten()
                                .is_some()
                    })
                    .and_then(|(_, pose)| *pose)
                    .unwrap_or("standing");
                Some(Value::String(string_hash(pose)))
            }
            _ => None,
        }
    }
}

/// A time truncated to a whole millisecond. The guard keeps a decimal time that binary cannot
/// hold, such as 0.85 s, at its own millisecond.
fn whole_milliseconds(seconds: f64) -> f32 {
    ((seconds * 1000.0 + 1e-6).floor() / 1000.0) as f32
}

#[derive(Clone, Copy, Debug, Default)]
struct Errors {
    rotation: f64,
    position: f64,
    scale: f64,
}

impl Errors {
    fn max(&mut self, other: Errors) {
        self.rotation = self.rotation.max(other.rotation);
        self.position = self.position.max(other.position);
        self.scale = self.scale.max(other.scale);
    }
}

fn largest(a: Vec3, b: [f64; 3]) -> f64 {
    a.to_array()
        .iter()
        .zip(b)
        .map(|(a, b)| (f64::from(*a) - b).abs())
        .fold(0.0, f64::max)
}

struct Mob {
    rig: Rig,
    geometries: Vec<BakedGeometry>,
}

impl Mob {
    fn geometry(&self, identifier: &str) -> &BakedGeometry {
        self.geometries
            .iter()
            .find(|geometry| geometry.identifier == identifier)
            .unwrap_or_else(|| panic!("no geometry {identifier}"))
    }
}

fn geometry_of(case: &PoseCase) -> &'static str {
    match (case.model.as_str(), case.layer.as_str()) {
        ("minecraft:pig", "main") => "geometry.pig",
        ("minecraft:cold_pig", "main") => "geometry.pig.cold",
        ("minecraft:pig_baby", "main") => "geometry.pig.baby",
        ("minecraft:pig", "saddle") => "geometry.pig.saddle",
        ("minecraft:frog", "main") => "geometry.frog",
        (model, layer) => panic!("no geometry for {model} / {layer}"),
    }
}

/// Sets the weight and the time of the clips that the case starts, as the pose artefact states.
fn set_case_clips(rig: &Rig, entity: &mut EntityAnimation, inputs: &Inputs) {
    for (clip, _) in STARTED_CLIPS {
        let Some(entry) = rig.entry(clip) else {
            continue;
        };
        match inputs.clip_start_ticks.get(clip).copied().flatten() {
            Some(start) => {
                entity.set_weight(entry, 1.0);
                let time = whole_milliseconds((inputs.age_ticks - start) / 20.0);
                entity.set_time(rig, entry, time);
            }
            None => entity.set_weight(entry, 0.0),
        }
    }
    for (clip, factor) in WALK_CLIPS {
        let Some(entry) = rig.entry(clip) else {
            continue;
        };
        let time = inputs.walk_position as f32 * factor;
        entity.set_time(rig, entry, whole_milliseconds(f64::from(time)));
    }
}

fn compare(mob: &Mob, case: &PoseCase, vm: &mut ScalarVm) -> Errors {
    let geometry = mob.geometry(geometry_of(case));
    let binding = PoseBinding::new(&mob.rig, geometry);
    let mut entity = EntityAnimation::new(&mob.rig);
    let source = CaseQueries {
        inputs: &case.inputs,
    };
    entity.update(&mob.rig, vm, case.inputs.age_ticks as f32, &source);
    set_case_clips(&mob.rig, &mut entity, &case.inputs);
    let mut pose = Pose::new(&mob.rig);
    entity.pose(&mob.rig, vm, &binding, &mut pose);
    let tongue = case
        .inputs
        .clip_start_ticks
        .get("tongue")
        .copied()
        .flatten()
        .is_some();
    let mut errors = Errors::default();
    for (path, expected) in &case.parts {
        let Some((parent_path, name)) = path.rsplit_once('/') else {
            continue;
        };
        let bone = geometry
            .bones
            .iter()
            .find(|bone| bone.name == name)
            .unwrap_or_else(|| panic!("{} has no bone {name}", geometry.identifier));
        let parent_pivot = bone.parent.map_or(Vec3::new(0.0, 24.0, 0.0), |parent| {
            file_pivot(&geometry.bones[parent].pivot)
        });
        let pivot = file_pivot(&bone.pivot);
        let rest = [
            pivot.x - parent_pivot.x,
            parent_pivot.y - pivot.y,
            pivot.z - parent_pivot.z,
        ];
        let [x, y, z] = expected.position;
        let offset = [
            x - f64::from(rest[0]),
            -(y - f64::from(rest[1])),
            z - f64::from(rest[2]),
        ];
        let animated = mob.rig.bone(name).and_then(|index| pose.bones.get(index));
        let rotation = bone.file_rotation + animated.map_or(Vec3::ZERO, |bone| bone.rotation);
        let rotation = Vec3::new(
            rotation.x.to_radians(),
            rotation.y.to_radians(),
            rotation.z.to_radians(),
        );
        let position = animated.map_or(Vec3::ZERO, |bone| bone.position);
        let scale = animated.map_or(Vec3::ONE, |bone| bone.scale);
        let skip_scale = tongue && parent_path.ends_with("/body") && name == "head";
        let part = Errors {
            rotation: largest(rotation, expected.rotation),
            position: largest(position, offset),
            scale: if skip_scale {
                0.0
            } else {
                largest(scale, expected.scale)
            },
        };
        assert!(
            part.rotation <= ROTATION_TOLERANCE
                && part.position <= POSITION_TOLERANCE
                && part.scale <= SCALE_TOLERANCE,
            "{} / {} {path} {:?}: {part:?}\nexpected {:?} {:?} {:?}\nposed {rotation} {position} {scale}",
            case.model,
            case.layer,
            serde_json::to_string(&case.inputs.clip_start_ticks).unwrap(),
            expected.rotation,
            offset,
            expected.scale,
        );
        errors.max(part);
    }
    errors
}

fn file_pivot(internal: &Vec3) -> Vec3 {
    Vec3::new(-internal.x, internal.y, internal.z)
}

#[test]
fn the_pig_and_the_frog_agree_with_each_case_of_the_pose_artefact() {
    let pig = Mob {
        rig: pack::rig("pig"),
        geometries: pack::geometries("pig"),
    };
    let frog = Mob {
        rig: pack::rig("frog"),
        geometries: pack::geometries("frog"),
    };
    let artefact = PoseArtefact::read();
    let mut vm = ScalarVm::new();
    let mut pig_errors = Errors::default();
    let mut frog_errors = Errors::default();
    let (mut pig_cases, mut frog_cases) = (0, 0);
    for case in &artefact.cases {
        if case.model == "minecraft:frog" {
            frog_errors.max(compare(&frog, case, &mut vm));
            frog_cases += 1;
        } else {
            pig_errors.max(compare(&pig, case, &mut vm));
            pig_cases += 1;
        }
    }
    println!("pig: {pig_cases} cases, max error {pig_errors:?}");
    println!("frog: {frog_cases} cases, max error {frog_errors:?}");
    assert_eq!(
        (pig_cases, frog_cases),
        (432, 59),
        "the artefact has each case"
    );
}
