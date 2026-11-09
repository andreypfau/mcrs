//! Runs the pose pass on a real adapter for each case of the pose artefact and compares each
//! bone with the pose of the CPU animation crate for the same inputs, within the pose tolerance
//! of `docs/spec/entity-animation.md`, EANIM-GPU-003.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod support;

use std::collections::{BTreeMap, HashMap};

use glam::{Affine3A, Mat3, Vec3, Vec4};
use mcrs_minecraft_entity_animation::{
    EntityAnimation, EntityQuery, Pose, PoseBinding, QuerySource, Rig,
};
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, CullRecord, DefinitionTables, EntityGpu, EntityLibrary, FrameUniform,
    PackEntity, SlotArena, StateRecord, TickRecord, f16_up, input_record,
};
use mcrs_minecraft_fixtures::fixture;
use mcrs_molang::{ScalarVm, Value, string_hash};
use serde::Deserialize;

#[derive(Deserialize)]
struct PoseArtefact {
    cases: Vec<PoseCase>,
}

#[derive(Deserialize)]
struct PoseCase {
    model: String,
    layer: String,
    inputs: Inputs,
}

#[derive(Deserialize)]
struct Inputs {
    walk_position: f64,
    walk_speed: f64,
    head_yaw_degrees: f64,
    head_pitch_degrees: f64,
    age_ticks: f64,
    #[serde(default)]
    swimming: bool,
    #[serde(default)]
    clip_start_ticks: BTreeMap<String, Option<f64>>,
}

/// The clips whose start tick a case gives, with the value of `query.pose` while they play.
const STARTED_CLIPS: [(&str, Option<&str>); 4] = [
    ("jump", Some("long_jumping")),
    ("croak", Some("croaking")),
    ("tongue", Some("using_tongue")),
    ("idle_water", None),
];

const TICK: u32 = 100_000;

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

