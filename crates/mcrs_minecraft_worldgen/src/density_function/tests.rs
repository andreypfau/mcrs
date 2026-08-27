use super::{BlendedNoise, EndIslands, FillScratch, RangeFunction};
use crate::density_function::DensityFunction;
use crate::density_function::beta_seed::seed_beta_terrain;
use crate::proto::NoiseGeneratorSettings;
use bevy_math::IVec3;
use mcrs_minecraft_random::RandomSource;

#[test]
fn end_outer_islands_matches_the_vanilla_oracle() {
    const EXPECTED: &[(u64, i32, i32, u32)] = &[
        (0, 0, 0, 0xbf58_0000),
        (0, 16, 16, 0xbf58_0000),
        (0, -16, -16, 0xbf58_0000),
        (0, 1000, 1000, 0x3e24_e8e8),
        (0, -1234, 5678, 0x3f10_0000),
        (0, 2000, -40, 0xbe9e_1e2c),
        (0, 123, -457, 0xbf58_0000),
        (0, 50000, 50000, 0xbd8f_4978),
        (42, 0, 0, 0xbf58_0000),
        (42, 16, 16, 0xbf58_0000),
        (42, -16, -16, 0xbf58_0000),
        (42, 1000, 1000, 0xbf11_75ce),
        (42, -1234, 5678, 0xbe3c_9478),
        (42, 2000, -40, 0xbf40_9450),
        (42, 123, -457, 0xbf58_0000),
        (42, 50000, 50000, 0xbdf7_af08),
        (845, 0, 0, 0xbf58_0000),
        (845, 16, 16, 0xbf58_0000),
        (845, -16, -16, 0xbf58_0000),
        (845, 1000, 1000, 0x3b9c_5800),
        (845, -1234, 5678, 0xbd4f_a270),
        (845, 2000, -40, 0xbdbc_6170),
        (845, 123, -457, 0xbf58_0000),
        (845, 50000, 50000, 0x3eab_39dc),
    ];

    let mut islands: Option<(u64, EndIslands)> = None;
    for &(seed, x, z, expected) in EXPECTED {
        let function = match &islands {
            Some((s, f)) if *s == seed => f,
            _ => {
                islands = Some((seed, EndIslands::new(seed)));
                &islands.as_ref().unwrap().1
            }
        };
        for y in [-64, 0, 200] {
            let actual = function.sample(IVec3::new(x, y, z));
            assert_eq!(
                actual.to_bits(),
                expected,
                "seed {seed} at ({x}, {y}, {z}): got {actual}"
            );
        }
        let value = f32::from_bits(expected);
        assert!((function.min_value()..=function.max_value()).contains(&value));
    }
}

#[test]
fn modern_blended_noise_unchanged() {
    let mut random = RandomSource::new(0, true);
    let noise = BlendedNoise::new(&mut random, 1.0, 1.0, 80.0, 160.0, 8.0, 128.0);
    for (pos, expected) in [
        ((0, 0, 0), 1050715813u32),
        ((4, 8, 4), 1044906416),
        ((8, 16, 8), 1054301785),
    ] {
        let sample = noise.sample(bevy_math::IVec3::new(pos.0, pos.1, pos.2));
        assert_eq!(sample.to_bits(), expected, "blended noise moved at {pos:?}");
    }
}

#[test]
fn blended_noise_never_leaves_its_declared_range() {
    let mut rng = 0x9e3779b97f4a7c15u64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for (xz_scale, y_scale, xz_factor, y_factor, smear, divisor) in [
        (1.0, 1.0, 80.0, 160.0, 8.0, 128.0),
        (1.0, 1.0, 80.0, 160.0, 8.0, 1.0),
        (0.25, 0.125, 80.0, 160.0, 8.0, 128.0),
        (0.25, 0.125, 80.0, 160.0, 8.0, 1.0),
        (0.25, 0.125, 20.0, 40.0, 1.0, 128.0),
        (1000.0, 0.001, 0.5, 1000.0, 8.0, 128.0),
    ] {
        let mut random = RandomSource::new(next() as i64 as u64, true);
        let noise = BlendedNoise::new(
            &mut random,
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear,
            divisor,
        );
        let bound = noise.max_value();
        let mut peak = 0.0f32;
        assert_eq!(bound, -noise.min_value());
        for _ in 0..200_000 {
            let pos = bevy_math::IVec3::new(
                (next() % 4_000_001) as i32 - 2_000_000,
                (next() % 2049) as i32 - 1024,
                (next() % 4_000_001) as i32 - 2_000_000,
            );
            let value = noise.sample(pos);
            peak = peak.max(value.abs());
            assert!(
                value.abs() <= bound,
                "{pos:?} sampled {value} outside +/-{bound} (divisor {divisor})"
            );
        }
        assert!(
            peak > bound * 0.25,
            "peak {peak} is so far under {bound} that the bound proves nothing"
        );
    }
}

/// Verify that disabling the /128 divisor yields exactly 128x the enabled-divisor output.
#[test]
fn blended_noise_no_128_divisor() {
    let mut r1 = RandomSource::new(12345, true);
    let noise_with_div = BlendedNoise::new(&mut r1, 1.0, 1.0, 80.0, 160.0, 8.0, 128.0);
    let mut r2 = RandomSource::new(12345, true);
    let noise_no_div = BlendedNoise::new(&mut r2, 1.0, 1.0, 80.0, 160.0, 8.0, 1.0);

    let pos = bevy_math::IVec3::new(4, 8, 4);
    let v_div = noise_with_div.sample(pos);
    let v_nodiv = noise_no_div.sample(pos);
    let ratio = v_nodiv / v_div;
    assert!(
        (ratio - 128.0).abs() < 1e-3,
        "disabling divisor should yield 128x output, got ratio {}",
        ratio
    );
}

