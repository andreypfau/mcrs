//! The `minecraft:overworld` multi-noise preset.
//!
//! `worldgen/multi_noise_biome_source_parameter_list/overworld.json` names this
//! preset rather than carrying it, so the table has to be built here. It is
//! data, and the reference is the authority on it: the spans, the slice
//! boundaries and the biome grids below are transcribed from
//! `OverworldBiomeBuilder`, and the order entries are emitted in is part of the
//! data because ties in the climate search go to the earlier entry.

use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceKey;

use super::climate::{Parameter, ParameterList, ParameterPoint};
use crate::Biome;
use crate::keys::biome::*;

type BiomeKey = ResourceKey<Biome, &'static str>;

const VALLEY_SIZE: f32 = 0.05;
const LOW_START: f32 = 0.266_666_68;
const HIGH_START: f32 = 0.4;
const HIGH_END: f32 = 0.933_333_34;
const PEAK_START: f32 = 0.566_666_66;
const PEAK_END: f32 = 0.766_666_7;

const MIDDLE_BIOMES: [[BiomeKey; 5]; 5] = [
    [SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_TAIGA, TAIGA],
    [PLAINS, PLAINS, FOREST, TAIGA, OLD_GROWTH_SPRUCE_TAIGA],
    [FLOWER_FOREST, PLAINS, FOREST, BIRCH_FOREST, DARK_FOREST],
    [SAVANNA, SAVANNA, FOREST, JUNGLE, JUNGLE],
    [DESERT, DESERT, DESERT, DESERT, DESERT],
];

const MIDDLE_BIOMES_VARIANT: [[Option<BiomeKey>; 5]; 5] = [
    [Some(ICE_SPIKES), None, Some(SNOWY_TAIGA), None, None],
    [
        Some(DAPPLED_FOREST),
        None,
        None,
        None,
        Some(OLD_GROWTH_PINE_TAIGA),
    ],
    [
        Some(SUNFLOWER_PLAINS),
        None,
        None,
        Some(OLD_GROWTH_BIRCH_FOREST),
        None,
    ],
    [
        None,
        None,
        Some(PLAINS),
        Some(SPARSE_JUNGLE),
        Some(BAMBOO_JUNGLE),
    ],
    [None, None, None, None, None],
];

const PLATEAU_BIOMES: [[BiomeKey; 5]; 5] = [
    [
        SNOWY_PLAINS,
        SNOWY_PLAINS,
        SNOWY_PLAINS,
        SNOWY_TAIGA,
        SNOWY_TAIGA,
    ],
    [MEADOW, MEADOW, FOREST, TAIGA, OLD_GROWTH_SPRUCE_TAIGA],
    [MEADOW, MEADOW, MEADOW, MEADOW, PALE_GARDEN],
    [SAVANNA_PLATEAU, SAVANNA_PLATEAU, FOREST, FOREST, JUNGLE],
    [
        BADLANDS,
        BADLANDS,
        BADLANDS,
        WOODED_BADLANDS,
        WOODED_BADLANDS,
    ],
];

const PLATEAU_BIOMES_VARIANT: [[Option<BiomeKey>; 5]; 5] = [
    [Some(ICE_SPIKES), None, None, None, None],
    [
        Some(CHERRY_GROVE),
        None,
        Some(MEADOW),
        Some(MEADOW),
        Some(OLD_GROWTH_PINE_TAIGA),
    ],
    [
        Some(CHERRY_GROVE),
        Some(CHERRY_GROVE),
        Some(FOREST),
        Some(BIRCH_FOREST),
        None,
    ],
    [None, None, None, None, None],
    [
        Some(ERODED_BADLANDS),
        Some(ERODED_BADLANDS),
        None,
        None,
        None,
    ],
];