fn whole_milliseconds(seconds: f64) -> f32 {
    ((seconds * 1000.0 + 1e-6).floor() / 1000.0) as f32
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

/// The animation of a case on the CPU, at the entity clock `clock`: the play list of the
/// client entity, with the started clips of the case.
fn animate(rig: &Rig, inputs: &Inputs, vm: &mut ScalarVm) -> EntityAnimation {
    let mut entity = EntityAnimation::new(rig);
    entity.update(
        rig,
        vm,
        inputs.age_ticks.floor() as f32,
        &CaseQueries { inputs },
    );
    for (clip, _) in STARTED_CLIPS {
        let Some(entry) = rig.entry(clip) else {
            continue;
        };
        match inputs.clip_start_ticks.get(clip).copied().flatten() {
            Some(start) => {
                entity.set_weight(entry, 1.0);
                entity.set_time(
                    rig,
                    entry,
                    whole_milliseconds((inputs.age_ticks - start) / 20.0),
                );
            }
            None => entity.set_weight(entry, 0.0),
        }
    }
    entity
}

fn turn(degrees: f64) -> u16 {
    ((degrees.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 % 65536) as u16
}

#[derive(Clone, Copy, Debug, Default)]
struct Errors {
    rotation: f32,
    position: f32,
    scale: f32,
}

impl Errors {
    fn max(&mut self, other: Errors) {
        self.rotation = self.rotation.max(other.rotation);
        self.position = self.position.max(other.position);
        self.scale = self.scale.max(other.scale);
    }
}

/// A local bone transform as the pose values of EGEO-POSE-001: the offset, the Euler angles
/// `Rz · Ry · Rx` in radians and the scale.
struct Decomposed {
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
}

fn decompose(local: Affine3A, pivot: Vec3) -> Decomposed {
    let linear = Mat3::from(local.matrix3);
    let scale = Vec3::new(
        linear.x_axis.length(),
        linear.y_axis.length(),
        linear.z_axis.length(),
    );
    let position = Vec3::from(local.translation) - pivot + linear * pivot;
    let r = Mat3::from_cols(
        linear.x_axis / scale.x.max(1e-12),
        linear.y_axis / scale.y.max(1e-12),
        linear.z_axis / scale.z.max(1e-12),
    );
    let rotation = Vec3::new(
        r.y_axis.z.atan2(r.z_axis.z),
        (-r.x_axis.z).clamp(-1.0, 1.0).asin(),
        r.x_axis.y.atan2(r.x_axis.x),
    );
    Decomposed {
        position,
        rotation,
        scale,
    }
}

fn limit(absolute: f32, a: f32, b: f32) -> f32 {
    absolute.max(a.abs().max(b.abs()) * 2f32.powi(-20))
}

fn angle_difference(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    ((a - b + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI).abs()
}

/// The worst ratio of the difference to the limit of each value, and the differences.
fn compare(gpu: &Decomposed, cpu: &Decomposed) -> (f32, Errors) {
    let mut ratio = 0.0f32;
    let mut errors = Errors::default();
    for axis in 0..3 {
        let (g, c) = (gpu.position[axis], cpu.position[axis]);
        errors.position = errors.position.max((g - c).abs());
        ratio = ratio.max((g - c).abs() / limit(1e-3, g, c));
        let (g, c) = (gpu.scale[axis], cpu.scale[axis]);
        errors.scale = errors.scale.max((g - c).abs());
        ratio = ratio.max((g - c).abs() / limit(1e-4, g, c));
    }
    if cpu.scale.min_element() > 1e-3 {
        for axis in 0..3 {
            let (g, c) = (gpu.rotation[axis], cpu.rotation[axis]);
            let difference = angle_difference(g, c);
            errors.rotation = errors.rotation.max(difference);
            ratio = ratio.max(difference / limit(1e-3, g, c));
        }
    }
    (ratio, errors)
}

fn locals(transforms: &[Affine3A], parents: &[Option<usize>]) -> Vec<Affine3A> {
    transforms
        .iter()
        .zip(parents)
        .map(|(world, parent)| match parent {
            Some(parent) => transforms[*parent].inverse() * *world,
            None => *world,
        })
        .collect()
}

/// Writes the slot records of one case, as the client writes them for an entity.
fn write_case(
    arena: &mut SlotArena,
    tables: &DefinitionTables,
    definition: usize,
    geometry: usize,
    entity: &PackEntity,
    animation: &EntityAnimation,
    inputs: &Inputs,
) -> u32 {
    let slot = arena.allocate();
    let age = inputs.age_ticks.floor() as i32;
    let program = tables.program.as_ref().unwrap();
    let uses_input = program.inputs.is_used();
    if uses_input {
        arena.write_input(
            slot,
            input_record(&program.inputs, &entity.rig, animation, age as f32),
        );
    }
    arena.write_state(
        slot,
        StateRecord {
            definition: definition as u16,
            layer_mask: 1,
            layers: [
                [tables.models[geometry].unwrap(), 0],
                [0; 2],
                [0; 2],
                [0; 2],
            ],
            hurt_start: -1e9,
            scale: f16_up(1.0),
            section: u32::MAX,
            clock_offset: TICK as i32 - age,
            input: if uses_input { slot } else { u32::MAX },
            ..StateRecord::default()
        },
    );
    let pitch = (inputs.head_pitch_degrees / 180.0 * 32767.0).round() as i16;
    arena.write_tick(
        slot,
        TICK,
        TickRecord {
            block: [0, 64, 0],
            fraction: [0; 3],
            body_yaw: 0,
            head_yaw: turn(inputs.head_yaw_degrees),
            pitch,
            walk_position: (inputs.walk_position as f32).to_bits(),
            walk_speed: (inputs.walk_speed as f32).to_bits(),
        },
        true,
    );
    arena.write_cull(
        slot,
        CullRecord {
            block: [0, 64, 0],
            radius: f16_up(2.0),
            flags: CULL_VALID,
        },
    );
    slot
}

struct Slot<'a> {
    case: &'a PoseCase,
    entity: &'a PackEntity,
    geometry: usize,
    reference: Vec<Affine3A>,
}

/// Compares the bones that the pose pass wrote for each visible slot with the CPU pose: the
/// count of posed slots, the worst ratio to the tolerance and the largest differences.
fn compare_with_cpu<'a>(
    gpu: &support::Gpu,
    entity_gpu: &EntityGpu,
    library: &EntityLibrary,
    capacities: Capacities,
    slots: &HashMap<u32, Slot<'a>>,
) -> (usize, f32, BTreeMap<&'a str, Errors>) {
    let counters: Vec<u32> = bytemuck::cast_slice(&gpu.read(entity_gpu.counters())).to_vec();
    let visible: Vec<[u32; 2]> = bytemuck::cast_slice(&gpu.read(entity_gpu.visible())).to_vec();
    let bones: Vec<[f32; 4]> = bytemuck::cast_slice(&gpu.read(entity_gpu.bone_matrices())).to_vec();
    let mut seen = 0;
    let mut errors: BTreeMap<&str, Errors> = BTreeMap::new();
    let mut worst = 0.0f32;
    for definition in 0..library.definitions.len() {
        let count = counters[1 + definition] as usize;
        let base = definition * capacities.full_rigs as usize;
        for &[slot, first_bone] in &visible[base..base + count] {
            let found = &slots[&slot];
            let baked = found.entity.geometries[found.geometry].as_ref().unwrap();
            let gpu_transforms: Vec<Affine3A> = (0..baked.bones.len())
                .map(|bone| {
                    let at = (first_bone as usize + bone) * 3;
                    let [r0, r1, r2] = [bones[at], bones[at + 1], bones[at + 2]].map(Vec4::from);
                    Affine3A::from_cols_array(&[
                        r0.x, r1.x, r2.x, r0.y, r1.y, r2.y, r0.z, r1.z, r2.z, r0.w, r1.w, r2.w,
                    ])
                })
                .collect();
            let parents: Vec<Option<usize>> = baked.bones.iter().map(|bone| bone.parent).collect();
            let gpu_locals = locals(&gpu_transforms, &parents);
            let cpu_locals = locals(&found.reference, &parents);
            for ((bone, gpu_local), cpu_local) in
                baked.bones.iter().zip(&gpu_locals).zip(&cpu_locals)
            {
                let (ratio, difference) = compare(
                    &decompose(*gpu_local, bone.pivot),
                    &decompose(*cpu_local, bone.pivot),
                );
                assert!(
                    ratio <= 1.0,
                    "{} / {} {:?} bone {}: {difference:?}\ngpu {gpu_local:?}\ncpu {cpu_local:?}",
                    found.case.model,
                    found.case.layer,
                    found.case.inputs.clip_start_ticks,
                    bone.name,
                );
                worst = worst.max(ratio);
                errors
                    .entry(found.case.model.as_str())
                    .or_default()
                    .max(difference);
            }
            seen += 1;
        }
    }
    (seen, worst, errors)
}

#[test]
fn the_gpu_pose_of_each_case_agrees_with_the_cpu_animation() {
    let Some(gpu) = support::Gpu::new() else {
        eprintln!("no adapter: the GPU pose parity does not run");
        return;
    };
    let pack = support::pack();
    let library = EntityLibrary::build(&pack.sources(), &[support::white_texture()], 2048);
    for definition in &library.definitions {
        assert!(
            definition.program.is_ok(),
            "{}: {:?}",
            definition.identifier,
            definition.program
        );
    }
    let artefact: PoseArtefact = serde_json::from_str(&fixture("entity_poses").text()).unwrap();
    let mut arena = SlotArena::new();
    let mut vm = ScalarVm::new();
    let mut slots: HashMap<u32, Slot<'_>> = HashMap::new();
    for case in &artefact.cases {
        let identifier = if case.model == "minecraft:frog" {
            "minecraft:frog"
        } else {
            "minecraft:pig"
        };
        let (definition, entity) = pack.entity(identifier).unwrap();
        let tables = &library.definitions[definition];
        let geometry = entity
            .resources
            .geometries
            .iter()
            .position(|(_, id)| id == geometry_of(case))
            .unwrap();
        let baked = entity.geometries[geometry].as_ref().unwrap();
        let inputs = &case.inputs;
        let animation = animate(&entity.rig, inputs, &mut vm);
        let mut reference_animation = animation.clone();
        let binding = PoseBinding::new(&entity.rig, baked);
        let mut pose = Pose::new(&entity.rig);
        reference_animation.pose(&entity.rig, &mut vm, &binding, &mut pose);
        let reference = binding.model_transforms(baked, &pose);

        let slot = write_case(
            &mut arena, tables, definition, geometry, entity, &animation, inputs,
        );
        slots.insert(
            slot,
            Slot {
                case,
                entity,
                geometry,
                reference,
            },
        );
    }

    let capacities = Capacities::for_cap(&library, 1024);
    let mut entity_gpu = gpu.entity_gpu(&library, capacities, arena.capacity());
    let upload = arena.take_upload();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
    let frame = FrameUniform {
        frustum: [[0.0, 0.0, 0.0, 1.0]; 5],
        tick: TICK,
        partial_tick: 0.0,
        viewport: [854.0, 480.0],
        ..FrameUniform::default()
    };
    entity_gpu.write_frame(&gpu.queue, &frame, upload.slot_count);
    entity_gpu.encode_compute(&mut encoder, None);
    gpu.queue.submit([encoder.finish()]);

    let (seen, worst, errors) = compare_with_cpu(&gpu, &entity_gpu, &library, capacities, &slots);
    println!(
        "{}: {seen} cases, worst error {worst:.3} of the tolerance",
        gpu.name
    );
    for (model, errors) in &errors {
        println!("{model}: {errors:?}");
    }
    assert_eq!(seen, artefact.cases.len(), "the pose pass posed each case");
}
