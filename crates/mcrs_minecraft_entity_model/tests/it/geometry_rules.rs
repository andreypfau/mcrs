use glam::{Vec2, Vec3};
use mcrs_minecraft_entity_model::{
    Bone, CUBOID_EXPLICIT, Cube, CubeUv, CuboidRecords, Diagnostic, FormatVersion, Geometry,
    GeometryError, GeometryFile, resolve_parents,
};

fn geometry(bones: Vec<Bone>) -> Geometry {
    let mut geometry = Geometry::new("geometry.test".to_owned(), FormatVersion::V1_12_0);
    geometry.texture_width = Some(64.0);
    geometry.texture_height = Some(64.0);
    geometry.bones = bones;
    geometry
}

fn bone(name: &str, parent: Option<&str>) -> Bone {
    let mut bone = Bone::new(name.to_owned());
    bone.parent = parent.map(str::to_owned);
    bone
}

fn unit_cube() -> Cube {
    Cube::new(Vec3::ZERO, Vec3::ONE, CubeUv::Box(Vec2::ZERO))
}

fn chain(length: usize) -> Vec<Bone> {
    (0..length)
        .map(|index| {
            let parent = index.checked_sub(1).map(|parent| format!("b{parent}"));
            bone(&format!("b{index}"), parent.as_deref())
        })
        .collect()
}

#[test]
fn a_positive_file_pitch_turns_the_front_down_and_a_positive_yaw_turns_it_right() {
    let front = Vec3::NEG_Z;
    for (rotation, expected) in [
        (Vec3::new(90.0, 0.0, 0.0), Vec3::NEG_Y),
        (Vec3::new(0.0, 90.0, 0.0), Vec3::X),
    ] {
        let mut turned = bone("head", None);
        turned.rotation = rotation;
        let baked = geometry(vec![turned]).bake(&mut Vec::new()).unwrap();
        let transform = baked.model_transforms(&[])[0];
        let direction = transform.transform_vector3(front);
        assert!(
            direction.distance(expected) < 1e-6,
            "the file rotation {rotation} turns the front to {direction}"
        );
    }
}

#[test]
fn a_geometry_over_a_limit_is_rejected() {
    let mut many_cubes = bone("body", None);
    many_cubes.cubes = vec![unit_cube(); 2049];
    let mut most_cubes = bone("body", None);
    most_cubes.cubes = vec![unit_cube(); 2048];
    let named = |count: usize| {
        (0..count)
            .map(|index| bone(&format!("b{index}"), None))
            .collect()
    };
    let cases: [(Vec<Bone>, Option<GeometryError>); 7] = [
        (Vec::new(), Some(GeometryError::NoBones)),
        (named(128), None),
        (named(129), Some(GeometryError::TooManyBones { count: 129 })),
        (chain(16), None),
        (
            chain(17),
            Some(GeometryError::TooDeep {
                bone: "b16".to_owned(),
            }),
        ),
        (vec![most_cubes], None),
        (
            vec![many_cubes],
            Some(GeometryError::TooManyCubes { count: 2049 }),
        ),
    ];
    for (bones, expected) in cases {
        let count = bones.len();
        let result = geometry(bones).bake(&mut Vec::new());
        assert_eq!(result.err(), expected, "a geometry with {count} bones");
    }
}

#[test]
fn bones_are_ordered_parent_first_and_bad_parents_are_reported() {
    let mut leg = bone("leg", Some("BODY"));
    leg.cubes.push(unit_cube());
    let bones = vec![leg, bone("tail", Some("missing")), bone("body", None)];
    let mut diagnostics = Vec::new();
    let baked = geometry(bones).bake(&mut diagnostics).unwrap();
    let names: Vec<&str> = baked.bones.iter().map(|bone| bone.name.as_str()).collect();
    assert_eq!(names, ["tail", "body", "leg"]);
    assert_eq!(baked.bones[2].parent, Some(1));
    assert_eq!(baked.bones[0].parent, None);
    assert_eq!(baked.cubes[0].bone, 2);
    assert_eq!(
        diagnostics,
        [Diagnostic::MissingParent {
            bone: "tail".to_owned(),
            parent: "missing".to_owned(),
        }]
    );

    let cycle = geometry(vec![bone("a", Some("b")), bone("b", Some("a"))]);
    assert!(matches!(
        cycle.bake(&mut Vec::new()),
        Err(GeometryError::ParentCycle { .. })
    ));
    let twice = geometry(vec![bone("head", None), bone("HEAD", None)]);
    assert!(matches!(
        twice.bake(&mut Vec::new()),
        Err(GeometryError::DuplicateBone { .. })
    ));
}

#[test]
fn computed_visible_bounds_grow_the_box_of_the_cubes_by_half() {
    let mut body = bone("body", None);
    body.cubes.push(Cube::new(
        Vec3::new(-8.0, 0.0, -8.0),
        Vec3::splat(16.0),
        CubeUv::Box(Vec2::ZERO),
    ));
    let bounds = geometry(vec![body])
        .bake(&mut Vec::new())
        .unwrap()
        .visible_bounds;
    assert!(
        bounds.min.distance(Vec3::new(-0.75, -0.25, -0.75)) < 1e-6,
        "{bounds:?}"
    );
    assert!(
        bounds.max.distance(Vec3::new(0.75, 1.25, 0.75)) < 1e-6,
        "{bounds:?}"
    );
}

