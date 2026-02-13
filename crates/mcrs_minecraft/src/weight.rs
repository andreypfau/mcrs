#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Weighted<T: PartialEq> {
    data: T,
    weight: f32,
}