#[test]
fn beta_scale_depth_2d_finite() {
    use crate::noise::normal_noise::NoiseSampler;
    let (_, _, _, _, _, scale_noise, depth_noise) = seed_beta_terrain(12345);
    let scale_node = NoiseSampler::beta_octave_2d(scale_noise.clone(), 1.121, 2048.0);
    let depth_node = NoiseSampler::beta_octave_2d(depth_noise.clone(), 200.0, 131072.0);

    let sv_a = scale_node.get(0.0, 0.0, 0.0);
    let sv_y = scale_node.get(0.0, 100.0, 0.0);
    let dv_a = depth_node.get(0.0, 0.0, 0.0);
    let dv_y = depth_node.get(0.0, 100.0, 0.0);

    assert!(sv_a.is_finite(), "scale at origin must be finite");
    assert!(dv_a.is_finite(), "depth at origin must be finite");
    assert!(
        scale_node.get(64.0, 0.0, 64.0).is_finite(),
        "scale at (64,0,64) must be finite"
    );
    assert!(
        depth_node.get(64.0, 0.0, 64.0).is_finite(),
        "depth at (64,0,64) must be finite"
    );
    assert_eq!(sv_a, sv_y, "beta scale sampler must ignore y");
    assert_eq!(dv_a, dv_y, "beta depth sampler must ignore y");

    // Noise-cell semantics: blocks within the same 4-block cell sample identically,
    // matching the deleted BetaScale2d/BetaDepth2d (pos.x >> 2) convention.
    assert_eq!(
        scale_node.get(5.0, 0.0, 7.0),
        scale_node.get(4.0, 0.0, 4.0),
        "scale sampler must quantize to noise cells (block >> 2)"
    );
    assert_eq!(
        depth_node.get(-1.0, 0.0, -4.0),
        depth_node.get(-4.0, 0.0, -1.0),
        "depth sampler must floor-quantize negative coords to noise cells"
    );
    // (pos.x >> 2) sampled directly through sample_xz must agree with the sampler.
    assert_eq!(
        scale_node.get(13.0, 0.0, -9.0),
        scale_noise.sample_xz((13 >> 2) as f32, (-9 >> 2) as f32, 1.121, 1.121),
        "sampler must match raw sample_xz at (block >> 2) coords"
    );
    assert_eq!(
        depth_node.get(13.0, 0.0, -9.0),
        depth_noise.sample_xz((13 >> 2) as f32, (-9 >> 2) as f32, 200.0, 200.0),
        "sampler must match raw sample_xz at (block >> 2) coords"
    );
}

#[test]
fn beta_blended_noise_samples_finite() {
    use std::collections::BTreeMap;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json should exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json should deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    let router = super::build_functions(
        &functions,
        &noises,
        &settings,
        12345,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );

    // Sample a column at multiple Y values to find a sign flip
    let mut all_densities = vec![];
    let mut scratch = FillScratch::new();
    for y in (0..128).step_by(8) {
        let v = router.sample_value(
            router.final_density_index,
            bevy_math::IVec3::new(0, y, 0),
            &mut scratch,
        );
        assert!(v.is_finite(), "density at y={y} must be finite");
        all_densities.push(v);
    }

    // Must have a sign flip somewhere in 0..128 (real terrain surface)
    let has_positive = all_densities.iter().any(|&v| v > 0.0);
    let has_negative = all_densities.iter().any(|&v| v < 0.0);
    assert!(
        has_positive && has_negative,
        "beta terrain must have both positive and negative densities across 0..128 (surface exists), got: {:?}",
        all_densities
    );
}

#[test]
fn beta_build_functions_wires_final_density() {
    use std::collections::BTreeMap;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json should exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json should deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    let router = super::build_functions(
        &functions,
        &noises,
        &settings,
        12345,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );
    // Zone A must contain the two cached 2D nodes (scale/depth).
    assert!(
        router.column_boundary > 0,
        "Zone A must be non-empty (cached 2D scale/depth nodes)"
    );
    // final_density must be wired into Zone B.
    assert!(
        router.final_density_index() >= router.column_boundary,
        "final_density must be in Zone B"
    );
}

