use mcrs_minecraft_core::mth::lerp;
use mcrs_minecraft_worldgen_density::proto::ProtoSpline;
use mcrs_minecraft_worldgen_density::proto::build::Df;

pub struct Coordinates {
    pub continents: Df,
    pub erosion: Df,
    pub weirdness: Df,
    pub ridges: Df,
}

fn amplified_offset(offset: f32) -> f32 {
    if offset < 0.0 { offset } else { offset * 2.0 }
}

fn amplified_factor(factor: f32) -> f32 {
    1.25 - 6.25 / (factor + 5.0)
}

fn amplified_jaggedness(jaggedness: f32) -> f32 {
    jaggedness * 2.0
}

fn amplify(spline: ProtoSpline, amplified: bool, f: fn(f32) -> f32) -> ProtoSpline {
    if amplified {
        spline.map_values(f)
    } else {
        spline
    }
}

fn peaks_and_valleys(weirdness: f32) -> f32 {
    -((weirdness.abs() - 0.6666667).abs() - 0.33333334) * 3.0
}

pub fn offset(c: &Coordinates, amplified: bool) -> ProtoSpline {
    let beach = erosion_offset(c, -0.15, 0.0, 0.0, 0.1, 0.0, -0.03, false, false);
    let low = erosion_offset(c, -0.1, 0.03, 0.1, 0.1, 0.01, -0.03, false, false);
    let mid = erosion_offset(c, -0.1, 0.03, 0.1, 0.7, 0.01, -0.03, true, true);
    let high = erosion_offset(c, -0.05, 0.03, 0.1, 1.0, 0.01, 0.01, true, true);
    let offset = ProtoSpline::points(&c.continents)
        .value(-1.1, 0.044)
        .value(-1.02, -0.2222)
        .value(-0.51, -0.2222)
        .value(-0.44, -0.12)
        .value(-0.18, -0.12)
        .spline(-0.16, &beach)
        .spline(-0.15, &beach)
        .spline(-0.1, &low)
        .spline(0.25, &mid)
        .spline(1.0, &high)
        .build();
    amplify(offset, amplified, amplified_offset)
}

pub fn factor(c: &Coordinates, amplified: bool) -> ProtoSpline {
    let inland = |base: f32, shattered: bool| {
        amplify(
            erosion_factor(c, base, shattered),
            amplified,
            amplified_factor,
        )
    };
    ProtoSpline::points(&c.continents)
        .value(-0.19, 3.95)
        .spline(-0.15, &erosion_factor(c, 6.25, true))
        .spline(-0.1, &inland(5.47, true))
        .spline(0.03, &inland(5.08, true))
        .spline(0.06, &inland(4.69, false))
        .build()
}

pub fn jaggedness(c: &Coordinates, amplified: bool) -> ProtoSpline {
    let jaggedness = ProtoSpline::points(&c.continents)
        .value(-0.11, 0.0)
        .spline(0.03, &erosion_jaggedness(c, 1.0, 0.5, 0.0, 0.0))
        .spline(0.65, &erosion_jaggedness(c, 1.0, 1.0, 1.0, 0.0))
        .build();
    amplify(jaggedness, amplified, amplified_jaggedness)
}

fn erosion_jaggedness(
    c: &Coordinates,
    peak_at_erosion_0: f32,
    peak_at_erosion_1: f32,
    high_at_erosion_0: f32,
    high_at_erosion_1: f32,
) -> ProtoSpline {
    let at_erosion_1 = ridge_jaggedness(c, peak_at_erosion_1, high_at_erosion_1);
    ProtoSpline::points(&c.erosion)
        .spline(
            -1.0,
            &ridge_jaggedness(c, peak_at_erosion_0, high_at_erosion_0),
        )
        .spline(-0.78, &at_erosion_1)
        .spline(-0.5775, &at_erosion_1)
        .value(-0.375, 0.0)
        .build()
}

