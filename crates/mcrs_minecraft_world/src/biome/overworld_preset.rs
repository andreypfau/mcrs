//! The `minecraft:overworld` multi-noise preset.
//!
//! `worldgen/multi_noise_biome_source_parameter_list/overworld.json` names this
//! preset rather than carrying it, so the table has to be built here. It is
//! data, and the reference is the authority on it: the spans, the slice
//! boundaries and the biome grids below are transcribed from
//! `OverworldBiomeBuilder`, and the order entries are emitted in is part of the
//! data because ties in the climate search go to the earlier entry.

use super::climate::{Parameter, ParameterList, ParameterPoint};

const VALLEY_SIZE: f32 = 0.05;
const LOW_START: f32 = 0.266_666_68;
const HIGH_START: f32 = 0.4;
const HIGH_END: f32 = 0.933_333_34;
const PEAK_START: f32 = 0.566_666_66;
const PEAK_END: f32 = 0.766_666_7;

const MIDDLE_BIOMES: [[&str; 5]; 5] = [
    [
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_taiga",
        "minecraft:taiga",
    ],
    [
        "minecraft:plains",
        "minecraft:plains",
        "minecraft:forest",
        "minecraft:taiga",
        "minecraft:old_growth_spruce_taiga",
    ],
    [
        "minecraft:flower_forest",
        "minecraft:plains",
        "minecraft:forest",
        "minecraft:birch_forest",
        "minecraft:dark_forest",
    ],
    [
        "minecraft:savanna",
        "minecraft:savanna",
        "minecraft:forest",
        "minecraft:jungle",
        "minecraft:jungle",
    ],
    [
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
    ],
];

const MIDDLE_BIOMES_VARIANT: [[Option<&str>; 5]; 5] = [
    [
        Some("minecraft:ice_spikes"),
        None,
        Some("minecraft:snowy_taiga"),
        None,
        None,
    ],
    [
        Some("minecraft:dappled_forest"),
        None,
        None,
        None,
        Some("minecraft:old_growth_pine_taiga"),
    ],
    [
        Some("minecraft:sunflower_plains"),
        None,
        None,
        Some("minecraft:old_growth_birch_forest"),
        None,
    ],
    [
        None,
        None,
        Some("minecraft:plains"),
        Some("minecraft:sparse_jungle"),
        Some("minecraft:bamboo_jungle"),
    ],
    [None, None, None, None, None],
];

const PLATEAU_BIOMES: [[&str; 5]; 5] = [
    [
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_taiga",
        "minecraft:snowy_taiga",
    ],
    [
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:forest",
        "minecraft:taiga",
        "minecraft:old_growth_spruce_taiga",
    ],
    [
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:pale_garden",
    ],
    [
        "minecraft:savanna_plateau",
        "minecraft:savanna_plateau",
        "minecraft:forest",
        "minecraft:forest",
        "minecraft:jungle",
    ],
    [
        "minecraft:badlands",
        "minecraft:badlands",
        "minecraft:badlands",
        "minecraft:wooded_badlands",
        "minecraft:wooded_badlands",
    ],
];

const PLATEAU_BIOMES_VARIANT: [[Option<&str>; 5]; 5] = [
    [Some("minecraft:ice_spikes"), None, None, None, None],
    [
        Some("minecraft:cherry_grove"),
        None,
        Some("minecraft:meadow"),
        Some("minecraft:meadow"),
        Some("minecraft:old_growth_pine_taiga"),
    ],
    [
        Some("minecraft:cherry_grove"),
        Some("minecraft:cherry_grove"),
        Some("minecraft:forest"),
        Some("minecraft:birch_forest"),
        None,
    ],
    [None, None, None, None, None],
    [
        Some("minecraft:eroded_badlands"),
        Some("minecraft:eroded_badlands"),
        None,
        None,
        None,
    ],
];