/// Java ground truth: full 5x17x5 noise field q for chunk (0,0), seed 845,
/// computed by replicating ChunkProviderGenerate.a / NoiseGeneratorOctaves /
/// NoiseGeneratorPerlin / WorldChunkManager from Beta 1.7.3 (f64) byte-exactly.
/// Order matches Java iteration: x outer, z mid, y inner (17 rows per column).
#[test]
fn beta_density_matches_java_ground_truth_seed845() {
    use std::collections::BTreeMap;
    const JAVA_Q: [f32; 425] = [
        666.401823,
        574.970280,
        478.681110,
        380.819986,
        284.135296,
        185.571644,
        89.311232,
        -6.778209,
        -55.011039,
        -76.112579,
        -99.501081,
        -121.413514,
        -146.573790,
        -171.385002,
        -132.712962,
        -77.591012,
        -10.000000,
        669.342833,
        574.713830,
        478.024659,
        383.470108,
        290.005486,
        188.965738,
        90.407789,
        -5.468320,
        -52.866059,
        -47.876501,
        -92.087417,
        -120.040676,
        -144.508432,
        -166.961468,
        -132.321389,
        -77.502219,
        -10.000000,
        672.734144,
        575.044405,
        480.980701,
        386.531316,
        289.862162,
        191.067845,
        92.867618,
        -6.419450,
        -51.188966,
        -76.268906,
        -102.445166,
        -118.400780,
        -140.788081,
        -161.827064,
        -131.322503,
        -77.179950,
        -10.000000,
        674.904594,
        576.065926,
        483.954049,
        392.399859,
        297.335288,
        196.118769,
        97.615984,
        0.451100,
        -49.337367,
        -75.968985,
        -102.672964,
        -115.084212,
        -137.307103,
        -158.665919,
        -127.474046,
        -75.037052,
        -10.000000,
        674.059567,
        576.376996,
        482.646532,
        390.885434,
        296.215616,
        198.251638,
        101.281242,
        6.233780,
        -47.346325,
        -76.445555,
        -103.679837,
        -113.507576,
        -134.853416,
        -157.808348,
        -124.986103,
        -73.264414,
        -10.000000,
        666.321338,
        573.754282,
        475.123270,
        377.912645,
        282.822846,
        184.034556,
        88.036727,
        -7.113503,
        -56.272692,
        -74.245514,
        -99.871382,
        -119.567582,
        -147.106212,
        -174.233256,
        -135.453875,
        -77.604082,
        -10.000000,
        667.064067,
        571.139682,
        476.550205,
        381.060976,
        286.908059,
        186.484358,
        89.676142,
        -4.531380,
        -54.241825,
        -74.624135,
        -100.887252,
        -118.632211,
        -146.377092,
        -169.212050,
        -134.893946,
        -77.778141,
        -10.000000,
        668.267770,
        571.907047,
        476.760097,
        382.324817,
        287.710318,
        187.011360,
        89.151144,
        -10.808942,
        -53.341635,
        -75.849727,
        -103.924576,
        -116.553822,
        -141.364431,
        -164.141531,
        -132.996807,
        -76.958444,
        -10.000000,
        669.074471,
        572.633310,
        478.862617,
        387.063419,
        292.075235,
        189.687961,
        92.529843,
        -8.852508,
        -53.687553,
        -74.794100,
        -101.081951,
        -114.399523,
        -137.428277,
        -159.724123,
        -128.698493,
        -75.314063,
        -10.000000,
        668.468517,
        570.640666,
        477.322101,
        385.537356,
        290.977160,
        190.682416,
        94.084826,
        -4.863159,
        -52.207734,
        -75.857369,
        -100.683690,
        -113.618091,
        -135.918690,
        -158.129795,
        -126.738469,
        -74.330833,
        -10.000000,
        660.251630,
        568.153617,
        470.324742,
        371.830594,
        277.114690,
        181.309854,
        84.001524,
        -13.314334,
        -59.489066,
        -73.863309,
        -99.237947,
        -118.479167,
        -148.881309,
        -175.314348,
        -137.261292,
        -78.901549,
        -10.000000,
        663.346902,
        565.708005,
        471.639085,
        374.919340,
        280.333148,
        183.333784,
        84.682601,
        -10.590852,
        -57.967263,
        -74.414654,
        -101.757907,
        -117.033592,
        -145.698244,
        -170.846144,
        -135.201435,
        -78.212604,
        -10.000000,
        664.982287,
        567.420291,
        473.498656,
        379.476121,
        285.950015,
        185.920028,
        87.153552,
        -10.297192,
        -56.471469,
        -75.109933,
        -102.838374,
        -115.232962,
        -141.504711,
        -165.216749,
        -132.713858,
        -77.356849,
        -10.000000,
        665.661169,
        568.743053,
        473.145448,
        382.455458,
        287.768767,
        186.227355,
        87.063511,
        -12.634489,
        -55.150207,
        -74.916734,
        -100.325923,
        -112.962969,
        -138.951443,
        -162.951349,
        -130.388150,
        -75.838621,
        -10.000000,
        663.752590,
        566.332611,
        470.935959,
        381.429498,
        286.673279,
        186.621242,
        89.461488,
        -11.957783,
        -55.266499,
        -75.807301,
        -102.301229,
        -113.103323,
        -138.112630,
        -162.233767,
        -128.716157,
        -74.759912,
        -10.000000,
        660.902565,
        569.469006,
        471.478709,
        371.553758,
        274.323295,
        181.006891,
        81.752000,
        -17.011096,
        -58.514686,
        -73.142533,
        -97.777174,
        -118.769499,
        -146.873997,
        -171.634882,
        -135.824081,
        -78.765859,
        -10.000000,
        662.076495,
        566.027445,
        471.393663,
        374.700455,
        278.211565,
        183.551246,
        83.185912,
        -14.907925,
        -58.845890,
        -73.436010,
        -98.670127,
        -115.418750,
        -143.681340,
        -168.945794,
        -134.432819,
        -78.418499,
        -10.000000,
        664.217491,
        567.223105,
        473.289568,
        379.379958,
        284.119886,
        185.970268,
        87.016266,
        -12.227263,
        -57.498530,
        -75.121935,
        -102.664991,
        -113.955838,
        -139.988759,
        -166.138875,
        -132.869586,
        -77.181576,
        -10.000000,
        664.704731,
        568.040216,
        473.040241,
        380.722090,
        287.176577,
        184.422623,
        87.538162,
        -9.589195,
        -54.660401,
        -74.029853,
        -100.387492,
        -113.290830,
        -139.539897,
        -164.076101,
        -130.401158,
        -76.018545,
        -10.000000,
        662.375413,
        565.716182,
        470.751240,
        380.620118,
        284.853525,
        185.194922,
        88.335615,
        -8.028071,
        -54.532626,
        -76.207332,
        -101.731390,
        -114.635772,
        -139.706055,
        -163.914887,
        -129.332874,
        -75.098150,
        -10.000000,
        664.370917,
        571.604140,
        472.797294,
        373.908726,
        276.933247,
        182.794375,
        84.345789,
        -14.746582,
        -58.100517,
        -73.880614,
        -95.543194,
        -117.777348,
        -143.937817,
        -166.800452,
        -131.611208,
        -77.183241,
        -10.000000,
        663.679301,
        570.768397,
        472.329104,
        376.523007,
        282.759980,
        185.809434,
        86.750410,
        -13.329684,
        -57.060859,
        -74.584002,
        -96.202405,
        -114.786064,
        -141.694237,
        -167.277362,
        -131.808050,
        -77.443974,
        -10.000000,
        664.869151,
        570.160451,
        475.823317,
        381.108105,
        288.270719,
        187.560799,
        89.310895,
        -10.293879,
        -55.514385,
        -74.190969,
        -101.284476,
        -115.137061,
        -140.664055,
        -166.131633,
        -132.072757,
        -76.846243,
        -10.000000,
        665.854372,
        569.408691,
        477.073104,
        384.448890,
        291.124860,
        188.113546,
        90.342023,
        -4.526532,
        -52.118861,
        -74.466064,
        -103.903487,
        -114.401566,
        -137.642271,
        -162.648556,
        -129.420618,
        -76.169560,
        -10.000000,
        663.084255,
        564.940685,
        474.790421,
        381.058727,
        286.722097,
        187.061383,
        89.863657,
        7.550697,
        -23.185904,
        -60.880314,
        -86.694535,
        -113.897573,
        -139.597863,
        -163.739809,
        -129.224495,
        -76.082980,
        -10.000000,
    ];
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json should exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json should deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    let router = super::build_functions(
        &functions,
        &noises,
        &settings,
        845,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );
    let mut i = 0;
    let mut max_diff = 0.0_f32;
    let mut scratch = FillScratch::new();
    for cx in 0..5i32 {
        for cz in 0..5i32 {
            for cy in 0..17i32 {
                let pos = bevy_math::IVec3::new(cx * 4, cy * 8, cz * 4);
                let rust_v = router.sample_value(router.final_density_index, pos, &mut scratch);
                let java_v = JAVA_Q[i];
                let diff = (rust_v - java_v).abs();
                max_diff = max_diff.max(diff);
                // Residual tolerance covers f32 noise accumulation and the
                // climate sample-point offset (quart origins vs Java's
                // per-chunk cell*3+1 stride, which is seam-inconsistent in
                // Java itself). All structural divergence is far above this.
                assert!(
                    diff < 8.0,
                    "q({},{},{}): java={} rust={} diff={}",
                    cx,
                    cz,
                    cy,
                    java_v,
                    rust_v,
                    diff
                );
                if java_v.abs() > 20.0 {
                    assert_eq!(
                        java_v > 0.0,
                        rust_v > 0.0,
                        "sign mismatch at q({},{},{}): java={} rust={}",
                        cx,
                        cz,
                        cy,
                        java_v,
                        rust_v
                    );
                }
                i += 1;
            }
        }
    }
    assert!(max_diff < 8.0, "max diff {}", max_diff);
}
#[test]
#[ignore]
fn beta_dump_density_chunk00_seed845() {
    use std::collections::BTreeMap;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json should exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json should deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    let router = super::build_functions(
        &functions,
        &noises,
        &settings,
        845,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );
    let mut scratch = FillScratch::new();
    for cx in 0..5i32 {
        for cz in 0..5i32 {
            for cy in 0..17i32 {
                let pos = bevy_math::IVec3::new(cx * 4, cy * 8, cz * 4);
                let d = router.sample_value(router.final_density_index, pos, &mut scratch);
                println!("q {} {} {} {:.6}", cx, cz, cy, d);
            }
        }
    }
}

