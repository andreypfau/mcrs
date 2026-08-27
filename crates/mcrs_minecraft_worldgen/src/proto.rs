use crate::density_function::proto::{DensityFunctionHolder, HashableF64, ProtoDensityFunction};
use mcrs_minecraft_core::ResourceLocation;

#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,
    pub material_rule: ResourceLocation,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub aquifers: Option<Aquifers>,
    pub legacy_random_source: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub debug_functions: Vec<DebugFunction>,
}

pub type SpawnTargetPoint = std::collections::BTreeMap<ResourceLocation, Interval<HashableF64>>;

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Aquifers {
    pub barrier: DensityFunctionHolder,
    pub exclusion: DensityFunctionHolder,
    pub fluid_level_floodedness: DensityFunctionHolder,
    pub fluid_level_spread: DensityFunctionHolder,
    pub lava: DensityFunctionHolder,
    pub surface_level: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct DebugFunction {
    pub label: String,
    pub function: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: u32,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct NoiseRouter {
    pub temperature: DensityFunctionHolder,
    pub vegetation: DensityFunctionHolder,
    pub continents: DensityFunctionHolder,
    pub erosion: DensityFunctionHolder,
    pub depth: DensityFunctionHolder,
    pub ridges: DensityFunctionHolder,
    pub chunk_surface_level: DensityFunctionHolder,
    pub final_density: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Debug, Clone)]
pub struct BlockState {
    pub name: ResourceLocation,
    pub properties: Option<std::collections::BTreeMap<String, String>>,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchedBlockState {
    id: ResourceLocation,
    #[serde(default)]
    properties: std::collections::BTreeMap<String, String>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BlockState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Either::<ResourceLocation, DispatchedBlockState>::deserialize(deserializer)? {
            Either::Left(name) => Ok(BlockState {
                name,
                properties: None,
            }),
            Either::Right(state) => Ok(BlockState {
                name: state.id,
                properties: Some(state.properties),
            }),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for BlockState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.properties {
            None => self.name.serialize(serializer),
            Some(properties) => DispatchedBlockState {
                id: self.name.clone(),
                properties: properties.clone(),
            }
            .serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(from = "Either<I, Either<[I; 2], InternalInterval<I>>>")]
#[serde(into = "Either<I, Either<[I; 2], InternalInterval<I>>>")]
pub struct Interval<I>
where
    I: Clone + PartialEq,
{
    pub(crate) min: I,
    pub(crate) max: I,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
struct InternalInterval<I> {
    min: I,
    max: I,
}

impl<I: Clone + PartialEq> From<Either<I, Either<[I; 2], InternalInterval<I>>>> for Interval<I> {
    fn from(value: Either<I, Either<[I; 2], InternalInterval<I>>>) -> Self {
        match value {
            Either::Left(i) => Interval {
                min: i.clone(),
                max: i,
            },
            Either::Right(Either::Left([min, max])) => Interval { min, max },
            Either::Right(Either::Right(i)) => Interval {
                min: i.min,
                max: i.max,
            },
        }
    }
}

impl<I: Clone + PartialEq> From<Interval<I>> for Either<I, Either<[I; 2], InternalInterval<I>>> {
    fn from(value: Interval<I>) -> Self {
        if value.min == value.max {
            Either::Left(value.min)
        } else {
            Either::Right(Either::Left([value.min, value.max]))
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    pub fn left(&self) -> Option<&L> {
        match self {
            Either::Left(l) => Some(l),
            _ => None,
        }
    }

    pub fn right(&self) -> Option<&R> {
        match self {
            Either::Right(r) => Some(r),
            _ => None,
        }
    }

    pub fn map<LT, RT, T>(self, left: LT, right: RT) -> T
    where
        LT: FnOnce(L) -> T,
        RT: FnOnce(R) -> T,
    {
        match self {
            Either::Left(l) => left(l),
            Either::Right(r) => right(r),
        }
    }

    pub fn swap(self) -> Either<R, L> {
        match self {
            Either::Left(l) => Either::Right(l),
            Either::Right(r) => Either::Left(r),
        }
    }

    pub fn flat_map<L2>(self, f: impl FnOnce(L) -> Either<L2, R>) -> Either<L2, R> {
        self.map(f, Either::Right)
    }
}
