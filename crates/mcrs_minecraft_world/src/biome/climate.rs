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

/// Climate coordinates are compared as fixed point, not floats: the reference
/// multiplies by ten thousand and truncates, and the search is exact integer
/// arithmetic from there on. The multiply happens in f32 because that is the
/// width the codec reads, and rounding it in f64 would land on a different
/// integer for some values.
pub fn quantize_coord(coord: f32) -> i64 {
    (coord * 10000.0) as i64
}

pub fn unquantize_coord(coord: i64) -> f32 {
    coord as f32 / 10000.0
}

/// One climate coordinate's accepted span, quantized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    pub fn point(value: f32) -> Self {
        Self::span(value, value)
    }

    pub fn span(min: f32, max: f32) -> Self {
        assert!(min <= max, "climate span min > max: {min} > {max}");
        Parameter {
            min: quantize_coord(min),
            max: quantize_coord(max),
        }
    }

    /// The union of two spans, which is how the reference widens a slice to
    /// cover several of its neighbours.
    pub fn union(self, other: Parameter) -> Self {
        Parameter {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// How far `target` lies outside the span, zero when inside it.
    pub fn distance(self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 { above } else { below.max(0) }
    }
}

impl From<&ParameterRange> for Parameter {
    fn from(range: &ParameterRange) -> Self {
        match *range {
            ParameterRange::Point(value) => Parameter::point(value as f32),
            ParameterRange::Range([min, max]) => Parameter::span(min as f32, max as f32),
        }
    }
}

/// The climate a biome accepts. `offset` is a flat penalty that lets one entry
/// lose a tie to another whose spans fit equally well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterPoint {
    pub temperature: Parameter,
    pub humidity: Parameter,
    pub continentalness: Parameter,
    pub erosion: Parameter,
    pub depth: Parameter,
    pub weirdness: Parameter,
    pub offset: i64,
}

impl ParameterPoint {
    pub fn fitness(&self, target: TargetPoint) -> i64 {
        let terms = [
            self.temperature.distance(target.temperature),
            self.humidity.distance(target.humidity),
            self.continentalness.distance(target.continentalness),
            self.erosion.distance(target.erosion),
            self.depth.distance(target.depth),
            self.weirdness.distance(target.weirdness),
            self.offset,
        ];
        terms.iter().map(|term| term * term).sum()
    }
}

impl From<&ClimateParameters> for ParameterPoint {
    fn from(parameters: &ClimateParameters) -> Self {
        ParameterPoint {
            temperature: (&parameters.temperature).into(),
            humidity: (&parameters.humidity).into(),
            continentalness: (&parameters.continentalness).into(),
            erosion: (&parameters.erosion).into(),
            depth: (&parameters.depth).into(),
            weirdness: (&parameters.weirdness).into(),
            offset: quantize_coord(parameters.offset as f32),
        }
    }
}

/// The climate actually sampled at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetPoint {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}

impl TargetPoint {
    pub fn new(
        temperature: f32,
        humidity: f32,
        continentalness: f32,
        erosion: f32,
        depth: f32,
        weirdness: f32,
    ) -> Self {
        TargetPoint {
            temperature: quantize_coord(temperature),
            humidity: quantize_coord(humidity),
            continentalness: quantize_coord(continentalness),
            erosion: quantize_coord(erosion),
            depth: quantize_coord(depth),
            weirdness: quantize_coord(weirdness),
        }
    }
}

/// A biome table, searched by nearest climate.
///
/// The reference indexes this with an R-tree over the seven-dimensional
/// parameter space. A linear scan returns the same entry — the tree only
/// prunes — and callers reach this once per chunk, memoized, rather than once
/// per block, so the tree's build cost buys nothing yet. It becomes worth
/// having the moment something needs a per-block biome.
#[derive(Debug, Clone)]
pub struct ParameterList<T> {
    values: Vec<(ParameterPoint, T)>,
}

