use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

#[test]
fn decodes_each_id_and_reads_an_unknown_id_as_standing() {
    let cases = [
        (0, Pose::Standing),
        (1, Pose::FallFlying),
        (5, Pose::Crouching),
        (7, Pose::Dying),
        (17, Pose::Inhaling),
        (18, Pose::Standing),
        (1000, Pose::Standing),
        (-1, Pose::Standing),
    ];
    for (id, expected) in cases {
        let mut bytes = Vec::new();
        VarInt(id).encode(&mut bytes).unwrap();
        let mut reader = bytes.as_slice();
        assert_eq!(Pose::decode(&mut reader).unwrap(), expected, "id {id}");
        assert!(reader.is_empty(), "id {id} leaves bytes");
    }
}

#[test]
fn encodes_the_wire_id() {
    for (id, pose) in [
        (0, Pose::Standing),
        (4, Pose::SpinAttack),
        (17, Pose::Inhaling),
    ] {
        let mut bytes = Vec::new();
        pose.encode(&mut bytes).unwrap();
        assert_eq!(bytes, [id], "{pose:?}");
    }
}