#[test]
fn beta_json_deserializes() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json should exist at assets path");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json should deserialize without error");
    assert_eq!(settings.sea_level, 64);
    assert_eq!(settings.noise.height, 128);
    assert!(settings.legacy_random_source);
}

fn recurse_density_functions(
    dir: &std::path::Path,
    prefix: &str,
    map: &mut std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::ProtoDensityFunction,
    >,
) {
    use crate::density_function::proto::DensityFunctionHolder;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let subdir = entry.file_name().to_string_lossy().to_string();
            let new_prefix = if prefix.is_empty() {
                subdir
            } else {
                format!("{}/{}", prefix, subdir)
            };
            recurse_density_functions(&path, &new_prefix, map);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let json = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let holder = serde_json::from_str::<DensityFunctionHolder>(&json)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let function = match holder {
                DensityFunctionHolder::Owned(pdf) => *pdf,
                DensityFunctionHolder::Value(value) => {
                    crate::density_function::ProtoDensityFunction::Constant(value)
                }
                DensityFunctionHolder::Reference(target) => {
                    panic!(
                        "{}: a density function file must not be a bare reference to {target}",
                        path.display()
                    )
                }
            };
            let stem = path.file_stem().unwrap().to_string_lossy();
            let key = if prefix.is_empty() {
                format!("minecraft:{}", stem)
            } else {
                format!("minecraft:{}/{}", prefix, stem)
            };
            let ident = key
                .parse::<mcrs_minecraft_core::ResourceLocation>()
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            map.insert(ident, function);
        }
    }
}

