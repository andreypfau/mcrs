#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

use std::path::Path;

use mcrs_minecraft_worldgen_headless::region::{SliceSpec, generate};
use mcrs_minecraft_worldgen_headless::{Depth, load};

fn surface_dump(name: &str) -> (u64, i32, i32, i32, i32, Vec<String>, Vec<u32>) {
    let bytes = mcrs_minecraft_fixtures::fixture("surface")
        .file(name)
        .dump(b"MCSURFC0");
    let mut at = 0usize;
    let mut take = |n: usize| {
        let s = &bytes[at..at + n];
        at += n;
        s.to_vec()
    };
    let u32_le = |b: Vec<u8>| u32::from_le_bytes(b.try_into().unwrap());
    let string = |take: &mut dyn FnMut(usize) -> Vec<u8>| {
        let len = u32::from_le_bytes(take(4).try_into().unwrap()) as usize;
        String::from_utf8(take(len)).unwrap()
    };
    let _settings = string(&mut take);
    let seed = i64::from_le_bytes(take(8).try_into().unwrap()) as u64;
    let chunk_x = u32_le(take(4)) as i32;
    let chunk_z = u32_le(take(4)) as i32;
    let min_y = u32_le(take(4)) as i32;
    let height = u32_le(take(4)) as i32;
    let palette = (0..u32_le(take(4))).map(|_| string(&mut take)).collect();
    let mut cells = Vec::new();
    for _ in 0..u32_le(take(4)) {
        let (id, run) = (u32_le(take(4)), u32_le(take(4)));
        cells.extend(std::iter::repeat_n(id, run as usize));
    }
    (seed, chunk_x, chunk_z, min_y, height, palette, cells)
}

#[test]
fn the_headless_overworld_matches_a_vanilla_chunk() {
    let (seed, cx, cz, min_y, height, palette, cells) = surface_dump("surface_s2_c3_-7.bin");
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let vanilla = mcrs_minecraft_fixtures::vanilla_dir();
    let world =
        load(&assets, vanilla, None, seed, Depth::Surface).unwrap_or_else(|e| panic!("{e}"));
    let slices: Vec<SliceSpec> = (0..16)
        .map(|x| SliceSpec {
            along_x: false,
            at: cx * 16 + x,
        })
        .collect();
    let region = generate(&world, (cx, cz, cx, cz), 1, &slices, None).unwrap();
    assert!(generate(&world, (cx, cz, cx, cz), 0, &slices, None).is_err());
    assert!(generate(&world, (cx + 1, cz, cx, cz), 1, &slices, None).is_err());
    assert_eq!(region.min_y, min_y);
    let mut differ = 0usize;
    let mut first = None;
    for x in 0..16 {
        let slice = &region.slices[x as usize];
        for z in 0..16 {
            for y in min_y..min_y + height {
                let want = palette[cells[((z * 16 + x) * height + (y - min_y)) as usize] as usize]
                    .split('[')
                    .next()
                    .unwrap();
                let row = (y - region.min_y) as usize;
                let got = &region.names[slice.cells[row * slice.len + z as usize] as usize];
                if got != want {
                    differ += 1;
                    first.get_or_insert_with(|| format!("{x},{y},{z}: want {want}, got {got}"));
                }
            }
        }
    }
    assert_eq!(
        differ, 0,
        "{differ} blocks differ from vanilla, first {first:?}"
    );
}
