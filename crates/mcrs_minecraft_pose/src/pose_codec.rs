use std::io::Write;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::Result;
use mcrs_minecraft_wire::VarInt;

use crate::Pose;

const POSES: [Pose; 18] = [
    Pose::Standing,
    Pose::FallFlying,
    Pose::Sleeping,
    Pose::Swimming,
    Pose::SpinAttack,
    Pose::Crouching,
    Pose::LongJumping,
    Pose::Dying,
    Pose::Croaking,
    Pose::UsingTongue,
    Pose::Sitting,
    Pose::Roaring,
    Pose::Sniffing,
    Pose::Emerging,
    Pose::Digging,
    Pose::Sliding,
    Pose::Shooting,
    Pose::Inhaling,
];

impl Encode for Pose {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(*self as i32).encode(w)
    }
}

impl Decode<'_> for Pose {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let id = VarInt::decode(r)?.0;
        let pose = usize::try_from(id)
            .ok()
            .and_then(|index| POSES.get(index))
            .copied()
            .unwrap_or_default();
        Ok(pose)
    }
}