/// Load all density_function JSON assets recursively into a `ProtoDensityFunction` map.
fn load_density_functions_from_disk() -> std::collections::BTreeMap<
    mcrs_minecraft_core::ResourceLocation,
    crate::density_function::ProtoDensityFunction,
> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/density_function");
    let mut map = std::collections::BTreeMap::new();
    recurse_density_functions(&base, "", &mut map);
    map
}

/// Load all noise JSON assets into a `NoiseParam` map.
fn load_noises_from_disk() -> std::collections::BTreeMap<
    mcrs_minecraft_core::ResourceLocation,
    crate::density_function::proto::NoiseParam,
> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/noise");
    let mut map = std::collections::BTreeMap::new();
    recurse_noises(&base, "", &mut map);
    map
}

fn recurse_noises(
    dir: &std::path::Path,
    prefix: &str,
    map: &mut std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::proto::NoiseParam,
    >,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let subdir = entry.file_name().to_string_lossy().to_string();
            let new_prefix = if prefix.is_empty() {
                subdir
            } else {
                format!("{}/{}", prefix, subdir)
            };
            recurse_noises(&path, &new_prefix, map);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let json =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let noise = serde_json::from_str::<crate::density_function::proto::NoiseParam>(&json)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let stem = path.file_stem().unwrap().to_string_lossy();
        let key = if prefix.is_empty() {
            format!("minecraft:{}", stem)
        } else {
            format!("minecraft:{}/{}", prefix, stem)
        };
        let ident = key
            .parse::<mcrs_minecraft_core::ResourceLocation>()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        map.insert(ident, noise);
    }
}

fn count_json_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} must exist", dir.display());
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                count_json_files(&path)
            } else {
                usize::from(path.extension().and_then(|s| s.to_str()) == Some("json"))
            }
        })
        .sum()
}