const SHATTERED_BIOMES: [[Option<BiomeKey>; 5]; 5] = [
    [
        Some(WINDSWEPT_GRAVELLY_HILLS),
        Some(WINDSWEPT_GRAVELLY_HILLS),
        Some(WINDSWEPT_HILLS),
        Some(WINDSWEPT_FOREST),
        Some(WINDSWEPT_FOREST),
    ],
    [
        Some(WINDSWEPT_GRAVELLY_HILLS),
        Some(WINDSWEPT_GRAVELLY_HILLS),
        Some(WINDSWEPT_HILLS),
        Some(WINDSWEPT_FOREST),
        Some(WINDSWEPT_FOREST),
    ],
    [
        Some(WINDSWEPT_HILLS),
        Some(WINDSWEPT_HILLS),
        Some(WINDSWEPT_HILLS),
        Some(WINDSWEPT_FOREST),
        Some(WINDSWEPT_FOREST),
    ],
    [None, None, None, None, None],
    [None, None, None, None, None],
];

const OCEANS: [[BiomeKey; 5]; 2] = [
    [
        DEEP_FROZEN_OCEAN,
        DEEP_COLD_OCEAN,
        DEEP_OCEAN,
        DEEP_LUKEWARM_OCEAN,
        WARM_OCEAN,
    ],
    [FROZEN_OCEAN, COLD_OCEAN, OCEAN, LUKEWARM_OCEAN, WARM_OCEAN],
];

struct Builder {
    full_range: Parameter,
    temperatures: [Parameter; 5],
    humidities: [Parameter; 5],
    erosions: [Parameter; 7],
    frozen: Parameter,
    unfrozen: Parameter,
    mushroom_fields: Parameter,
    deep_ocean: Parameter,
    ocean: Parameter,
    coast: Parameter,
    inland: Parameter,
    near_inland: Parameter,
    mid_inland: Parameter,
    far_inland: Parameter,
    out: Vec<(ParameterPoint, &'static str)>,
}

impl Builder {
    fn new() -> Self {
        let temperatures = [
            Parameter::span(-1.0, -0.45),
            Parameter::span(-0.45, -0.15),
            Parameter::span(-0.15, 0.2),
            Parameter::span(0.2, 0.55),
            Parameter::span(0.55, 1.0),
        ];
        Builder {
            full_range: Parameter::span(-1.0, 1.0),
            temperatures,
            humidities: [
                Parameter::span(-1.0, -0.35),
                Parameter::span(-0.35, -0.1),
                Parameter::span(-0.1, 0.1),
                Parameter::span(0.1, 0.3),
                Parameter::span(0.3, 1.0),
            ],
            erosions: [
                Parameter::span(-1.0, -0.78),
                Parameter::span(-0.78, -0.375),
                Parameter::span(-0.375, -0.2225),
                Parameter::span(-0.2225, 0.05),
                Parameter::span(0.05, 0.45),
                Parameter::span(0.45, 0.55),
                Parameter::span(0.55, 1.0),
            ],
            frozen: temperatures[0],
            unfrozen: temperatures[1].union(temperatures[4]),
            mushroom_fields: Parameter::span(-1.2, -1.05),
            deep_ocean: Parameter::span(-1.05, -0.455),
            ocean: Parameter::span(-0.455, -0.19),
            coast: Parameter::span(-0.19, -0.11),
            inland: Parameter::span(-0.11, 0.55),
            near_inland: Parameter::span(-0.11, 0.03),
            mid_inland: Parameter::span(0.03, 0.3),
            far_inland: Parameter::span(0.3, 1.0),
            out: Vec::new(),
        }
    }

