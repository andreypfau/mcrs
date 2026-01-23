use crate::climate::ParamPoint;
use crate::density_function::proto::{DensityFunctionHolder, ProtoDensityFunction};
use mcrs_protocol::Ident;

#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,
    pub surface_rule: SurfaceRule,
    pub spawn_target: Vec<ParamPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    pub aquifers_enabled: bool,
    pub ore_veins_enabled: bool,
    pub legacy_random_source: bool,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: u32,
    pub size_horizontal: u8,
    pub size_vertical: u8,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoiseRouter {
    pub barrier: DensityFunctionHolder,
    pub fluid_level_floodedness: DensityFunctionHolder,
    pub fluid_level_spread: DensityFunctionHolder,
    pub lava: DensityFunctionHolder,
    pub temperature: DensityFunctionHolder,
    pub vegetation: DensityFunctionHolder,
    pub continents: DensityFunctionHolder,
    pub erosion: DensityFunctionHolder,
    pub depth: DensityFunctionHolder,
    pub ridges: DensityFunctionHolder,
    pub preliminary_surface_level: DensityFunctionHolder,
    pub final_density: DensityFunctionHolder,
    pub vein_toggle: DensityFunctionHolder,
    pub vein_ridged: DensityFunctionHolder,
    pub vein_gap: DensityFunctionHolder,
}

#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(tag = "type")]
pub enum SurfaceRule {
    #[serde(rename = "minecraft:bandlands")]
    Bandlands,
    #[serde(rename = "minecraft:block")]
    Block { result_state: BlockState },
    #[serde(rename = "minecraft:sequence")]
    Sequence { sequence: Vec<SurfaceRule> },
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: Box<ConditionSource>,
        then_run: Box<SurfaceRule>,
    },
}

#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(tag = "type")]
pub enum ConditionSource {
    #[serde(rename = "minecraft:biome")]
    Biome { biome_is: Vec<Ident<String>> },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: Ident<String>,
        min_threshold: f64,
        max_threshold: f64,
    },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: Ident<String>,
        true_at_and_below: VerticalAnchor,
        false_at_and_above: VerticalAnchor,
    },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i8,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i8,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:temperature")]
    Temperature,
    #[serde(rename = "minecraft:steep")]
    Steep,
    #[serde(rename = "minecraft:not")]
    Not { invert: Box<ConditionSource> },
    #[serde(rename = "minecraft:hole")]
    Hole,
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface,
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: CaveSurface,
    },
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(untagged)]
pub enum VerticalAnchor {
    Absolute { absolute: i32 },
    AboveBottom { above_bottom: i32 },
    BelowTop { below_top: i32 },
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(rename_all = "lowercase")]
pub enum CaveSurface {
    Ceiling,
    Floor,
}

#[derive(Hash, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockState {
    #[cfg(feature = "serde")]
    #[serde(rename = "Name")]
    pub name: Ident<String>,
    #[cfg(feature = "serde")]
    #[serde(rename = "Properties")]
    pub properties: Option<std::collections::BTreeMap<String, String>>,
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

    pub fn map_left<F, T>(self, f: F) -> Either<T, R>
    where
        F: FnOnce(L) -> T,
    {
        match self {
            Either::Left(l) => Either::Left(f(l)),
            Either::Right(r) => Either::Right(r),
        }
    }

    pub fn map_right<F, T>(self, f: F) -> Either<L, T>
    where
        F: FnOnce(R) -> T,
    {
        match self {
            Either::Left(l) => Either::Left(l),
            Either::Right(r) => Either::Right(f(r)),
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