fn settings_for(settings_file: &str) -> NoiseGeneratorSettings {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/noise_settings")
        .join(settings_file);
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn router_for(settings_file: &str) -> super::NoiseRouter {
    let settings = settings_for(settings_file);
    super::build_functions(
        &load_density_functions_from_disk(),
        &load_noises_from_disk(),
        &settings,
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    )
}

/// A `FillScratch` is not one root's private buffer: every root sampled
/// through a shared one, in any order, must answer exactly as a scratch
/// built for it alone does.
#[test]
fn a_shared_scratch_answers_every_root() {
    let router = router_for("overworld.json");
    let mut roots = router.roots();
    roots.sort_by_key(|&(_, index)| index);

    for (x, y, z) in [(37, 55, -19), (37, 71, -19), (38, 55, -19)] {
        let pos = bevy_math::IVec3::new(x, y, z);
        let alone: Vec<f32> = roots
            .iter()
            .map(|&(_, index)| router.sample_value(index, pos, &mut FillScratch::new()))
            .collect();

        for descending in [false, true] {
            let mut shared = FillScratch::new();
            let mut order: Vec<usize> = (0..roots.len()).collect();
            if descending {
                order.reverse();
            }
            for k in order {
                let (name, index) = roots[k];
                let got = router.sample_value(index, pos, &mut shared);
                assert_eq!(
                    got.to_bits(),
                    alone[k].to_bits(),
                    "{name} at {pos:?} through a shared cache (descending={descending}) \
                     gave {got}, alone it gives {}",
                    alone[k]
                );
            }
        }
    }
}

/// The slicing rewrite has to leave every parent-to-child edge either uniform
/// in its axes or bridged by slices pinning exactly the axes the child drops,
/// because that is what lets a consumer read the child's invariance off the
/// child alone.
#[test]
fn slicing_pins_every_axis_a_child_drops() {
    use super::ALL_AXES;
    use super::proto::{
        DensityFunctionHolder, InlineReference, ProtoDensityFunction, RewriteRule,
        SliceUniformAxes, is_uniform_axis_slice_leaf, sliced_axes,
    };

    let functions = load_density_functions_from_disk();
    let inline = InlineReference(&functions);
    let mut inserted = 0usize;
    for settings_file in ["overworld.json", "beta.json"] {
        let settings = settings_for(settings_file);
        let nr = &settings.noise_router;
        for holder in [
            &nr.temperature,
            &nr.vegetation,
            &nr.continents,
            &nr.erosion,
            &nr.depth,
            &nr.ridges,
            &nr.chunk_surface_level,
            &nr.final_density,
        ] {
            let original = inline.rewrite(holder);
            let rewritten = SliceUniformAxes::new(ALL_AXES).rewrite(&original);
            assert_eq!(rewritten.domain_axes(), original.domain_axes());

            let mut pending = vec![(ALL_AXES, rewritten.clone())];
            while let Some((parent_axes, holder)) = pending.pop() {
                if is_uniform_axis_slice_leaf(&holder) {
                    continue;
                }
                let axes = holder.domain_axes();
                let pinned = sliced_axes(&holder);
                inserted += pinned.count_ones() as usize;
                assert_eq!(
                    pinned,
                    parent_axes & !axes,
                    "{settings_file}: a child with axes {axes:#05b} under a parent with \
                     {parent_axes:#05b} pins {pinned:#05b}"
                );
                let mut inner = &holder;
                while let DensityFunctionHolder::Owned(f) = inner {
                    let ProtoDensityFunction::Slice { input, .. } = &**f else {
                        break;
                    };
                    inner = input;
                }
                if let DensityFunctionHolder::Owned(f) = inner {
                    f.visit_children(&mut |child| pending.push((axes, child.clone())));
                }
            }
        }
    }
    assert!(inserted > 0, "the rewrite never fired");
    println!("{inserted} axes pinned across both presets");
}

/// The rewrite rules run on the AST, so their axis masks have to be at least
/// as wide as the ones the compiled stack is checked against by sampling.
#[test]
fn ast_domain_axes_cover_the_compiled_ones() {
    use super::proto::{InlineReference, RewriteRule};

    let functions = load_density_functions_from_disk();
    let inline = InlineReference(&functions);
    for settings_file in ["overworld.json", "beta.json"] {
        let settings = settings_for(settings_file);
        let router = router_for(settings_file);
        let axes = router.domain_axes();
        let nr = &settings.noise_router;
        let roots = [
            ("temperature", &nr.temperature),
            ("vegetation", &nr.vegetation),
            ("continents", &nr.continents),
            ("erosion", &nr.erosion),
            ("depth", &nr.depth),
            ("ridges", &nr.ridges),
            ("chunk_surface_level", &nr.chunk_surface_level),
            ("final_density", &nr.final_density),
        ];
        for (name, holder) in roots {
            let ast_axes = inline.rewrite(holder).domain_axes();
            let index = router
                .roots()
                .into_iter()
                .find(|(root, _)| *root == name)
                .unwrap()
                .1;
            assert_eq!(
                axes[index] & !ast_axes,
                0,
                "{settings_file}: {name} compiled axes {:#05b} escape ast axes {ast_axes:#05b}",
                axes[index]
            );
        }
    }
}

/// A too-narrow axis mask silently corrupts terrain once a consumer pins the
/// axis it dropped, so every entry of every router is checked directly:
/// zeroing the coordinates outside the mask must not move the value at all.
#[test]
fn domain_axes_are_sound() {
    use super::{AXIS_X, AXIS_Y, AXIS_Z};

    let mut positions = vec![
        bevy_math::IVec3::new(0, 64, 0),
        bevy_math::IVec3::new(1, 5, 2),
        bevy_math::IVec3::new(13, -37, 7),
        bevy_math::IVec3::new(-19, 123, 41),
        bevy_math::IVec3::new(-4, 0, -9),
        bevy_math::IVec3::new(1000, 200, -1000),
        bevy_math::IVec3::new(-333, -60, 333),
        bevy_math::IVec3::new(7, 319, 7),
    ];
    // The slide gradients and the cell lattice both branch on Y, so the
    // sweep must land on and between their boundaries, not only near them.
    for y in [
        -64, -63, -56, -48, -40, -39, 8, 232, 239, 240, 248, 255, 256, 257,
    ] {
        positions.push(bevy_math::IVec3::new(3, y, -5));
        positions.push(bevy_math::IVec3::new(-16, y, 32));
    }
    let mut state = 0x2545_f491_4f6c_dd1du64;
    for _ in 0..256 {
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        positions.push(bevy_math::IVec3::new(
            (next() % 4096) as i32 - 2048,
            (next() % 512) as i32 - 128,
            (next() % 4096) as i32 - 2048,
        ));
    }

    for settings_file in ["overworld.json", "beta.json"] {
        let router = router_for(settings_file);
        let stack = &router.stack;
        let axes = router.domain_axes();

        for (i, entry) in stack.iter().enumerate() {
            entry.visit_input_indices(&mut |j| {
                assert!(j < i, "{settings_file}: entry {i} reads later entry {j}");
            });
        }

        let mut checked = 0usize;
        let mut restricted_entries = 0usize;
        for i in 0..stack.len() {
            if axes[i] != super::ALL_AXES {
                restricted_entries += 1;
            }
        }
        for &pos in &positions {
            for i in 0..stack.len() {
                let mask = axes[i];
                let pinned = bevy_math::IVec3::new(
                    if mask & AXIS_X != 0 { pos.x } else { 0 },
                    if mask & AXIS_Y != 0 { pos.y } else { 0 },
                    if mask & AXIS_Z != 0 { pos.z } else { 0 },
                );
                if pinned == pos {
                    continue;
                }
                checked += 1;
                let mut scratch = FillScratch::new();
                let full = router.sample_value(i, pos, &mut scratch);
                let restricted = router.sample_value(i, pinned, &mut scratch);
                assert_eq!(
                    full.to_bits(),
                    restricted.to_bits(),
                    "{settings_file}: entry {i} ({}) declares axes {mask:#05b} but {pos:?} -> {full} \
                     differs from {pinned:?} -> {restricted}",
                    router.node_labels[i],
                );
            }
        }

        let mut conservative = Vec::new();
        for i in 0..stack.len() {
            if axes[i] & AXIS_Y != 0 {
                assert!(
                    router.per_block[i],
                    "{settings_file}: entry {i} ({}) varies with Y but per_block says otherwise",
                    router.node_labels[i],
                );
            } else if router.per_block[i] {
                conservative.push((i, router.node_labels[i].clone()));
            }
        }
        assert!(restricted_entries > 0 && checked > 0);
        println!(
            "{settings_file}: {} entries, {restricted_entries} with a restricted domain, \
             {checked} pinned-coordinate comparisons, {} per_block-only",
            stack.len(),
            conservative.len(),
        );
        for (i, label) in &conservative {
            println!("  per_block over-approximates entry {i} ({label})");
        }
    }
}

/// Regression gate: the modern NoiseRouter built from overworld.json via
/// build_functions must remain structurally and numerically unchanged after
/// the data-driven preset refactor.  Any perturbation to the modern path
/// (wrong seed forwarding, different build_functions code path, changed
/// stack ordering) will cause this test to fail.
#[test]
fn overworld_router_unchanged() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/overworld.json"
    );
    let json = std::fs::read_to_string(path).expect("overworld.json must exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("overworld.json must deserialize");

    let functions: std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::ProtoDensityFunction,
    > = load_density_functions_from_disk();
    let noises: std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::proto::NoiseParam,
    > = load_noises_from_disk();

    let router = super::build_functions(
        &functions,
        &noises,
        &settings,
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );

    assert!(
        router.final_density_index() > 0,
        "modern router final_density_index must be non-zero (wired)"
    );
    assert!(
        router.column_boundary > 0,
        "modern router must have Zone A column-only entries"
    );

    let sample = router.sample_value(
        router.final_density_index,
        bevy_math::IVec3::new(0, 64, 0),
        &mut FillScratch::new(),
    );
    assert!(
        sample.is_finite(),
        "modern router sample at (0,64,0) must be finite"
    );

    assert_eq!(
        sample.to_bits(),
        3168561611u32,
        "modern router sample must match baseline (seed=2, pos=(0,64,0))"
    );
}

