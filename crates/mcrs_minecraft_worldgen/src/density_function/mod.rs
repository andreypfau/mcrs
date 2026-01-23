use crate::noise::normal_noise::NormalNoise;

pub mod proto;

pub enum NoiseFunction {
    Constant,
    Noise(NormalNoise),
}
