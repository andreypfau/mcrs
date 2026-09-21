use std::collections::HashMap;
use std::collections::HashSet;

use mcrs_minecraft_mesh::block::SpriteRef;

use super::bake::ItemQuad;
use crate::bake::{Dir, VariantRotation, face_geometry};
use crate::model::{Element, Face};

const MIN_Z: f32 = 7.5;
const MAX_Z: f32 = 8.5;
const UV_SHRINK: f32 = 0.1;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Up,
    Down,
    Left,
    Right,
}

impl Side {
    const ALL: [Side; 4] = [Side::Up, Side::Down, Side::Left, Side::Right];

    fn step(self) -> (i32, i32) {
        match self {
            Side::Up => (0, -1),
            Side::Down => (0, 1),
            Side::Left => (-1, 0),
            Side::Right => (1, 0),
        }
    }

    fn direction(self) -> Dir {
        match self {
            Side::Up => Dir::Up,
            Side::Down => Dir::Down,
            Side::Left => Dir::East,
            Side::Right => Dir::West,
        }
    }
}

/// Extrudes one `layerN` sprite of a `builtin/generated` item: a front and back quad plus a
/// one-pixel side quad wherever an opaque texel of any frame borders a transparent one.
pub fn extrude(
    sprite: SpriteRef,
    side: u32,
    frames: &[&[u8]],
    layer: u32,
) -> Result<Vec<ItemQuad>, String> {
    let mut quads = vec![
        quad([0.0, 0.0, MIN_Z], [16.0, 16.0, MAX_Z], [0.0, 0.0, 16.0, 16.0], Dir::South, sprite, layer)?,
        quad([0.0, 0.0, MIN_Z], [16.0, 16.0, MAX_Z], [16.0, 0.0, 0.0, 16.0], Dir::North, sprite, layer)?,
    ];
    let scale = 16.0 / side as f32;
    for (facing, x, y) in side_faces(side, frames) {
        let (x, y) = (x as f32, y as f32);
        let u0 = x + UV_SHRINK;
        let u1 = x + 1.0 - UV_SHRINK;
        let (v0, v1) = match facing {
            Side::Up | Side::Down => (y + UV_SHRINK, y + 1.0 - UV_SHRINK),
            Side::Left | Side::Right => (y + 1.0 - UV_SHRINK, y + UV_SHRINK),
        };
        let (start_x, start_y, end_x, end_y) = match facing {
            Side::Up => (x, y, x + 1.0, y),
            Side::Down => (x, y + 1.0, x + 1.0, y + 1.0),
            Side::Left => (x, y, x, y + 1.0),
            Side::Right => (x + 1.0, y, x + 1.0, y + 1.0),
        };
        let (start_x, end_x) = (start_x * scale, end_x * scale);
        let (start_y, end_y) = (16.0 - start_y * scale, 16.0 - end_y * scale);
        let (from, to) = match facing {
            Side::Up => ([start_x, start_y, MIN_Z], [end_x, start_y, MAX_Z]),
            Side::Down => ([start_x, end_y, MIN_Z], [end_x, end_y, MAX_Z]),
            Side::Left => ([start_x, start_y, MIN_Z], [start_x, end_y, MAX_Z]),
            Side::Right => ([end_x, start_y, MIN_Z], [end_x, end_y, MAX_Z]),
        };
        let uv = [u0 * scale, v0 * scale, u1 * scale, v1 * scale];
        quads.push(quad(from, to, uv, facing.direction(), sprite, layer)?);
    }
    Ok(quads)
}

fn side_faces(side: u32, frames: &[&[u8]]) -> Vec<(Side, u32, u32)> {
    let transparent = |frame: &[u8], x: i32, y: i32| {
        if x < 0 || y < 0 || x >= side as i32 || y >= side as i32 {
            return true;
        }
        frame[((y as u32 * side + x as u32) * 4 + 3) as usize] == 0
    };
    let mut seen = HashSet::new();
    let mut faces = Vec::new();
    for frame in frames {
        for y in 0..side {
            for x in 0..side {
                if transparent(frame, x as i32, y as i32) {
                    continue;
                }
                for facing in Side::ALL {
                    let (dx, dy) = facing.step();
                    if transparent(frame, x as i32 + dx, y as i32 + dy)
                        && seen.insert((facing, x, y))
                    {
                        faces.push((facing, x, y));
                    }
                }
            }
        }
    }
    faces
}