/// Regression gate: the Beta router must produce a numerically distinct
/// final_density sample from the modern overworld router, confirming the
/// two `build_functions` codepaths diverge as expected.
#[test]
fn beta_router_differs_from_modern() {
    let beta_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let beta_json = std::fs::read_to_string(beta_path).expect("beta.json must exist");
    let beta_settings: NoiseGeneratorSettings =
        serde_json::from_str(&beta_json).expect("beta.json must deserialize");

    let overworld_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/overworld.json"
    );
    let overworld_json =
        std::fs::read_to_string(overworld_path).expect("overworld.json must exist");
    let overworld_settings: NoiseGeneratorSettings =
        serde_json::from_str(&overworld_json).expect("overworld.json must deserialize");

    let functions = load_density_functions_from_disk();
    let noises = load_noises_from_disk();

    let modern_router = super::build_functions(
        &functions,
        &noises,
        &overworld_settings,
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );
    let beta_router = super::build_functions(
        &functions,
        &noises,
        &beta_settings,
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );

    let pos = bevy_math::IVec3::new(0, 64, 0);
    let modern_sample = modern_router.sample_value(
        modern_router.final_density_index,
        pos,
        &mut FillScratch::new(),
    );
    let beta_sample = beta_router.sample_value(
        beta_router.final_density_index,
        pos,
        &mut FillScratch::new(),
    );

    assert!(modern_sample.is_finite(), "modern sample must be finite");
    assert!(beta_sample.is_finite(), "beta sample must be finite");
    assert_ne!(
        modern_sample.to_bits(),
        beta_sample.to_bits(),
        "beta router must produce a different final_density than the modern router at (0,64,0)"
    );
}

