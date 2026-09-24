use crate::StructurePlacement;
use crate::placement::SpreadPlacement;
use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, ColumnPos};

use super::frozen::{SetId, StructureId};

pub const MAX_SEARCH_RADIUS: i32 = 100;

pub struct LocatePlacement<'a> {
    pub set: SetId,
    pub placement: &'a StructurePlacement,
    pub structures: Vec<StructureId>,
}

pub fn locate<'r>(
    seed: i64,
    origin: IVec3,
    radius: i32,
    placements: &[LocatePlacement<'_>],
    rings: impl Fn(SetId) -> Option<&'r [ColumnPos]>,
    mut present: impl FnMut(SetId, ColumnPos, StructureId) -> bool,
) -> Option<(IVec3, StructureId)> {
    let mut nearest = None;
    let mut distance_sqr = f64::MAX;
    let mut spread = Vec::with_capacity(placements.len());
    for entry in placements {
        match entry.placement {
            StructurePlacement::ConcentricRings { .. } => {
                let positions =
                    rings(entry.set).expect("a live concentric_rings set has ring positions");
                let mut closest = None;
                let mut closest_sqr = f64::MAX;
                for &chunk in positions {
                    let centre = IVec3::new(chunk.middle_block_x(), 32, chunk.middle_block_z());
                    let d = centre.as_dvec3().distance_squared(origin.as_dvec3());
                    if (closest.is_none() || d < closest_sqr)
                        && let Some(hit) = generating_at(entry, chunk, &mut present)
                    {
                        closest = Some(hit);
                        closest_sqr = d;
                    }
                }
                if let Some(hit) = closest {
                    let d = origin.as_dvec3().distance_squared(hit.0.as_dvec3());
                    if d < distance_sqr {
                        distance_sqr = d;
                        nearest = Some(hit);
                    }
                }
            }
            StructurePlacement::RandomSpread { .. } => spread.push(entry),
            StructurePlacement::DimensionOrigin {} => {}
        }
    }
    if spread.is_empty() {
        return nearest;
    }
    let chunk_origin = ColumnPos::from(BlockPos::from(origin));
    for r in 0..=radius {
        let mut found = false;
        for entry in &spread {
            let config = SpreadPlacement::of(entry.placement).expect("a random_spread placement");
            let Some(hit) = perimeter_hit(seed, chunk_origin, r, entry, &config, &mut present)
            else {
                continue;
            };
            found = true;
            let d = origin.as_dvec3().distance_squared(hit.0.as_dvec3());
            if d < distance_sqr {
                distance_sqr = d;
                nearest = Some(hit);
            }
        }
        if found {
            return nearest;
        }
    }
    nearest
}

fn perimeter_hit(
    seed: i64,
    chunk_origin: ColumnPos,
    radius: i32,
    entry: &LocatePlacement<'_>,
    config: &SpreadPlacement,
    present: &mut impl FnMut(SetId, ColumnPos, StructureId) -> bool,
) -> Option<(IVec3, StructureId)> {
    for x in -radius..=radius {
        let x_edge = x == -radius || x == radius;
        for z in -radius..=radius {
            let z_edge = z == -radius || z == radius;
            if !(x_edge || z_edge) {
                continue;
            }
            let sector_x = chunk_origin.x.wrapping_add(config.spacing.wrapping_mul(x));
            let sector_z = chunk_origin.z.wrapping_add(config.spacing.wrapping_mul(z));
            let target = config.potential_chunk(seed, ColumnPos::new(sector_x, sector_z));
            if let Some(hit) = generating_at(entry, target, present) {
                return Some(hit);
            }
        }
    }
    None
}

fn generating_at(
    entry: &LocatePlacement<'_>,
    chunk: ColumnPos,
    present: &mut impl FnMut(SetId, ColumnPos, StructureId) -> bool,
) -> Option<(IVec3, StructureId)> {
    entry
        .structures
        .iter()
        .copied()
        .find(|&structure| present(entry.set, chunk, structure))
        .map(|structure| (locate_pos(entry.placement, chunk), structure))
}

