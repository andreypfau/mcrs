//! Beta terrain noise at both value widths, same kernel, same lattice.
//!
//! One "column" here is what the Beta density functions ask of the Perlin
//! generators for one chunk: the low and high noises at 16 octaves each and the
//! selector at 8, all over the 5×17×5 density grid.

use std::hint::black_box;
use std::time::Instant;

use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::noise::gradient::NoiseFloat;
use mcrs_minecraft_worldgen::noise::improved_noise::ImprovedNoise;

const GRID_X: usize = 5;
const GRID_Y: usize = 17;
const GRID_Z: usize = 5;
const POINTS: usize = GRID_X * GRID_Y * GRID_Z;
const SCALE: f64 = 684.412;

fn column<V: NoiseFloat>(octaves: &[ImprovedNoise<V>], out: &mut [V], chunk_x: f64, chunk_z: f64) {
    for (index, noise) in octaves.iter().enumerate() {
        let d = (0.5f64).powi(index as i32 % 16);
        noise.fill_3d_bulk_at(
            out,
            chunk_x * 4.0,
            0.0,
            chunk_z * 4.0,
            GRID_X,
            GRID_Y,
            GRID_Z,
            SCALE * d,
            SCALE * d,
            SCALE * d,
            1.0 / d,
        );
    }
}

fn run<V: NoiseFloat>(label: &str, columns: usize) -> f64 {
    // 16 + 16 + 8 octaves, seeded once so both widths walk the same lattice.
    let mut rng = LegacyRandom::new(12345);
    let octaves: Vec<ImprovedNoise<V>> =
        (0..40).map(|_| ImprovedNoise::from_random(&mut rng)).collect();
    let mut out = vec![V::from_f64(0.0); POINTS];

    for i in 0..64 {
        column(&octaves, &mut out, i as f64, -(i as f64));
    }

    let started = Instant::now();
    for i in 0..columns {
        out.fill(V::from_f64(0.0));
        column(&octaves, &mut out, i as f64, (i / 32) as f64);
        black_box(&out);
    }
    let per_column = started.elapsed().as_secs_f64() * 1000.0 / columns as f64;
    println!("  {label:>3}: {per_column:.4} ms per column ({POINTS} points x 40 octaves)");
    per_column
}

fn main() {
    let columns: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4000);

    println!("beta terrain noise, {columns} columns:");
    let f64_ms = run::<f64>("f64", columns);
    let f32_ms = run::<f32>("f32", columns);
    println!("  f32 is {:.2}x the speed of f64", f64_ms / f32_ms);
}