const SHATTERED_BIOMES: [[Option<&str>; 5]; 5] = [
    [
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [None, None, None, None, None],
    [None, None, None, None, None],
];

const OCEANS: [[&str; 5]; 2] = [
    [
        "minecraft:deep_frozen_ocean",
        "minecraft:deep_cold_ocean",
        "minecraft:deep_ocean",
        "minecraft:deep_lukewarm_ocean",
        "minecraft:warm_ocean",
    ],
    [
        "minecraft:frozen_ocean",
        "minecraft:cold_ocean",
        "minecraft:ocean",
        "minecraft:lukewarm_ocean",
        "minecraft:warm_ocean",
    ],
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
        biome: &'static str,
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
        biome: &'static str,
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
        biome: &'static str,
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
        biome: &'static str,
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
            biome,
        ));
    }

    fn middle(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        if weirdness.max < 0 {
            return MIDDLE_BIOMES[t][h];
        }
        MIDDLE_BIOMES_VARIANT[t][h].unwrap_or(MIDDLE_BIOMES[t][h])
    }

    fn badlands(&self, h: usize, weirdness: Parameter) -> &'static str {
        if h < 2 {
            if weirdness.max < 0 {
                "minecraft:badlands"
            } else {
                "minecraft:eroded_badlands"
            }
        } else if h < 3 {
            "minecraft:badlands"
        } else {
            "minecraft:wooded_badlands"
        }
    }

    fn middle_or_badlands_if_hot(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
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
    ) -> &'static str {
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
        underlying: &'static str,
    ) -> &'static str {
        if t > 1 && h < 4 && weirdness.max >= 0 {
            "minecraft:windswept_savanna"
        } else {
            underlying
        }
    }

    fn beach(&self, t: usize) -> &'static str {
        match t {
            0 => "minecraft:snowy_beach",
            4 => "minecraft:desert",
            _ => "minecraft:beach",
        }
    }

    fn shattered_coast(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        let underlying = if weirdness.max >= 0 {
            self.middle(t, h, weirdness)
        } else {
            self.beach(t)
        };
        self.maybe_windswept_savanna(t, h, weirdness, underlying)
    }

    fn plateau(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        if weirdness.max >= 0
            && let Some(variant) = PLATEAU_BIOMES_VARIANT[t][h]
        {
            return variant;
        }
        PLATEAU_BIOMES[t][h]
    }

    fn peak(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        if t <= 2 {
            if weirdness.max < 0 {
                "minecraft:jagged_peaks"
            } else {
                "minecraft:frozen_peaks"
            }
        } else if t == 3 {
            "minecraft:stony_peaks"
        } else {
            self.badlands(h, weirdness)
        }
    }

    fn slope(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        if t >= 3 {
            self.plateau(t, h, weirdness)
        } else if h <= 1 {
            "minecraft:snowy_slopes"
        } else {
            "minecraft:grove"
        }
    }

    fn shattered(&self, t: usize, h: usize, weirdness: Parameter) -> &'static str {
        SHATTERED_BIOMES[t][h].unwrap_or_else(|| self.middle(t, h, weirdness))
    }

    fn add_off_coast_biomes(&mut self) {
        let (full, mushroom) = (self.full_range, self.mushroom_fields);
        self.surface(
            full,
            full,
            mushroom,
            full,
            full,
            0.0,
            "minecraft:mushroom_fields",
        );
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

        self.surface(
            full,
            full,
            coast,
            e0.union(e2),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.surface(
            temperate,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.surface(
            hot,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
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

        self.surface(
            full,
            full,
            coast,
            e0.union(e2),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.surface(
            temperate,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.surface(
            hot,
            full,
            near.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
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
            "minecraft:stony_shore"
        } else {
            "minecraft:frozen_river"
        };
        let warm_shore = if weirdness.max < 0 {
            "minecraft:stony_shore"
        } else {
            "minecraft:river"
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
            "minecraft:frozen_river",
        );
        self.surface(
            unfrozen,
            full,
            near,
            e0.union(e1),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.surface(
            frozen,
            full,
            coast.union(far),
            e2.union(e5),
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.surface(
            unfrozen,
            full,
            coast.union(far),
            e2.union(e5),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.surface(
            frozen,
            full,
            coast,
            e6,
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.surface(unfrozen, full, coast, e6, weirdness, 0.0, "minecraft:river");
        let temperate = self.temperatures[1].union(self.temperatures[2]);
        let hot = self.temperatures[3].union(self.temperatures[4]);
        self.surface(
            temperate,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.surface(
            hot,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );
        self.surface(
            frozen,
            full,
            inland.union(far),
            e6,
            weirdness,
            0.0,
            "minecraft:frozen_river",
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
            full,
            Parameter::span(0.8, 1.0),
            full,
            full,
            0.0,
            "minecraft:dripstone_caves",
        );
        self.underground(
            full,
            Parameter::span(0.7, 1.0),
            full,
            full,
            full,
            0.0,
            "minecraft:lush_caves",
        );
        self.underground(
            full,
            full,
            coast.union(inland),
            e5.union(e6),
            Parameter::span(-1.1, -0.85),
            0.0,
            "minecraft:sulfur_caves",
        );
        self.bottom(
            full,
            full,
            full,
            e0.union(e1),
            full,
            0.0,
            "minecraft:deep_dark",
        );
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
pub fn overworld_parameter_list() -> ParameterList<&'static str> {
    let mut builder = Builder::new();
    builder.add_off_coast_biomes();
    builder.add_inland_biomes();
    builder.add_underground_biomes();
    ParameterList::new(builder.out)
}

/// The `minecraft:nether` preset, which is small enough to be a literal.
pub fn nether_parameter_list() -> ParameterList<&'static str> {
    let point = Parameter::point(0.0);
    let entry = |temperature: Parameter, humidity: Parameter, offset: f32, biome| {
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
            biome,
        )
    };
    ParameterList::new(vec![
        entry(point, point, 0.0, "minecraft:nether_wastes"),
        entry(
            Parameter::point(0.0),
            Parameter::point(-0.5),
            0.0,
            "minecraft:soul_sand_valley",
        ),
        entry(
            Parameter::point(0.4),
            Parameter::point(0.0),
            0.0,
            "minecraft:crimson_forest",
        ),
        entry(
            Parameter::point(0.0),
            Parameter::point(0.5),
            0.375,
            "minecraft:warped_forest",
        ),
        entry(
            Parameter::point(-0.5),
            Parameter::point(0.0),
            0.175,
            "minecraft:basalt_deltas",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::climate::TargetPoint;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn shipped_biomes() -> BTreeSet<String> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets/minecraft/worldgen/biome");
        std::fs::read_dir(dir)
            .expect("biome dir must exist")
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    return None;
                }
                Some(format!(
                    "minecraft:{}",
                    path.file_stem().unwrap().to_string_lossy()
                ))
            })
            .collect()
    }

    /// The whole point of transcribing the table: a mistyped or renamed biome
    /// is a name that no shipped biome answers to.
    #[test]
    fn every_biome_the_presets_name_is_shipped() {
        let shipped = shipped_biomes();
        for table in [overworld_parameter_list(), nether_parameter_list()] {
            for (_, biome) in table.values() {
                assert!(shipped.contains(*biome), "{biome} is not a shipped biome");
            }
        }
    }

    #[test]
    fn the_overworld_table_covers_the_overworld_biomes() {
        let table = overworld_parameter_list();
        let named: BTreeSet<&str> = table.values().iter().map(|(_, biome)| *biome).collect();
        // Every climate cell of every slice, twice for the depth ends: 22
        // off-coast, 1100 peak, 2600 high, 1432 + 1332 mid, 1032 low, 72
        // valley and the 4 underground entries.
        assert_eq!(table.len(), 7594);
        for expected in [
            "minecraft:plains",
            "minecraft:desert",
            "minecraft:jagged_peaks",
            "minecraft:mushroom_fields",
            "minecraft:deep_dark",
            "minecraft:lush_caves",
            "minecraft:sulfur_caves",
            "minecraft:dripstone_caves",
            "minecraft:pale_garden",
            "minecraft:dappled_forest",
            "minecraft:cherry_grove",
            "minecraft:mangrove_swamp",
        ] {
            assert!(
                named.contains(expected),
                "{expected} missing from the table"
            );
        }
        // Nothing from another dimension leaked in.
        for absent in [
            "minecraft:nether_wastes",
            "minecraft:the_end",
            "minecraft:basalt_deltas",
        ] {
            assert!(!named.contains(absent), "{absent} does not belong here");
        }
    }

    /// An independent check on the whole transcription: a biome carries the
    /// overworld carver set exactly when the overworld climate table can
    /// produce it. The two sets are written down in different places — one in
    /// the shipped biome JSON, one in this table — so agreeing is evidence
    /// neither is wrong.
    #[test]
    fn the_table_names_exactly_the_biomes_with_overworld_carvers() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets/minecraft/worldgen/biome");
        let mut with_overworld_carvers = BTreeSet::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let carvers: Vec<&str> = match raw.get("carvers") {
                Some(serde_json::Value::String(one)) => vec![one.as_str()],
                Some(serde_json::Value::Array(many)) => {
                    many.iter().filter_map(|v| v.as_str()).collect()
                }
                _ => Vec::new(),
            };
            if carvers.contains(&"minecraft:cave") {
                with_overworld_carvers.insert(format!(
                    "minecraft:{}",
                    path.file_stem().unwrap().to_string_lossy()
                ));
            }
        }

        let table = overworld_parameter_list();
        let named: BTreeSet<String> = table
            .values()
            .iter()
            .map(|(_, biome)| (*biome).to_owned())
            .collect();
        assert_eq!(named.len(), 56);
        assert_eq!(named, with_overworld_carvers);
    }

    #[test]
    fn the_first_entry_is_the_off_coast_one() {
        let table = overworld_parameter_list();
        assert_eq!(table.values()[0].1, "minecraft:mushroom_fields");
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
            "minecraft:mushroom_fields"
        );
        assert_eq!(
            at(-0.8, 0.0, -0.8, 0.0, 0.0, 0.0),
            "minecraft:deep_frozen_ocean"
        );
        assert_eq!(at(0.8, 0.0, -0.8, 0.0, 0.0, 0.0), "minecraft:warm_ocean");
        assert_eq!(at(-0.8, 0.0, -0.3, 0.0, 0.0, 0.0), "minecraft:frozen_ocean");
        assert_eq!(at(0.0, 0.0, 0.0, -0.9, 1.1, 0.0), "minecraft:deep_dark");
        assert_eq!(at(0.0, 0.8, 0.0, 0.0, 0.5, 0.0), "minecraft:lush_caves");
        assert_eq!(
            at(0.0, 0.0, 0.9, 0.0, 0.5, 0.0),
            "minecraft:dripstone_caves"
        );
        assert_eq!(at(0.8, 0.0, 0.5, 0.3, 0.0, -0.5), "minecraft:desert");
    }

    #[test]
    fn the_nether_table_is_the_five_reference_entries() {
        let table = nether_parameter_list();
        assert_eq!(table.len(), 5);
        let at = |temperature, humidity| {
            *table.find_value(TargetPoint::new(temperature, humidity, 0.0, 0.0, 0.0, 0.0))
        };
        assert_eq!(at(0.0, 0.0), "minecraft:nether_wastes");
        assert_eq!(at(0.0, -0.5), "minecraft:soul_sand_valley");
        assert_eq!(at(0.4, 0.0), "minecraft:crimson_forest");
        assert_eq!(at(0.0, 0.5), "minecraft:warped_forest");
        assert_eq!(at(-0.5, 0.0), "minecraft:basalt_deltas");
    }

    /// Weirdness picks between a slice's plain biome and its variant, so the
    /// same climate on either side of zero must be able to differ.
    #[test]
    fn weirdness_selects_the_variant_grid() {
        let table = overworld_parameter_list();
        let at =
            |weirdness| *table.find_value(TargetPoint::new(-0.8, -0.8, 0.2, 0.2, 0.0, weirdness));
        assert_eq!(at(-0.2), "minecraft:snowy_plains");
        assert_eq!(at(0.2), "minecraft:ice_spikes");
    }
}
