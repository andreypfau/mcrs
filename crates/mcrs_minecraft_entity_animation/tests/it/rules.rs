//! Rules of the clip time, the accumulation of a pose and the scripts that the pose artefact
//! does not reach.

use glam::Vec3;
use mcrs_minecraft_entity_animation::{
    ClientEntityDefinition, ClipLibrary, EntityAnimation, EntityQuery, Pose, PoseBinding, PoseBone,
    QuerySource, Rig,
};
use mcrs_minecraft_entity_model::{BakedGeometry, GeometryFile};
use mcrs_molang::{ScalarVm, Value};

/// One bone with the rest rotation 20° about X.
const GEOMETRY: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
    "description": {"identifier": "geometry.test", "texture_width": 16, "texture_height": 16},
    "bones": [{"name": "bone", "pivot": [0, 0, 0], "rotation": [20, 0, 0]}]}]}"#;

struct Inputs {
    life_time: f32,
    baby: bool,
}

impl QuerySource for Inputs {
    fn query(&self, query: EntityQuery) -> Option<Value> {
        match query {
            EntityQuery::LifeTime => Some(Value::Number(self.life_time)),
            EntityQuery::IsBaby => Some(Value::Number(if self.baby { 1.0 } else { 0.0 })),
            _ => None,
        }
    }
}

struct Fixture {
    rig: Rig,
    geometry: BakedGeometry,
    entity: EntityAnimation,
    vm: ScalarVm,
}

impl Fixture {
    /// A rig of the clips `animations` with the `scripts` object of a client entity. Each clip
    /// has the short name of the last part of its identifier.
    fn new(scripts: &str, animations: &str) -> Self {
        let mut library = ClipLibrary::new();
        library
            .add_file(&format!(
                r#"{{"format_version": "1.8.0", "animations": {animations}}}"#
            ))
            .unwrap();
        let names: Vec<String> = ["a", "b", "c"]
            .iter()
            .map(|name| format!(r#""{name}": "animation.test.{name}""#))
            .collect();
        let definition = ClientEntityDefinition::parse(&format!(
            r#"{{"format_version": "1.10.0", "minecraft:client_entity": {{"description": {{
                "identifier": "test", "animations": {{{}}}, "scripts": {scripts}}}}}}}"#,
            names.join(", ")
        ))
        .unwrap();
        let mut diagnostics = Vec::new();
        let rig = Rig::compile(&definition, &library, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file: GeometryFile = GEOMETRY.parse().unwrap();
        let geometry = file.geometries[0].bake(&mut Vec::new()).unwrap();
        let entity = EntityAnimation::new(&rig);
        Self {
            rig,
            geometry,
            entity,
            vm: ScalarVm::new(),
        }
    }

    /// The file pose of the bone after a frame at the entity clock `clock`, in ticks: the
    /// absolute rotation, the position and the scale.
    fn frame(&mut self, clock: f32, baby: bool) -> [Vec3; 3] {
        let inputs = Inputs {
            life_time: clock / 20.0,
            baby,
        };
        self.entity.update(&self.rig, &mut self.vm, clock, &inputs);
        let binding = PoseBinding::new(&self.rig, &self.geometry);
        let mut pose = Pose::new(&self.rig);
        self.entity
            .pose(&self.rig, &mut self.vm, &binding, &mut pose);
        let bone = self
            .rig
            .bone("bone")
            .map_or(PoseBone::REST, |index| pose.bones[index]);
        [
            Vec3::new(20.0, 0.0, 0.0) + bone.rotation,
            bone.position,
            bone.scale,
        ]
    }

    fn variable(&self, name: &str) -> Option<f32> {
        let slot = self.rig.layout().slot(name)?;
        let raw = self.entity.variables().get(slot)?;
        Some(f32::from_bits(raw as u32))
    }
}

fn close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, 1e-4),
        "{actual} is not {expected}"
    );
}

#[test]
fn weights_add_rotations_multiply_scales_and_this_reads_the_pose_before_the_clip() {
    let mut fixture = Fixture::new(
        r#"{"animate": [{"a": 0.5}, "b"]}"#,
        r#"{
            "animation.test.a": {"loop": true, "bones": {"Bone": {"rotation": [10, 0, 0], "scale": 2}}},
            "animation.test.b": {"loop": true, "bones": {"bone": {"rotation": ["this * 2", 0, 0], "scale": [3]}}}
        }"#,
    );
    let [rotation, _, scale] = fixture.frame(0.0, false);
    // a adds 0.5 · 10; b reads this = 20 + 5 and adds 50.
    close(rotation, Vec3::new(75.0, 0.0, 0.0));
    close(scale, Vec3::splat((1.0 + (2.0 - 1.0) * 0.5) * 3.0));
}

#[test]
fn an_override_resets_rotation_and_position_but_keeps_a_scale_it_does_not_animate() {
    let mut fixture = Fixture::new(
        r#"{"animate": ["a", "b"]}"#,
        r#"{
            "animation.test.a": {"loop": true, "bones": {"bone": {"rotation": [10, 0, 0], "position": [5, 5, 5], "scale": 2}}},
            "animation.test.b": {"loop": true, "override_previous_animation": true, "bones": {"bone": {"position": [1, 2, 3]}}}
        }"#,
    );
    let [rotation, position, scale] = fixture.frame(0.0, false);
    close(rotation, Vec3::new(20.0, 0.0, 0.0));
    close(position, Vec3::new(1.0, 2.0, 3.0));
    close(scale, Vec3::splat(2.0));
}