pub fn locate_pos(placement: &StructurePlacement, chunk: ColumnPos) -> IVec3 {
    let offset = match placement {
        StructurePlacement::RandomSpread { spreading, .. }
        | StructurePlacement::ConcentricRings { spreading, .. } => spreading.locate_offset.0,
        StructurePlacement::DimensionOrigin {} => [0; 3],
    };
    IVec3::new(
        chunk.min_block_x().wrapping_add(offset[0]),
        offset[1],
        chunk.min_block_z().wrapping_add(offset[2]),
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const SEED: i64 = 0x5EED;
    const ORIGIN: IVec3 = IVec3::new(0, 64, 0);
    const A: SetId = SetId(0);
    const B: SetId = SetId(1);
    const RINGS: SetId = SetId(2);
    const S: StructureId = StructureId(0);

    fn placement(json: &str) -> StructurePlacement {
        serde_json::from_str(json).unwrap()
    }

    fn wide() -> StructurePlacement {
        placement(r#"{"type":"minecraft:random_spread","salt":1,"spacing":4,"separation":0}"#)
    }

    fn narrow() -> StructurePlacement {
        placement(r#"{"type":"minecraft:random_spread","salt":2,"spacing":2,"separation":0}"#)
    }

    fn rings() -> StructurePlacement {
        placement(
            r##"{"type":"minecraft:concentric_rings","salt":0,"distance":32,"spread":3,"count":128,"preferred_biomes":"#minecraft:x"}"##,
        )
    }

    fn spread_entry<'a>(set: SetId, placement: &'a StructurePlacement) -> LocatePlacement<'a> {
        LocatePlacement {
            set,
            placement,
            structures: vec![S],
        }
    }

    fn cell(placement: &StructurePlacement, sector: ColumnPos) -> ColumnPos {
        SpreadPlacement::of(placement)
            .unwrap()
            .potential_chunk(SEED, sector)
    }

    fn run(
        placements: &[LocatePlacement<'_>],
        ring_positions: &[ColumnPos],
        starts: &[(SetId, ColumnPos)],
    ) -> (Option<(IVec3, StructureId)>, Vec<(SetId, ColumnPos)>) {
        let asked = RefCell::new(Vec::new());
        let found = locate(
            SEED,
            ORIGIN,
            MAX_SEARCH_RADIUS,
            placements,
            |set| (set == RINGS).then_some(ring_positions),
            |set, chunk, structure| {
                assert_eq!(structure, S);
                asked.borrow_mut().push((set, chunk));
                starts.contains(&(set, chunk))
            },
        );
        (found, asked.into_inner())
    }

    #[test]
    fn the_first_non_empty_radius_answers_even_when_a_closer_start_lies_beyond_it() {
        let (wide, narrow) = (wide(), narrow());
        let far = cell(&wide, ColumnPos::new(4, 4));
        let near = cell(&narrow, ColumnPos::new(4, 0));
        assert!(
            ORIGIN
                .as_dvec3()
                .distance_squared(locate_pos(&narrow, near).as_dvec3())
                < ORIGIN
                    .as_dvec3()
                    .distance_squared(locate_pos(&wide, far).as_dvec3())
        );

        let placements = [spread_entry(A, &wide), spread_entry(B, &narrow)];
        let (found, asked) = run(&placements, &[], &[(A, far), (B, near)]);
        assert_eq!(found, Some((locate_pos(&wide, far), S)));
        assert_eq!(asked.len(), 2 + 8 + 8);
        assert_eq!(asked[0], (A, cell(&wide, ColumnPos::new(0, 0))));
        assert_eq!(asked[1], (B, cell(&narrow, ColumnPos::new(0, 0))));
        assert_eq!(asked[9], (A, far));
        assert!(!asked.contains(&(B, near)));
    }

    #[test]
    fn hits_on_the_same_radius_are_ranked_by_distance() {
        let (wide, narrow) = (wide(), narrow());
        let far = cell(&wide, ColumnPos::new(4, 4));
        let near = cell(&narrow, ColumnPos::new(2, 0));
        assert!(
            ORIGIN
                .as_dvec3()
                .distance_squared(locate_pos(&narrow, near).as_dvec3())
                < ORIGIN
                    .as_dvec3()
                    .distance_squared(locate_pos(&wide, far).as_dvec3())
        );

        let placements = [spread_entry(A, &wide), spread_entry(B, &narrow)];
        let (found, asked) = run(&placements, &[], &[(A, far), (B, near)]);
        assert_eq!(found, Some((locate_pos(&narrow, near), S)));
        assert_eq!(asked.len(), 2 + 8 + 7);
        assert_eq!(asked.last(), Some(&(B, near)));
    }

    #[test]
    fn the_perimeter_is_walked_x_then_z_ascending() {
        let wide = wide();
        let placements = [spread_entry(A, &wide)];
        let (found, asked) = run(&placements, &[], &[]);
        assert_eq!(found, None);
        let expected: Vec<_> = (0..=MAX_SEARCH_RADIUS)
            .flat_map(|r| {
                (-r..=r).flat_map(move |x| {
                    (-r..=r)
                        .filter(move |&z| x == -r || x == r || z == -r || z == r)
                        .map(move |z| (x, z))
                })
            })
            .map(|(x, z)| (A, cell(&wide, ColumnPos::new(4 * x, 4 * z))))
            .collect();
        assert_eq!(asked, expected);
    }

    #[test]
    fn a_ring_answer_only_loses_to_a_strictly_closer_spread_hit() {
        let (wide, rings) = (wide(), rings());
        let here = cell(&wide, ColumnPos::new(0, 0));
        let ring_entry = LocatePlacement {
            set: RINGS,
            placement: &rings,
            structures: vec![S],
        };
        let placements = [spread_entry(A, &wide), ring_entry];

        let (found, asked) = run(
            &placements,
            &[ColumnPos::new(10, 10), ColumnPos::new(20, 20)],
            &[(RINGS, ColumnPos::new(10, 10)), (A, here)],
        );
        assert_eq!(found, Some((locate_pos(&wide, here), S)));
        assert_eq!(asked, vec![(RINGS, ColumnPos::new(10, 10)), (A, here)]);

        let (found, asked) = run(
            &placements,
            &[ColumnPos::new(20, 20), ColumnPos::new(0, 0)],
            &[(RINGS, ColumnPos::new(0, 0)), (A, here)],
        );
        assert_eq!(found, Some((locate_pos(&rings, ColumnPos::new(0, 0)), S)));
        assert_eq!(
            asked,
            vec![
                (RINGS, ColumnPos::new(20, 20)),
                (RINGS, ColumnPos::new(0, 0)),
                (A, here)
            ]
        );

        let (found, asked) = run(
            &placements,
            &[ColumnPos::new(10, 10)],
            &[(RINGS, ColumnPos::new(10, 10))],
        );
        assert_eq!(found, Some((locate_pos(&rings, ColumnPos::new(10, 10)), S)));
        assert_eq!(
            asked.len(),
            1 + (0..=MAX_SEARCH_RADIUS)
                .map(|r| if r == 0 { 1 } else { 8 * r })
                .sum::<i32>() as usize
        );
    }
}
