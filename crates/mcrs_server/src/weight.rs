
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Weighted<T> {
    data: T,
    weight: f32,
}