fn ridge_jaggedness(c: &Coordinates, at_peak: f32, at_high: f32) -> ProtoSpline {
    let high_start = peaks_and_valleys(0.4);
    let high_end = peaks_and_valleys(0.56666666);
    ProtoSpline::points(&c.ridges)
        .value(high_start, 0.0)
        .spline(
            (high_start + high_end) / 2.0,
            &weirdness_jaggedness(c, at_high),
        )
        .spline(1.0, &weirdness_jaggedness(c, at_peak))
        .build()
}

fn weirdness_jaggedness(c: &Coordinates, factor: f32) -> ProtoSpline {
    if factor <= 0.0 {
        return ProtoSpline::Constant(0.0);
    }
    ProtoSpline::points(&c.weirdness)
        .value(-0.01, 0.63 * factor)
        .value(0.01, 0.3 * factor)
        .build()
}

fn erosion_factor(c: &Coordinates, base: f32, shattered: bool) -> ProtoSpline {
    let weirdness = |from: (f32, f32), to: (f32, f32)| {
        ProtoSpline::points(&c.weirdness)
            .value(from.0, from.1)
            .value(to.0, to.1)
            .build()
    };
    let base_spline = weirdness((-0.2, 6.3), (0.2, base));
    let erosion = ProtoSpline::points(&c.erosion)
        .spline(-0.6, &base_spline)
        .spline(-0.5, &weirdness((-0.05, 6.3), (0.05, 2.67)))
        .spline(-0.35, &base_spline)
        .spline(-0.25, &base_spline)
        .spline(-0.1, &weirdness((-0.05, 2.67), (0.05, 6.3)))
        .spline(0.03, &base_spline);
    if shattered {
        let ridges_shattered = ProtoSpline::points(&c.ridges)
            .value(-0.9, base)
            .spline(-0.69, &weirdness((0.0, base), (0.1, 0.625)))
            .build();
        erosion
            .value(0.35, base)
            .spline(0.45, &ridges_shattered)
            .spline(0.55, &ridges_shattered)
            .value(0.62, base)
            .build()
    } else {
        let extreme_hills = ProtoSpline::points(&c.ridges)
            .spline(-0.7, &base_spline)
            .value(-0.15, 1.37)
            .build();
        let peaks_only = ProtoSpline::points(&c.ridges)
            .spline(0.45, &base_spline)
            .value(0.7, 1.56)
            .build();
        erosion
            .spline(0.05, &peaks_only)
            .spline(0.4, &peaks_only)
            .spline(0.45, &extreme_hills)
            .spline(0.55, &extreme_hills)
            .value(0.58, base)
            .build()
    }
}

const RIDGE_OFFSET: f32 = 1.17;
const RIDGE_AMPLITUDE: f32 = 0.46082947;
const ALLOW_RIVERS_BELOW: f32 = -0.7;

fn mountain_continentalness(ridge: f32, modulation: f32) -> f32 {
    let ridge_slope = 1.0 - (1.0 - modulation) * 0.5;
    let ridge_intersect = 0.5 * (1.0 - modulation);
    let continentalness = (ridge + RIDGE_OFFSET) * RIDGE_AMPLITUDE * ridge_slope - ridge_intersect;
    let floor = if ridge < ALLOW_RIVERS_BELOW {
        -0.2222
    } else {
        0.0
    };
    continentalness.max(floor)
}

fn mountain_ridge_zero_point(modulation: f32) -> f32 {
    let ridge_slope = 1.0 - (1.0 - modulation) * 0.5;
    let ridge_intersect = 0.5 * (1.0 - modulation);
    ridge_intersect / (RIDGE_AMPLITUDE * ridge_slope) - RIDGE_OFFSET
}

