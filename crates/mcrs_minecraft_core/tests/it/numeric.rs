#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

use std::sync::LazyLock;

use mcrs_minecraft_core::numeric;
use mcrs_minecraft_fixtures::fixture;
use serde::Deserialize;
use sha2::{Digest, Sha256};

type Rows = Vec<Vec<String>>;

#[derive(Deserialize)]
struct Artefact {
    sine_table: SineTable,
    floor: Rows,
    floor_div_mod: Rows,
    min_max_signum: Rows,
    clamp: Rows,
    log_pow: Rows,
    lerp: Lerp,
    lerp_int: Rows,
    map: Map,
    wrap_degrees: Rows,
    sin_cos: Rows,
    inverse_sqrt_estimate: Rows,
    atan2: Rows,
    hsv_to_rgb: Rows,
}

#[derive(Deserialize)]
struct SineTable {
    entries: usize,
    sha256_of_le_bits: String,
    probes: Vec<(usize, String)>,
}

#[derive(Deserialize)]
struct Lerp {
    f32: Rows,
    f64: Rows,
}

#[derive(Deserialize)]
struct Map {
    clamped_f32: Rows,
    clamped_f64: Rows,
    unclamped_f64: Rows,
}

static ARTEFACT: LazyLock<Artefact> =
    LazyLock::new(|| serde_json::from_str(&fixture("numeric").text()).unwrap());

fn f32_of(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
}

fn f64_of(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).unwrap())
}

fn i32_of(text: &str) -> i32 {
    text.parse().unwrap()
}

fn same_f32(actual: f32, expected: &str) -> bool {
    let expected = f32_of(expected);
    actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan())
}

fn same_f64(actual: f64, expected: &str) -> bool {
    let expected = f64_of(expected);
    actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan())
}

fn check(section: &str, rows: &Rows, agrees: impl Fn(&[String]) -> bool) {
    assert!(!rows.is_empty(), "the section {section} has rows");
    for (index, row) in rows.iter().enumerate() {
        assert!(agrees(row), "{section} row {index} disagrees: {row:?}");
    }
}

