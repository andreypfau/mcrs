use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateParameters {
    pub temperature: ParameterRange,
    pub humidity: ParameterRange,
    pub continentalness: ParameterRange,
    pub erosion: ParameterRange,
    pub depth: ParameterRange,
    pub weirdness: ParameterRange,
    pub offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterRange {
    Point(f64),
    Range([f64; 2]),
}