fn mountain_ridge(c: &Coordinates, modulation: f32, saddle: bool) -> ProtoSpline {
    let at = |ridge: f32| mountain_continentalness(ridge, modulation);
    let (min, max) = (at(-1.0), at(1.0));
    let zero_point = mountain_ridge_zero_point(modulation);
    let ridge = ProtoSpline::points(&c.ridges);
    if -0.65 < zero_point && zero_point < 1.0 {
        let before_river = at(-0.75);
        let at_zero_point = at(zero_point);
        let max_derivative = (max - at_zero_point) / (1.0 - zero_point);
        ridge
            .sloped(-1.0, min, (before_river - min) / (-0.75 - -1.0))
            .value(-0.75, before_river)
            .value(-0.65, at(-0.65))
            .value(zero_point - 0.01, at_zero_point)
            .sloped(zero_point, at_zero_point, max_derivative)
            .sloped(1.0, max, max_derivative)
            .build()
    } else {
        let derivative = (max - min) / (1.0 - -1.0);
        let ridge = if saddle {
            ridge
                .value(-1.0, min.max(0.2))
                .sloped(0.0, lerp(0.5, min, max), derivative)
        } else {
            ridge.sloped(-1.0, min, derivative)
        };
        ridge.sloped(1.0, max, derivative).build()
    }
}

#[allow(clippy::too_many_arguments)]
fn erosion_offset(
    c: &Coordinates,
    low_valley: f32,
    hill: f32,
    tall_hill: f32,
    mountain_factor: f32,
    plain: f32,
    swamp: f32,
    extreme_hills: bool,
    saddle: bool,
) -> ProtoSpline {
    let very_low_erosion_mountains = mountain_ridge(c, lerp(mountain_factor, 0.6, 1.5), saddle);
    let low_erosion_mountains = mountain_ridge(c, lerp(mountain_factor, 0.6, 1.0), saddle);
    let mountains = mountain_ridge(c, mountain_factor, saddle);
    let (half, most) = (0.5 * mountain_factor, 0.6 * mountain_factor);
    let wide_plateau = ridge_spline(c, low_valley - 0.15, half, half, half, most, 0.5);
    let narrow_plateau = ridge_spline(
        c,
        low_valley,
        plain * mountain_factor,
        hill * mountain_factor,
        half,
        most,
        0.5,
    );
    let plains = ridge_spline(c, low_valley, plain, plain, hill, tall_hill, 0.5);
    let swamps = ridge_spline(c, -0.02, swamp, swamp, hill, tall_hill, 0.0);
    let erosion = ProtoSpline::points(&c.erosion)
        .spline(-0.85, &very_low_erosion_mountains)
        .spline(-0.7, &low_erosion_mountains)
        .spline(-0.4, &mountains)
        .spline(-0.35, &wide_plateau)
        .spline(-0.1, &narrow_plateau)
        .spline(0.2, &plains);
    let erosion = if extreme_hills {
        let hills = ProtoSpline::points(&c.ridges)
            .value(-1.0, low_valley)
            .spline(-0.4, &plains)
            .value(0.0, tall_hill + 0.07)
            .build();
        erosion
            .spline(0.4, &plains)
            .spline(0.45, &hills)
            .spline(0.55, &hills)
            .spline(0.58, &plains)
    } else {
        erosion
    };
    erosion.spline(0.7, &swamps).build()
}

fn ridge_spline(
    c: &Coordinates,
    valley: f32,
    low: f32,
    mid: f32,
    high: f32,
    peaks: f32,
    min_valley_steepness: f32,
) -> ProtoSpline {
    let d1 = (0.5 * (low - valley)).max(min_valley_steepness);
    let d2 = 5.0 * (mid - low);
    ProtoSpline::points(&c.ridges)
        .sloped(-1.0, valley, d1)
        .sloped(-0.4, low, d1.min(d2))
        .sloped(0.0, mid, d2)
        .sloped(0.4, high, 2.0 * (high - mid))
        .sloped(1.0, peaks, 0.7 * (peaks - high))
        .build()
}