#[test]
fn catmull_rom_uses_the_end_keyframe_as_its_missing_neighbour() {
    let mut fixture = Fixture::new(
        r#"{"animate": ["a"]}"#,
        r#"{"animation.test.a": {"loop": "hold_on_last_frame", "bones": {"bone": {"position": {
            "0": {"post": [0, 0, 0], "lerp_mode": "catmullrom"},
            "1": {"post": [10, 0, 0], "lerp_mode": "catmullrom"},
            "2": {"post": [0, 0, 0], "lerp_mode": "catmullrom"}}}}}}"#,
    );
    fixture.frame(0.0, false);
    // p0 = p1 = 0, p2 = 10, p3 = 0 at α = 0.5.
    let [_, first, _] = fixture.frame(10.0, false);
    let expected = 0.5 * (10.0 * 0.5 + 40.0 * 0.25 - 30.0 * 0.125);
    close(first, Vec3::new(expected, 0.0, 0.0));
    // p0 = 0, p1 = 10, p2 = 0, p3 = p2 = 0 at α = 0.5.
    let [_, second, _] = fixture.frame(30.0, false);
    let expected = 0.5 * (20.0 - 50.0 * 0.25 + 30.0 * 0.125);
    close(second, Vec3::new(expected, 0.0, 0.0));
}

#[test]
fn the_loop_modes_wrap_hold_or_finish_the_time() {
    let clip = |name: &str, mode: &str| {
        format!(
            r#""animation.test.{name}": {{"loop": {mode}, "bones": {{"bone": {{"position": {{"0": [0, 0, 0], "1": [10, 0, 0]}}}}}}}}"#
        )
    };
    for (mode, expected) in [
        ("true", Some(5.0)),
        (r#""hold_on_last_frame""#, Some(10.0)),
        ("false", None),
    ] {
        let mut fixture =
            Fixture::new(r#"{"animate": ["a"]}"#, &format!("{{{}}}", clip("a", mode)));
        fixture.frame(100.0, false);
        let [_, position, _] = fixture.frame(130.0, false);
        let expected = expected.map_or(Vec3::ZERO, |x| Vec3::new(x, 0.0, 0.0));
        close(position, expected);
    }
}

#[test]
fn a_start_delay_holds_the_time_and_a_weight_of_zero_restarts_the_clip() {
    let mut fixture = Fixture::new(
        r#"{"animate": [{"a": "q.is_baby"}]}"#,
        r#"{"animation.test.a": {"loop": true, "start_delay": 0.5, "animation_length": 10,
            "bones": {"bone": {"position": {"0": [0, 0, 0], "10": [100, 0, 0]}}}}}"#,
    );
    assert_eq!(
        fixture.frame(0.0, true)[1],
        Vec3::ZERO,
        "the delay adds nothing"
    );
    close(fixture.frame(10.0, true)[1], Vec3::ZERO);
    close(fixture.frame(30.0, true)[1], Vec3::new(10.0, 0.0, 0.0));
    fixture.frame(40.0, false);
    fixture.frame(50.0, true);
    close(fixture.frame(70.0, true)[1], Vec3::ZERO);
    close(fixture.frame(80.0, true)[1], Vec3::new(5.0, 0.0, 0.0));
}

#[test]
fn script_arrays_are_one_program_and_a_script_runs_again_when_its_inputs_change() {
    let mut fixture = Fixture::new(
        r#"{
            "initialize": ["v.count = 10;"],
            "pre_animation": ["q.is_baby ? {", "v.doubled = q.life_time * 2;", "};", "v.count = v.count + 1;"],
            "animate": []
        }"#,
        "{}",
    );
    fixture.frame(0.0, true);
    fixture.frame(20.0, true);
    assert_eq!(fixture.variable("doubled"), Some(2.0));
    assert_eq!(
        fixture.variable("count"),
        Some(12.0),
        "a script that reads its own write runs each frame"
    );
    let mut fixture = Fixture::new(
        r#"{"pre_animation": ["v.doubled = q.life_time * 2;"], "animate": []}"#,
        "{}",
    );
    fixture.frame(20.0, false);
    fixture.frame(20.0, false);
    fixture.frame(40.0, false);
    assert_eq!(fixture.variable("doubled"), Some(4.0));
}

#[test]
fn a_bone_relative_to_the_entity_keeps_its_rotation_and_follows_its_parent_pivot() {
    let geometry = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"identifier": "geometry.test", "texture_width": 16, "texture_height": 16},
        "bones": [{"name": "parent", "pivot": [0, 0, 0]},
                  {"name": "child", "parent": "parent", "pivot": [0, 4, 2]}]}]}"#;
    let mut fixture = Fixture::new(
        r#"{"animate": ["a"]}"#,
        r#"{"animation.test.a": {"loop": true, "bones": {
            "parent": {"rotation": [0, 90, 0]},
            "child": {"rotation": [0, 0, 0], "relative_to": {"rotation": "entity"}}}}}"#,
    );
    let file: GeometryFile = geometry.parse().unwrap();
    fixture.geometry = file.geometries[0].bake(&mut Vec::new()).unwrap();
    fixture.frame(0.0, false);
    let binding = PoseBinding::new(&fixture.rig, &fixture.geometry);
    let mut pose = Pose::new(&fixture.rig);
    fixture
        .entity
        .pose(&fixture.rig, &mut fixture.vm, &binding, &mut pose);
    let transforms = binding.model_transforms(&fixture.geometry, &pose);
    let pivot = fixture.geometry.bones[1].pivot;
    assert!(
        transforms[1]
            .matrix3
            .abs_diff_eq(glam::Mat3A::IDENTITY, 1e-6),
        "the parent rotation does not turn the child: {:?}",
        transforms[1].matrix3
    );
    close(
        transforms[1].transform_point3(pivot),
        transforms[0].transform_point3(pivot),
    );
    assert!(
        !transforms[0]
            .transform_point3(pivot)
            .abs_diff_eq(pivot, 1e-3),
        "the parent moves the pivot of the child"
    );
}