fn quad(
    from: [f32; 3],
    to: [f32; 3],
    uv: [f32; 4],
    dir: Dir,
    sprite: SpriteRef,
    layer: u32,
) -> Result<ItemQuad, String> {
    let element = Element {
        from,
        to,
        rotation: None,
        shade: true,
        faces: HashMap::new(),
    };
    let face = Face {
        texture: String::new(),
        uv: Some(uv),
        cullface: None,
        rotation: 0,
        tint_index: Some(layer),
    };
    let geometry = face_geometry(&element, dir, &face, VariantRotation::default(), false)?;
    Ok(ItemQuad {
        positions: geometry.positions,
        uvs: geometry.uvs,
        dir: geometry.facing,
        sprite,
        tint: Some(layer),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    fn pixel_sprite(side: u32, opaque: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        let mut data = vec![0; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                if opaque(x, y) {
                    data[((y * side + x) * 4 + 3) as usize] = 255;
                }
            }
        }
        data
    }

    #[test]
    fn a_single_opaque_texel_extrudes_four_sides_around_the_two_faces() {
        let sprite = SpriteRef { array: 0, layer: 3 };
        let pixels = pixel_sprite(16, |x, y| x == 2 && y == 5);
        let quads = extrude(sprite, 16, &[&pixels], 1).unwrap();
        assert_eq!(quads.len(), 6);
        assert!(quads.iter().all(|q| q.tint == Some(1) && q.sprite == sprite));
        assert_eq!(quads[0].dir, Dir::South);
        assert_eq!(quads[1].dir, Dir::North);
        let dirs: Vec<Dir> = quads[2..].iter().map(|q| q.dir).collect();
        assert_eq!(dirs, [Dir::Up, Dir::Down, Dir::West, Dir::East]);
        let up = &quads[2];
        let (min, max) = up.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(min, max), p| (min.min(*p), max.max(*p)),
        );
        assert_eq!(min, Vec3::new(2.0 / 16.0, 11.0 / 16.0, 7.5 / 16.0));
        assert_eq!(max, Vec3::new(3.0 / 16.0, 11.0 / 16.0, 8.5 / 16.0));
        for uv in up.uvs {
            assert!(uv[0] >= 2.1 / 16.0 - 1e-6 && uv[0] <= 2.9 / 16.0 + 1e-6, "{uv:?}");
            assert!(uv[1] >= 5.1 / 16.0 - 1e-6 && uv[1] <= 5.9 / 16.0 + 1e-6, "{uv:?}");
        }
    }

    #[test]
    fn every_frame_contributes_and_shared_edges_are_emitted_once() {
        let sprite = SpriteRef { array: 0, layer: 0 };
        let a = pixel_sprite(4, |x, y| x == 0 && y == 0);
        let b = pixel_sprite(4, |x, y| x <= 1 && y == 0);
        let quads = extrude(sprite, 4, &[&a, &b], 0).unwrap();
        assert_eq!(quads.len() - 2, 4 + 3);
        let scale = 4.0;
        let widest = quads[2..]
            .iter()
            .map(|q| q.positions.iter().map(|p| p.x).fold(0.0, f32::max))
            .fold(0.0, f32::max);
        assert_eq!(widest, 2.0 * scale / 16.0);
    }

    #[test]
    fn side_faces_sit_on_the_silhouette_edges_not_between_opaque_texels() {
        let sprite = SpriteRef { array: 0, layer: 0 };
        let column = pixel_sprite(2, |x, _| x == 0);
        let quads = extrude(sprite, 2, &[&column], 0).unwrap();
        let edge = |dir: Dir, axis: fn(&Vec3) -> f32| -> Vec<f32> {
            quads[2..]
                .iter()
                .filter(|q| q.dir == dir)
                .map(|q| axis(&q.positions[0]))
                .collect()
        };
        assert_eq!(edge(Dir::Up, |p| p.y), [1.0]);
        assert_eq!(edge(Dir::Down, |p| p.y), [0.0]);
        assert_eq!(edge(Dir::West, |p| p.x), [0.0, 0.0]);
        assert_eq!(edge(Dir::East, |p| p.x), [0.5, 0.5]);
    }
}