const LEGACY: &str = r#"{
  "format_version": "1.8.0",
  // a comment
  "geometry.base": {
    "texturewidth": 32, "textureheight": 32,
    "bones": [
      { "name": "body", "pivot": [0, 4, 0], "cubes": [{ "origin": [-2, 0, -2], "size": [4, 4, 4], "uv": [0, 0] }] },
      { "name": "head", "parent": "body", "pivot": [0, 8, 0], "cubes": [{ "origin": [-1, 4, -1], "size": [2, 2, 2], "uv": [0, 0] }] },
    ],
  },
  /* the child replaces the head and adds an ear */
  "geometry.child:geometry.base": {
    "textureheight": 16,
    "textureheight": 64,
    "bones": [
      { "name": "HEAD", "parent": "body", "pivot": [0, 8, 0], "rotation": [10, 0, 0], "cubes": [{ "origin": [-1, 4, -1], "size": [2, 3, 2], "uv": [8, 0], "mirror": true }] },
      { "name": "ear", "parent": "HEAD", "pivot": [0, 7, 0], "neverRender": true, "cubes": [{ "origin": [0, 7, 0], "size": [1, 1, 0], "uv": [16, 0], "inflate": 0.25 }] }
    ]
  }
}"#;

const CURRENT: &str = r#"{
  "format_version": "1.16.0",
  "minecraft:geometry": [{
    "description": { "identifier": "geometry.child", "texture_width": 32, "texture_height": 64 },
    "bones": [
      { "name": "body", "pivot": [0, 4, 0], "cubes": [{ "origin": [-2, 0, -2], "size": [4, 4, 4], "uv": [0, 0] }] },
      { "name": "HEAD", "parent": "body", "pivot": [0, 8, 0], "rotation": [10, 0, 0], "cubes": [{ "origin": [-1, 4, -1], "size": [2, 3, 2], "uv": [8, 0], "mirror": true }] },
      { "name": "ear", "parent": "HEAD", "pivot": [0, 7, 0], "cubes": [{ "origin": [0, 7, 0], "size": [1, 1, 0], "uv": [16, 0], "inflate": 0.25 }] }
    ]
  }]
}"#;

#[test]
fn a_lenient_legacy_file_with_a_parent_bakes_like_the_same_current_file() {
    let legacy: GeometryFile = LEGACY.parse().unwrap();
    assert!(legacy.diagnostics.is_empty(), "{:?}", legacy.diagnostics);
    let mut diagnostics = Vec::new();
    let resolved = resolve_parents(legacy.geometries, &mut diagnostics);
    let child = resolved
        .iter()
        .find(|geometry| geometry.identifier == "geometry.child")
        .unwrap();
    let current: GeometryFile = CURRENT.parse().unwrap();
    let mut baked_legacy = child.bake(&mut diagnostics).unwrap();
    let baked_current = current.geometries[0].bake(&mut diagnostics).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(baked_legacy.bones[2].never_render);
    baked_legacy.bones[2].never_render = false;
    assert_eq!(baked_legacy, baked_current);
}

#[test]
fn a_rotated_cube_gets_explicit_faces_with_the_rotation_baked_in() {
    let mut body = bone("body", None);
    let mut cube = Cube::new(
        Vec3::ZERO,
        Vec3::new(2.0, 1.0, 1.0),
        CubeUv::Box(Vec2::ZERO),
    );
    cube.rotation = Vec3::new(0.0, 90.0, 0.0);
    cube.pivot = Some(Vec3::ZERO);
    body.cubes.push(cube);
    let baked = geometry(vec![body]).bake(&mut Vec::new()).unwrap();
    let records = CuboidRecords::new(&baked).unwrap();
    let cuboid = records.cuboids[0];
    assert_eq!(cuboid.flags, CUBOID_EXPLICIT | 0b11_1111);
    assert_eq!(records.slot_count(), 1 + 2 * 6);
    let east = records.explicit_faces[cuboid.first_explicit_face().unwrap() as usize];
    let normal = Vec3::from(east.edge_1)
        .cross(Vec3::from(east.edge_2))
        .normalize();
    assert!(
        normal.distance(Vec3::Z) < 1e-6,
        "a file yaw of 90° turns the east face to +Z, not {normal}"
    );
    assert!(
        Vec3::from(east.corner).distance(Vec3::new(-1.0, 1.0, 0.0)) < 1e-6,
        "the east corner (0, 1, 1) turns to (-1, 1, 0), not {:?}",
        east.corner
    );
}

const WARNINGS: &str = r#"{
  "format_version": "1.16.0",
  "minecraft:geometry": [{
    "description": { "identifier": "geometry.a", "texture_width": 0, "texture_height": 32 },
    "bones": [
      { "name": "body", "poly_mesh": {}, "locators": { "lead": [1, 0, 0], "LEAD": [2, 0, 0] } },
      { "name": "head", "parent": "body", "texture_meshes": [], "locators": { "Lead": [3, 0, 0] } }
    ]
  }, {
    "description": { "identifier": "geometry.b", "texture_width": 32, "texture_height": 32 },
    "bones": [{ "name": "body", "poly_mesh": {} }]
  }]
}"#;

#[test]
fn a_bad_texture_size_meshes_and_duplicate_locators_give_one_warning_each() {
    let file: GeometryFile = WARNINGS.parse().unwrap();
    let meshes = file
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic, Diagnostic::IgnoredMesh { .. }))
        .count();
    assert_eq!(
        meshes, 2,
        "one mesh warning per geometry: {:?}",
        file.diagnostics
    );
    let mut diagnostics = Vec::new();
    let baked = file.geometries[0].bake(&mut diagnostics).unwrap();
    assert_eq!(baked.texture_size, Vec2::splat(16.0));
    assert_eq!(baked.locators.len(), 1);
    assert_eq!(baked.locators[0].position, Vec3::new(-1.0, 0.0, 0.0));
    let duplicates = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic, Diagnostic::DuplicateLocator { .. }))
        .count();
    assert_eq!(duplicates, 2, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic, Diagnostic::DefaultTextureSize { .. })),
        "a width of 0 is not set: {diagnostics:?}"
    );
}