impl<T> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> Self {
        assert!(
            !values.is_empty(),
            "a climate table needs at least one entry"
        );
        ParameterList { values }
    }

    pub fn values(&self) -> &[(ParameterPoint, T)] {
        &self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The entry whose climate fits `target` best. Ties go to the earlier
    /// entry, which is the order the table was built in.
    pub fn find_value(&self, target: TargetPoint) -> &T {
        let mut best = &self.values[0];
        let mut best_fitness = best.0.fitness(target);
        for candidate in &self.values[1..] {
            let fitness = candidate.0.fitness(target);
            if fitness < best_fitness {
                best_fitness = fitness;
                best = candidate;
            }
        }
        &best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(temperature: f32, humidity: f32) -> ParameterPoint {
        ParameterPoint {
            temperature: Parameter::point(temperature),
            humidity: Parameter::point(humidity),
            continentalness: Parameter::point(0.0),
            erosion: Parameter::point(0.0),
            depth: Parameter::point(0.0),
            weirdness: Parameter::point(0.0),
            offset: 0,
        }
    }

    #[test]
    fn quantization_truncates_toward_zero_at_f32_width() {
        assert_eq!(quantize_coord(0.0), 0);
        assert_eq!(quantize_coord(1.0), 10000);
        assert_eq!(quantize_coord(-1.0), -10000);
        assert_eq!(quantize_coord(0.05), 500);
        // 0.26666668 * 10000 lands just under 2666.667, and truncation is what
        // the reference does with it.
        assert_eq!(quantize_coord(0.266_666_68), 2666);
        assert_eq!(unquantize_coord(quantize_coord(0.5)), 0.5);
    }

    #[test]
    fn a_span_measures_only_the_distance_outside_itself() {
        let span = Parameter::span(-0.2, 0.2);
        assert_eq!(span.distance(quantize_coord(0.0)), 0);
        assert_eq!(span.distance(quantize_coord(0.2)), 0);
        assert_eq!(span.distance(quantize_coord(0.3)), 1000);
        assert_eq!(span.distance(quantize_coord(-0.3)), 1000);
    }

    #[test]
    fn a_union_covers_both_spans() {
        let joined = Parameter::span(-0.5, -0.2).union(Parameter::span(0.1, 0.4));
        assert_eq!(joined, Parameter::span(-0.5, 0.4));
    }

    #[test]
    fn the_offset_is_a_flat_penalty_that_breaks_a_tie() {
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let plain = point(0.0, 0.0);
        let penalized = ParameterPoint {
            offset: quantize_coord(0.1),
            ..plain
        };
        assert_eq!(plain.fitness(target), 0);
        assert_eq!(penalized.fitness(target), 1000 * 1000);
        let table = ParameterList::new(vec![(penalized, "penalized"), (plain, "plain")]);
        assert_eq!(*table.find_value(target), "plain");
    }

    #[test]
    fn the_nearest_entry_wins_and_ties_go_to_the_earlier_one() {
        let table = ParameterList::new(vec![
            (point(-0.5, 0.0), "cold"),
            (point(0.5, 0.0), "warm"),
            (point(0.5, 0.0), "warm duplicate"),
        ]);
        assert_eq!(
            *table.find_value(TargetPoint::new(-0.4, 0.0, 0.0, 0.0, 0.0, 0.0)),
            "cold"
        );
        assert_eq!(
            *table.find_value(TargetPoint::new(0.6, 0.0, 0.0, 0.0, 0.0, 0.0)),
            "warm"
        );
    }

    #[test]
    fn every_coordinate_counts_toward_the_fit() {
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut only_weirdness = point(0.0, 0.0);
        only_weirdness.weirdness = Parameter::point(1.0);
        assert_eq!(only_weirdness.fitness(target), 10000 * 10000);
        let mut only_depth = point(0.0, 0.0);
        only_depth.depth = Parameter::point(1.0);
        assert_eq!(only_depth.fitness(target), 10000 * 10000);
    }

    /// The explicit `biomes` form of a multi-noise source carries the same
    /// spans through the JSON codec, so a parsed entry and a built one must
    /// quantize identically.
    #[test]
    fn a_parsed_entry_matches_a_built_one() {
        let parsed: ClimateParameters = serde_json::from_str(
            r#"{"temperature":[-0.45,-0.15],"humidity":0.0,"continentalness":[-0.11,0.55],
                "erosion":[-0.375,0.05],"depth":0.0,"weirdness":[-1.0,-0.78],"offset":0.0}"#,
        )
        .unwrap();
        let built = ParameterPoint {
            temperature: Parameter::span(-0.45, -0.15),
            humidity: Parameter::point(0.0),
            continentalness: Parameter::span(-0.11, 0.55),
            erosion: Parameter::span(-0.375, 0.05),
            depth: Parameter::point(0.0),
            weirdness: Parameter::span(-1.0, -0.78),
            offset: 0,
        };
        assert_eq!(ParameterPoint::from(&parsed), built);
    }
}