/// Every shipped worldgen asset must deserialize. A loader that swallowed
/// errors once hid 42 of 62 unparseable density functions, so this asserts
/// the on-disk file count and the parsed count agree.
#[test]
fn whole_worldgen_corpus_parses() {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen");

    let functions = load_density_functions_from_disk();
    assert_eq!(
        functions.len(),
        count_json_files(&assets.join("density_function")),
        "every density_function asset must parse"
    );

    for (ident, function) in &functions {
        let reencoded = serde_json::to_string(function).unwrap();
        let roundtripped =
            serde_json::from_str::<crate::density_function::ProtoDensityFunction>(&reencoded)
                .unwrap_or_else(|e| panic!("{ident}: {e}\n{reencoded}"));
        assert_eq!(&roundtripped, function, "{ident} must round-trip");
    }

    let noises = load_noises_from_disk();
    assert_eq!(
        noises.len(),
        count_json_files(&assets.join("noise")),
        "every noise asset must parse"
    );

    let settings_dir = assets.join("noise_settings");
    let mut settings_count = 0;
    for entry in std::fs::read_dir(&settings_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).unwrap();
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        super::build_functions(
            &functions,
            &noises,
            &settings,
            2,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );
        settings_count += 1;
    }
    assert_eq!(settings_count, count_json_files(&settings_dir));
}
/// The three opcodes that evaluate at a substituted position used to leave
/// the register file and walk the stack recursively; they now run a stored
/// member list forward into a scratch level of their own. These digests were
/// captured from the recursive walk before it was deleted.
#[test]
fn substituted_position_evaluation_matches_the_recursive_walk() {
    const EXPECTED: &[(&str, &str, u64)] = &[
        ("caves.json", "chunk_surface_level", 0x14d5bceae7b5b1a5),
        ("caves.json", "continents", 0x14d5bceae7b5b1a5),
        ("caves.json", "depth", 0x14d5bceae7b5b1a5),
        ("caves.json", "erosion", 0x14d5bceae7b5b1a5),
        ("caves.json", "final_density", 0x59ee0276aa40e18e),
        ("caves.json", "ridges", 0x14d5bceae7b5b1a5),
        ("caves.json", "temperature", 0x14d5bceae7b5b1a5),
        ("caves.json", "vegetation", 0x14d5bceae7b5b1a5),
        ("end.json", "chunk_surface_level", 0x14d5bceae7b5b1a5),
        ("end.json", "continents", 0x14d5bceae7b5b1a5),
        ("end.json", "depth", 0x14d5bceae7b5b1a5),
        ("end.json", "erosion", 0xa6b0d65c5400b585),
        ("end.json", "final_density", 0x2e692d68276d0b0f),
        ("end.json", "ridges", 0x14d5bceae7b5b1a5),
        ("end.json", "temperature", 0x14d5bceae7b5b1a5),
        ("end.json", "vegetation", 0x14d5bceae7b5b1a5),
        (
            "floating_islands.json",
            "chunk_surface_level",
            0x14d5bceae7b5b1a5,
        ),
        ("floating_islands.json", "continents", 0x14d5bceae7b5b1a5),
        ("floating_islands.json", "depth", 0x14d5bceae7b5b1a5),
        ("floating_islands.json", "erosion", 0x14d5bceae7b5b1a5),
        ("floating_islands.json", "final_density", 0x34670f4acb0f9c53),
        ("floating_islands.json", "ridges", 0x14d5bceae7b5b1a5),
        ("floating_islands.json", "temperature", 0x14d5bceae7b5b1a5),
        ("floating_islands.json", "vegetation", 0x14d5bceae7b5b1a5),
        ("nether.json", "chunk_surface_level", 0x14d5bceae7b5b1a5),
        ("nether.json", "continents", 0x14d5bceae7b5b1a5),
        ("nether.json", "depth", 0x14d5bceae7b5b1a5),
        ("nether.json", "erosion", 0x14d5bceae7b5b1a5),
        ("nether.json", "final_density", 0x7fa120eb23cf8e2d),
        ("nether.json", "ridges", 0x14d5bceae7b5b1a5),
        ("nether.json", "temperature", 0xf3fb67beee416075),
        ("nether.json", "vegetation", 0x465fb5fd15685115),
        ("overworld.json", "chunk_surface_level", 0x797cbee7e2025605),
        ("overworld.json", "continents", 0x1abef3bb8dc94ee5),
        ("overworld.json", "depth", 0x80adf8d39580cc42),
        ("overworld.json", "erosion", 0x41903cf3b87af165),
        ("overworld.json", "final_density", 0xb5e2f421ac2d79f9),
        ("overworld.json", "ridges", 0x75248f177bef94a5),
        ("overworld.json", "temperature", 0x737146785309bbd5),
        ("overworld.json", "vegetation", 0x9a2b3c9a221c5875),
    ];

    let mut digests: std::collections::BTreeMap<(&str, &str), u64> =
        std::collections::BTreeMap::new();
    for settings in [
        "overworld.json",
        "end.json",
        "nether.json",
        "caves.json",
        "floating_islands.json",
    ] {
        let router = router_for(settings);
        let mut roots = router.roots();
        roots.sort_by_key(|&(_, index)| index);
        let mut scratch = FillScratch::new();
        for x in [-37i32, 0, 3, 16, 41] {
            for z in [-19i32, 0, 5, 12, 64] {
                for y in [-60i32, -1, 0, 3, 55, 64, 71, 200] {
                    let pos = bevy_math::IVec3::new(x, y, z);
                    for &(name, index) in roots.iter() {
                        let bits = router.sample_value(index, pos, &mut scratch).to_bits();
                        let digest = digests
                            .entry((settings, name))
                            .or_insert(0xcbf2_9ce4_8422_2325);
                        for shift in [0, 8, 16, 24] {
                            *digest ^= ((bits >> shift) & 0xff) as u64;
                            *digest = digest.wrapping_mul(0x100_0000_01b3);
                        }
                    }
                }
            }
        }
    }

    for &(settings, name, expected) in EXPECTED {
        assert_eq!(
            digests.get(&(settings, name)).copied(),
            Some(expected),
            "{settings} {name} moved"
        );
    }
    assert_eq!(digests.len(), EXPECTED.len());
}

/// `shift` is absent from the 26.3 corpus, so no oracle fixture reaches it.
/// Each variant is the same noise read through its own coordinate permutation,
/// which these identities pin: `shift_a` drops y, `shift_b` reads (z, x, 0).
#[test]
fn every_shift_variant_permutes_its_coordinates() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/noise_settings/overworld.json");
    let mut settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let noise = serde_json::json!("minecraft:offset");
    for (root, kind) in [
        ("temperature", "minecraft:shift"),
        ("vegetation", "minecraft:shift_a"),
        ("continents", "minecraft:shift_b"),
    ] {
        settings["noise_router"][root] =
            serde_json::json!({ "type": kind, "noise": noise.clone() });
    }
    let router = super::build_functions(
        &load_density_functions_from_disk(),
        &load_noises_from_disk(),
        &serde_json::from_value(settings).unwrap(),
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );

    let mut scratch = FillScratch::new();
    let shift = |p: IVec3, s: &mut FillScratch| router.sample_value(router.temperature_index, p, s);
    for pos in [
        IVec3::new(0, 0, 0),
        IVec3::new(13, -47, 5),
        IVec3::new(-8, 91, 200),
        IVec3::new(4, 4, 4),
    ] {
        let shift_a = router.sample_value(router.vegetation_index, pos, &mut scratch);
        let shift_b = router.sample_value(router.continents_index, pos, &mut scratch);
        assert_eq!(
            shift_a,
            shift(IVec3::new(pos.x, 0, pos.z), &mut scratch),
            "shift_a at {pos:?}"
        );
        assert_eq!(
            shift_b,
            shift(IVec3::new(pos.z, pos.x, 0), &mut scratch),
            "shift_b at {pos:?}"
        );
    }

    let off_diagonal = IVec3::new(13, -47, 5);
    let here = shift(off_diagonal, &mut scratch);
    assert_ne!(
        here,
        router.sample_value(router.vegetation_index, off_diagonal, &mut scratch)
    );
    assert_ne!(
        here,
        router.sample_value(router.continents_index, off_diagonal, &mut scratch)
    );
}
