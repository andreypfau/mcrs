use uuid::Uuid;

use mcrs_minecraft_core::codec::IntArray;

pub fn uuid_ints(id: Uuid) -> IntArray<4> {
    let (msb, lsb) = id.as_u64_pair();
    IntArray([
        (msb >> 32) as i32,
        msb as i32,
        (lsb >> 32) as i32,
        lsb as i32,
    ])
}

pub fn ints_uuid(IntArray([a, b, c, d]): IntArray<4>) -> Uuid {
    let half = |high: i32, low: i32| ((high as u32 as u64) << 32) | low as u32 as u64;
    Uuid::from_u64_pair(half(a, b), half(c, d))
}