fn sha256_hex(table: &[f32]) -> String {
    let bytes: Vec<u8> = table.iter().flat_map(|entry| entry.to_le_bytes()).collect();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn sine_tables_match_the_artefact() {
    let artefact = &ARTEFACT.sine_table;
    let current = numeric::sine_table();
    let earlier = numeric::earlier_sine_table();
    assert_eq!(current.len(), artefact.entries);
    assert_eq!(sha256_hex(current), artefact.sha256_of_le_bits);
    for (index, bits) in &artefact.probes {
        assert!(same_f32(current[*index], bits), "sine_table probe {index}");
    }
    let differences: Vec<usize> = (0..numeric::SINE_TABLE_LEN)
        .filter(|&index| current[index].to_bits() != earlier[index].to_bits())
        .collect();
    assert_eq!(differences, [32768]);
    assert_eq!(earlier[32768].to_bits(), 0x250d_3132);
}

#[test]
fn conversions_match_the_artefact() {
    check("floor", &ARTEFACT.floor, |row| {
        let x = f32_of(&row[0]);
        numeric::floor_to_i32(x) == i32_of(&row[1])
            && numeric::ceil_to_i32(x) == i32_of(&row[2])
            && numeric::truncate_to_i32(x) == i32_of(&row[3])
    });
    check("floor_div_mod", &ARTEFACT.floor_div_mod, |row| {
        let (a, b) = (i32_of(&row[0]), i32_of(&row[1]));
        numeric::floor_div(a, b) == i32_of(&row[2]) && numeric::floor_mod(a, b) == i32_of(&row[3])
    });
}

#[test]
fn comparisons_match_the_artefact() {
    check("min_max_signum", &ARTEFACT.min_max_signum, |row| {
        let (a, b) = (f32_of(&row[0]), f32_of(&row[1]));
        same_f32(numeric::min_f32(a, b), &row[2])
            && same_f32(numeric::max_f32(a, b), &row[3])
            && same_f32(numeric::sign_f32(a), &row[4])
    });
    check("clamp", &ARTEFACT.clamp, |row| {
        let [value, low, high] = [&row[0], &row[1], &row[2]].map(|hex| f32_of(hex));
        same_f32(numeric::clamp_f32(value, low, high), &row[3])
    });
}

#[test]
fn logarithm_and_power_match_the_artefact() {
    check("log_pow", &ARTEFACT.log_pow, |row| {
        let (a, b) = (f32_of(&row[0]), f32_of(&row[1]));
        same_f32(numeric::ln_f32(a), &row[2]) && same_f32(numeric::pow_f32(a, b), &row[3])
    });
}

#[test]
fn interpolations_match_the_artefact() {
    check("lerp.f32", &ARTEFACT.lerp.f32, |row| {
        let [t, start, end] = [&row[0], &row[1], &row[2]].map(|hex| f32_of(hex));
        same_f32(numeric::lerp(t, start, end), &row[3])
    });
    check("lerp.f64", &ARTEFACT.lerp.f64, |row| {
        let [t, start, end] = [&row[0], &row[1], &row[2]].map(|hex| f64_of(hex));
        same_f64(numeric::lerp(t, start, end), &row[3])
    });
    check("lerp_int", &ARTEFACT.lerp_int, |row| {
        numeric::lerp_i32(f32_of(&row[0]), i32_of(&row[1]), i32_of(&row[2])) == i32_of(&row[3])
    });
    let map = &ARTEFACT.map;
    check("map.clamped_f32", &map.clamped_f32, |row| {
        let [v, a, b, c, e] = [&row[0], &row[1], &row[2], &row[3], &row[4]].map(|hex| f32_of(hex));
        same_f32(numeric::remap_clamped(v, a, b, c, e), &row[5])
    });
    check("map.clamped_f64", &map.clamped_f64, |row| {
        let [v, a, b, c, e] = [&row[0], &row[1], &row[2], &row[3], &row[4]].map(|hex| f64_of(hex));
        same_f64(numeric::remap_clamped(v, a, b, c, e), &row[5])
    });
    check("map.unclamped_f64", &map.unclamped_f64, |row| {
        let [v, a, b, c, e] = [&row[0], &row[1], &row[2], &row[3], &row[4]].map(|hex| f64_of(hex));
        same_f64(numeric::remap(v, a, b, c, e), &row[5])
    });
}

#[test]
fn angles_match_the_artefact() {
    check("wrap_degrees", &ARTEFACT.wrap_degrees, |row| {
        same_f32(numeric::wrap_degrees(f32_of(&row[0])), &row[1])
    });
    check("sin_cos", &ARTEFACT.sin_cos, |row| {
        let x = f64_of(&row[0]);
        same_f32(numeric::sin_nearest(x), &row[1]) && same_f32(numeric::cos_nearest(x), &row[2])
    });
}

#[test]
fn arctangent_matches_the_artefact() {
    check(
        "inverse_sqrt_estimate",
        &ARTEFACT.inverse_sqrt_estimate,
        |row| same_f64(numeric::inverse_sqrt_estimate(f64_of(&row[0])), &row[1]),
    );
    check("atan2", &ARTEFACT.atan2, |row| {
        same_f64(numeric::atan2(f64_of(&row[0]), f64_of(&row[1])), &row[2])
    });
}

#[test]
fn colours_match_the_artefact() {
    check("hsv_to_rgb", &ARTEFACT.hsv_to_rgb, |row| {
        let [hue, saturation, value] = [&row[0], &row[1], &row[2]].map(|hex| f32_of(hex));
        numeric::hsv_to_rgb(hue, saturation, value) == i32_of(&row[3])
    });
}