    /// A surface biome is entered twice, once for each end of the depth range,
    /// so the search reaches it whether the column is at the surface or below.
    #[allow(clippy::too_many_arguments)]
    fn surface(
        &mut self,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: BiomeKey,
    ) {
        for depth in [Parameter::point(0.0), Parameter::point(1.0)] {
            self.emit(
                temperature,
                humidity,
                continentalness,
                erosion,
                depth,
                weirdness,
                offset,
                biome,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn underground(
        &mut self,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: BiomeKey,
    ) {
        self.emit(
            temperature,
            humidity,
            continentalness,
            erosion,
            Parameter::span(0.2, 0.9),
            weirdness,
            offset,
            biome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn bottom(
        &mut self,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: BiomeKey,
    ) {
        self.emit(
            temperature,
            humidity,
            continentalness,
            erosion,
            Parameter::point(1.1),
            weirdness,
            offset,
            biome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        depth: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: BiomeKey,
    ) {
        self.out.push((
            ParameterPoint {
                temperature,
                humidity,
                continentalness,
                erosion,
                depth,
                weirdness,
                offset: super::climate::quantize_coord(offset),
            },
            biome.as_static_str(),
        ));
    }

    fn middle(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        if weirdness.max < 0 {
            return MIDDLE_BIOMES[t][h];
        }
        MIDDLE_BIOMES_VARIANT[t][h].unwrap_or(MIDDLE_BIOMES[t][h])
    }

    fn badlands(&self, h: usize, weirdness: Parameter) -> BiomeKey {
        if h < 2 {
            if weirdness.max < 0 {
                BADLANDS
            } else {
                ERODED_BADLANDS
            }
        } else if h < 3 {
            BADLANDS
        } else {
            WOODED_BADLANDS
        }
    }

    fn middle_or_badlands_if_hot(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        if t == 4 {
            self.badlands(h, weirdness)
        } else {
            self.middle(t, h, weirdness)
        }
    }

    fn middle_or_badlands_if_hot_or_slope_if_cold(
        &self,
        t: usize,
        h: usize,
        weirdness: Parameter,
    ) -> BiomeKey {
        if t == 0 {
            self.slope(t, h, weirdness)
        } else {
            self.middle_or_badlands_if_hot(t, h, weirdness)
        }
    }

    fn maybe_windswept_savanna(
        &self,
        t: usize,
        h: usize,
        weirdness: Parameter,
        underlying: BiomeKey,
    ) -> BiomeKey {
        if t > 1 && h < 4 && weirdness.max >= 0 {
            WINDSWEPT_SAVANNA
        } else {
            underlying
        }
    }

    fn beach(&self, t: usize) -> BiomeKey {
        match t {
            0 => SNOWY_BEACH,
            4 => DESERT,
            _ => BEACH,
        }
    }

    fn shattered_coast(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        let underlying = if weirdness.max >= 0 {
            self.middle(t, h, weirdness)
        } else {
            self.beach(t)
        };
        self.maybe_windswept_savanna(t, h, weirdness, underlying)
    }

    fn plateau(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        if weirdness.max >= 0
            && let Some(variant) = PLATEAU_BIOMES_VARIANT[t][h]
        {
            return variant;
        }
        PLATEAU_BIOMES[t][h]
    }

    fn peak(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        if t <= 2 {
            if weirdness.max < 0 {
                JAGGED_PEAKS
            } else {
                FROZEN_PEAKS
            }
        } else if t == 3 {
            STONY_PEAKS
        } else {
            self.badlands(h, weirdness)
        }
    }

    fn slope(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        if t >= 3 {
            self.plateau(t, h, weirdness)
        } else if h <= 1 {
            SNOWY_SLOPES
        } else {
            GROVE
        }
    }

    fn shattered(&self, t: usize, h: usize, weirdness: Parameter) -> BiomeKey {
        SHATTERED_BIOMES[t][h].unwrap_or_else(|| self.middle(t, h, weirdness))
    }

    fn add_off_coast_biomes(&mut self) {
        let (full, mushroom) = (self.full_range, self.mushroom_fields);
        self.surface(full, full, mushroom, full, full, 0.0, MUSHROOM_FIELDS);
        for t in 0..5 {
            let temperature = self.temperatures[t];
            let deep = self.deep_ocean;
            let ocean = self.ocean;
            self.surface(temperature, full, deep, full, full, 0.0, OCEANS[0][t]);
            self.surface(temperature, full, ocean, full, full, 0.0, OCEANS[1][t]);
        }
    }

    fn add_inland_biomes(&mut self) {
        self.add_mid_slice(Parameter::span(-1.0, -HIGH_END));
        self.add_high_slice(Parameter::span(-HIGH_END, -PEAK_END));
        self.add_peaks(Parameter::span(-PEAK_END, -PEAK_START));
        self.add_high_slice(Parameter::span(-PEAK_START, -HIGH_START));
        self.add_mid_slice(Parameter::span(-HIGH_START, -LOW_START));
        self.add_low_slice(Parameter::span(-LOW_START, -VALLEY_SIZE));
        self.add_valleys(Parameter::span(-VALLEY_SIZE, VALLEY_SIZE));
        self.add_low_slice(Parameter::span(VALLEY_SIZE, LOW_START));
        self.add_mid_slice(Parameter::span(LOW_START, HIGH_START));
        self.add_high_slice(Parameter::span(HIGH_START, PEAK_START));
        self.add_peaks(Parameter::span(PEAK_START, PEAK_END));
        self.add_high_slice(Parameter::span(PEAK_END, HIGH_END));
        self.add_mid_slice(Parameter::span(HIGH_END, 1.0));
    }

    fn add_peaks(&mut self, weirdness: Parameter) {
        for t in 0..5 {
            for h in 0..5 {
                let temperature = self.temperatures[t];
                let humidity = self.humidities[h];
                let middle = self.middle(t, h, weirdness);
                let middle_or_badlands = self.middle_or_badlands_if_hot(t, h, weirdness);
                let middle_or_badlands_or_slope =
                    self.middle_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let plateau = self.plateau(t, h, weirdness);
                let shattered = self.shattered(t, h, weirdness);
                let shattered_or_savanna = self.maybe_windswept_savanna(t, h, weirdness, shattered);
                let peak = self.peak(t, h, weirdness);
                let (coast, near, mid, far) = (
                    self.coast,
                    self.near_inland,
                    self.mid_inland,
                    self.far_inland,
                );
                let (e0, e1, e2, e3, e4, e5, e6) = self.erosion_septet();

                self.surface(
                    temperature,
                    humidity,
                    coast.union(far),
                    e0,
                    weirdness,
                    0.0,
                    peak,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e1,
                    weirdness,
                    0.0,
                    middle_or_badlands_or_slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e1,
                    weirdness,
                    0.0,
                    peak,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e2.union(e3),
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e2,
                    weirdness,
                    0.0,
                    plateau,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid,
                    e3,
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.surface(temperature, humidity, far, e3, weirdness, 0.0, plateau);
                self.surface(
                    temperature,
                    humidity,
                    coast.union(far),
                    e4,
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e5,
                    weirdness,
                    0.0,
                    shattered_or_savanna,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e5,
                    weirdness,
                    0.0,
                    shattered,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(far),
                    e6,
                    weirdness,
                    0.0,
                    middle,
                );
            }
        }
    }

    fn add_high_slice(&mut self, weirdness: Parameter) {
        for t in 0..5 {
            for h in 0..5 {
                let temperature = self.temperatures[t];
                let humidity = self.humidities[h];
                let middle = self.middle(t, h, weirdness);
                let middle_or_badlands = self.middle_or_badlands_if_hot(t, h, weirdness);
                let middle_or_badlands_or_slope =
                    self.middle_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let plateau = self.plateau(t, h, weirdness);
                let shattered = self.shattered(t, h, weirdness);
                let middle_or_savanna = self.maybe_windswept_savanna(t, h, weirdness, middle);
                let slope = self.slope(t, h, weirdness);
                let peak = self.peak(t, h, weirdness);
                let (coast, near, mid, far) = (
                    self.coast,
                    self.near_inland,
                    self.mid_inland,
                    self.far_inland,
                );
                let (e0, e1, e2, e3, e4, e5, e6) = self.erosion_septet();

                self.surface(
                    temperature,
                    humidity,
                    coast,
                    e0.union(e1),
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(temperature, humidity, near, e0, weirdness, 0.0, slope);
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e0,
                    weirdness,
                    0.0,
                    peak,
                );
                self.surface(
                    temperature,
                    humidity,
                    near,
                    e1,
                    weirdness,
                    0.0,
                    middle_or_badlands_or_slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e1,
                    weirdness,
                    0.0,
                    slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e2.union(e3),
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e2,
                    weirdness,
                    0.0,
                    plateau,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid,
                    e3,
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.surface(temperature, humidity, far, e3, weirdness, 0.0, plateau);
                self.surface(
                    temperature,
                    humidity,
                    coast.union(far),
                    e4,
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e5,
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e5,
                    weirdness,
                    0.0,
                    shattered,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast.union(far),
                    e6,
                    weirdness,
                    0.0,
                    middle,
                );
            }
        }
    }

    fn add_mid_slice(&mut self, weirdness: Parameter) {
        let full = self.full_range;
        let (coast, near, mid, far) = (
            self.coast,
            self.near_inland,
            self.mid_inland,
            self.far_inland,
        );
        let (e0, e1, e2, e3, e4, e5, e6) = self.erosion_septet();
        let temperate = self.temperatures[1].union(self.temperatures[2]);
        let hot = self.temperatures[3].union(self.temperatures[4]);

        self.surface(full, full, coast, e0.union(e2), weirdness, 0.0, STONY_SHORE);
        self.surface(temperate, full, near.union(far), e6, weirdness, 0.0, SWAMP);
        self.surface(
            hot,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            MANGROVE_SWAMP,
        );

        for t in 0..5 {
            for h in 0..5 {
                let temperature = self.temperatures[t];
                let humidity = self.humidities[h];
                let middle = self.middle(t, h, weirdness);
                let middle_or_badlands = self.middle_or_badlands_if_hot(t, h, weirdness);
                let middle_or_badlands_or_slope =
                    self.middle_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let shattered = self.shattered(t, h, weirdness);
                let plateau = self.plateau(t, h, weirdness);
                let beach = self.beach(t);
                let middle_or_savanna = self.maybe_windswept_savanna(t, h, weirdness, middle);
                let shattered_coast = self.shattered_coast(t, h, weirdness);
                let slope = self.slope(t, h, weirdness);

                self.surface(
                    temperature,
                    humidity,
                    near.union(far),
                    e0,
                    weirdness,
                    0.0,
                    slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    near.union(mid),
                    e1,
                    weirdness,
                    0.0,
                    middle_or_badlands_or_slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    far,
                    e1,
                    weirdness,
                    0.0,
                    if t == 0 { slope } else { plateau },
                );
                self.surface(temperature, humidity, near, e2, weirdness, 0.0, middle);
                self.surface(
                    temperature,
                    humidity,
                    mid,
                    e2,
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.surface(temperature, humidity, far, e2, weirdness, 0.0, plateau);
                self.surface(
                    temperature,
                    humidity,
                    coast.union(near),
                    e3,
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e3,
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                if weirdness.max < 0 {
                    self.surface(temperature, humidity, coast, e4, weirdness, 0.0, beach);
                    self.surface(
                        temperature,
                        humidity,
                        near.union(far),
                        e4,
                        weirdness,
                        0.0,
                        middle,
                    );
                } else {
                    self.surface(
                        temperature,
                        humidity,
                        coast.union(far),
                        e4,
                        weirdness,
                        0.0,
                        middle,
                    );
                }
                self.surface(
                    temperature,
                    humidity,
                    coast,
                    e5,
                    weirdness,
                    0.0,
                    shattered_coast,
                );
                self.surface(
                    temperature,
                    humidity,
                    near,
                    e5,
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e5,
                    weirdness,
                    0.0,
                    shattered,
                );
                if weirdness.max < 0 {
                    self.surface(temperature, humidity, coast, e6, weirdness, 0.0, beach);
                } else {
                    self.surface(temperature, humidity, coast, e6, weirdness, 0.0, middle);
                }
                if t == 0 {
                    self.surface(
                        temperature,
                        humidity,
                        near.union(far),
                        e6,
                        weirdness,
                        0.0,
                        middle,
                    );
                }
            }
        }
    }

    fn add_low_slice(&mut self, weirdness: Parameter) {
        let full = self.full_range;
        let (coast, near, mid, far) = (
            self.coast,
            self.near_inland,
            self.mid_inland,
            self.far_inland,
        );
        let (e0, e1, e2, e3, e4, e5, e6) = self.erosion_septet();
        let temperate = self.temperatures[1].union(self.temperatures[2]);
        let hot = self.temperatures[3].union(self.temperatures[4]);

        self.surface(full, full, coast, e0.union(e2), weirdness, 0.0, STONY_SHORE);
        self.surface(temperate, full, near.union(far), e6, weirdness, 0.0, SWAMP);
        self.surface(
            hot,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            MANGROVE_SWAMP,
        );

        for t in 0..5 {
            for h in 0..5 {
                let temperature = self.temperatures[t];
                let humidity = self.humidities[h];
                let middle = self.middle(t, h, weirdness);
                let middle_or_badlands = self.middle_or_badlands_if_hot(t, h, weirdness);
                let middle_or_badlands_or_slope =
                    self.middle_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let beach = self.beach(t);
                let middle_or_savanna = self.maybe_windswept_savanna(t, h, weirdness, middle);
                let shattered_coast = self.shattered_coast(t, h, weirdness);

                self.surface(
                    temperature,
                    humidity,
                    near,
                    e0.union(e1),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e0.union(e1),
                    weirdness,
                    0.0,
                    middle_or_badlands_or_slope,
                );
                self.surface(
                    temperature,
                    humidity,
                    near,
                    e2.union(e3),
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e2.union(e3),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast,
                    e3.union(e4),
                    weirdness,
                    0.0,
                    beach,
                );
                self.surface(
                    temperature,
                    humidity,
                    near.union(far),
                    e4,
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(
                    temperature,
                    humidity,
                    coast,
                    e5,
                    weirdness,
                    0.0,
                    shattered_coast,
                );
                self.surface(
                    temperature,
                    humidity,
                    near,
                    e5,
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e5,
                    weirdness,
                    0.0,
                    middle,
                );
                self.surface(temperature, humidity, coast, e6, weirdness, 0.0, beach);
                if t == 0 {
                    self.surface(
                        temperature,
                        humidity,
                        near.union(far),
                        e6,
                        weirdness,
                        0.0,
                        middle,
                    );
                }
            }
        }
    }

    fn add_valleys(&mut self, weirdness: Parameter) {
        let full = self.full_range;
        let (frozen, unfrozen) = (self.frozen, self.unfrozen);
        let (coast, near, inland, mid, far) = (
            self.coast,
            self.near_inland,
            self.inland,
            self.mid_inland,
            self.far_inland,
        );
        let (e0, e1, e2, _e3, _e4, e5, e6) = self.erosion_septet();
        let cold_shore = if weirdness.max < 0 {
            STONY_SHORE
        } else {
            FROZEN_RIVER
        };
        let warm_shore = if weirdness.max < 0 {
            STONY_SHORE
        } else {
            RIVER
        };

        self.surface(
            frozen,
            full,
            coast,
            e0.union(e1),
            weirdness,
            0.0,
            cold_shore,
        );
        self.surface(
            unfrozen,
            full,
            coast,
            e0.union(e1),
            weirdness,
            0.0,
            warm_shore,
        );
        self.surface(
            frozen,
            full,
            near,
            e0.union(e1),
            weirdness,
            0.0,
            FROZEN_RIVER,
        );
        self.surface(unfrozen, full, near, e0.union(e1), weirdness, 0.0, RIVER);
        self.surface(
            frozen,
            full,
            coast.union(far),
            e2.union(e5),
            weirdness,
            0.0,
            FROZEN_RIVER,
        );
        self.surface(
            unfrozen,
            full,
            coast.union(far),
            e2.union(e5),
            weirdness,
            0.0,
            RIVER,
        );
        self.surface(frozen, full, coast, e6, weirdness, 0.0, FROZEN_RIVER);
        self.surface(unfrozen, full, coast, e6, weirdness, 0.0, RIVER);
        let temperate = self.temperatures[1].union(self.temperatures[2]);
        let hot = self.temperatures[3].union(self.temperatures[4]);
        self.surface(
            temperate,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            SWAMP,
        );
        self.surface(
            hot,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            MANGROVE_SWAMP,
        );
        self.surface(
            frozen,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            FROZEN_RIVER,
        );

        for t in 0..5 {
            for h in 0..5 {
                let temperature = self.temperatures[t];
                let humidity = self.humidities[h];
                let middle_or_badlands = self.middle_or_badlands_if_hot(t, h, weirdness);
                self.surface(
                    temperature,
                    humidity,
                    mid.union(far),
                    e0.union(e1),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
            }
        }
    }

    fn add_underground_biomes(&mut self) {
        let full = self.full_range;
        let (coast, inland) = (self.coast, self.inland);
        let (e0, e1, _e2, _e3, _e4, e5, e6) = self.erosion_septet();
        self.underground(
            full,
            Parameter::span(-1.0, 0.7),
            Parameter::span(0.8, 1.0),
            full,
            full,
            0.0,
            DRIPSTONE_CAVES,
        );
        self.underground(
            full,
            Parameter::span(0.7, 1.0),
            full,
            full,
            full,
            0.0,
            LUSH_CAVES,
        );
        self.underground(
            full,
            Parameter::span(-1.0, 0.7),
            coast.union(inland),
            e5.union(e6),
            Parameter::span(-1.1, -0.85),
            0.0,
            SULFUR_CAVES,
        );
        self.bottom(full, full, full, e0.union(e1), full, 0.0, DEEP_DARK);
    }

    fn erosion_septet(
        &self,
    ) -> (
        Parameter,
        Parameter,
        Parameter,
        Parameter,
        Parameter,
        Parameter,
        Parameter,
    ) {
        let e = self.erosions;
        (e[0], e[1], e[2], e[3], e[4], e[5], e[6])
    }
}

/// The overworld climate table, in the order the reference emits it.
pub fn overworld_parameter_list() -> &'static ParameterList<&'static str> {
    static LIST: LazyLock<ParameterList<&'static str>> = LazyLock::new(|| {
        let mut builder = Builder::new();
        builder.add_off_coast_biomes();
        builder.add_inland_biomes();
        builder.add_underground_biomes();
        ParameterList::new(builder.out)
    });
    &LIST
}

/// The `minecraft:nether` preset, which is small enough to be a literal.
pub fn nether_parameter_list() -> &'static ParameterList<&'static str> {
    static LIST: LazyLock<ParameterList<&'static str>> = LazyLock::new(|| {
        let point = Parameter::point(0.0);
        let entry = |temperature: Parameter, humidity: Parameter, offset: f32, biome: BiomeKey| {
            (
                ParameterPoint {
                    temperature,
                    humidity,
                    continentalness: point,
                    erosion: point,
                    depth: point,
                    weirdness: point,
                    offset: super::climate::quantize_coord(offset),
                },
                biome.as_static_str(),
            )
        };
        ParameterList::new(vec![
            entry(point, point, 0.0, NETHER_WASTES),
            entry(
                Parameter::point(0.0),
                Parameter::point(-0.5),
                0.0,
                SOUL_SAND_VALLEY,
            ),
            entry(
                Parameter::point(0.4),
                Parameter::point(0.0),
                0.0,
                CRIMSON_FOREST,
            ),
            entry(
                Parameter::point(0.0),
                Parameter::point(0.5),
                0.375,
                WARPED_FOREST,
            ),
            entry(
                Parameter::point(-0.5),
                Parameter::point(0.0),
                0.175,
                BASALT_DELTAS,
            ),
        ])
    });
    &LIST
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::climate::TargetPoint;
    use std::collections::BTreeSet;

    /// An independent check on the whole transcription: a biome carries the
    /// overworld carver set exactly when the overworld climate table can
    /// produce it. The two sets are written down in different places — one in
    /// the shipped biome JSON, one in this table — so agreeing is evidence
    /// neither is wrong.
    #[test]
    fn the_table_names_exactly_the_biomes_with_overworld_carvers() {
        let with_overworld_carvers: BTreeSet<String> = mcrs_minecraft_worldgen_testing::registry::<
            mcrs_minecraft_biome_file::BiomeFile,
        >("biome")
        .into_iter()
        .filter(|(_, biome)| {
            mcrs_minecraft_worldgen_testing::names_of(&biome.carvers)
                .iter()
                .any(|carver| carver == mcrs_minecraft_worldgen_carver::keys::carver::CAVE.as_str())
        })
        .map(|(id, _)| id.as_str().to_owned())
        .collect();

        let table = overworld_parameter_list();
        // Every climate cell of every slice, twice for the depth ends: 22
        // off-coast, 1100 peak, 2600 high, 1432 + 1332 mid, 1032 low, 72
        // valley and the 4 underground entries.
        assert_eq!(table.len(), 7594);
        let named: BTreeSet<String> = table
            .values()
            .iter()
            .map(|(_, biome)| (*biome).to_owned())
            .collect();
        assert_eq!(named.len(), 56);
        assert_eq!(named, with_overworld_carvers);
    }

    #[test]
    fn the_climate_search_lands_on_the_expected_biomes() {
        let table = overworld_parameter_list();
        let at = |temperature, humidity, continentalness, erosion, depth, weirdness| {
            *table.find_value(TargetPoint::new(
                temperature,
                humidity,
                continentalness,
                erosion,
                depth,
                weirdness,
            ))
        };
        assert_eq!(
            at(-0.8, 0.0, -1.1, 0.0, 0.0, 0.0),
            crate::keys::biome::MUSHROOM_FIELDS.as_static_str()
        );
        assert_eq!(
            at(-0.8, 0.0, -0.8, 0.0, 0.0, 0.0),
            crate::keys::biome::DEEP_FROZEN_OCEAN.as_static_str()
        );
        assert_eq!(
            at(0.8, 0.0, -0.8, 0.0, 0.0, 0.0),
            crate::keys::biome::WARM_OCEAN.as_static_str()
        );
        assert_eq!(
            at(-0.8, 0.0, -0.3, 0.0, 0.0, 0.0),
            crate::keys::biome::FROZEN_OCEAN.as_static_str()
        );
        assert_eq!(
            at(0.0, 0.0, 0.0, -0.9, 1.1, 0.0),
            crate::keys::biome::DEEP_DARK.as_static_str()
        );
        assert_eq!(
            at(0.0, 0.8, 0.0, 0.0, 0.5, 0.0),
            crate::keys::biome::LUSH_CAVES.as_static_str()
        );
        assert_eq!(
            at(0.0, 0.0, 0.9, 0.0, 0.5, 0.0),
            crate::keys::biome::DRIPSTONE_CAVES.as_static_str()
        );
        assert_eq!(
            at(0.8, 0.0, 0.5, 0.3, 0.0, -0.5),
            crate::keys::biome::DESERT.as_static_str()
        );
    }

    #[test]
    fn the_nether_table_is_the_five_reference_entries() {
        let table = nether_parameter_list();
        assert_eq!(table.len(), 5);
        let at = |temperature, humidity| {
            *table.find_value(TargetPoint::new(temperature, humidity, 0.0, 0.0, 0.0, 0.0))
        };
        assert_eq!(
            at(0.0, 0.0),
            crate::keys::biome::NETHER_WASTES.as_static_str()
        );
        assert_eq!(
            at(0.0, -0.5),
            crate::keys::biome::SOUL_SAND_VALLEY.as_static_str()
        );
        assert_eq!(
            at(0.4, 0.0),
            crate::keys::biome::CRIMSON_FOREST.as_static_str()
        );
        assert_eq!(
            at(0.0, 0.5),
            crate::keys::biome::WARPED_FOREST.as_static_str()
        );
        assert_eq!(
            at(-0.5, 0.0),
            crate::keys::biome::BASALT_DELTAS.as_static_str()
        );
    }

    /// Weirdness picks between a slice's plain biome and its variant, so the
    /// same climate on either side of zero must be able to differ.
    #[test]
    fn weirdness_selects_the_variant_grid() {
        let table = overworld_parameter_list();
        let at =
            |weirdness| *table.find_value(TargetPoint::new(-0.8, -0.8, 0.2, 0.2, 0.0, weirdness));
        assert_eq!(at(-0.2), crate::keys::biome::SNOWY_PLAINS.as_static_str());
        assert_eq!(at(0.2), crate::keys::biome::ICE_SPIKES.as_static_str());
    }
}
