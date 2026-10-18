use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, LocalPos, SectionPos};
use mcrs_minecraft_volume::{
    BlockVolume, BlockVolumeMut, BoxVolume, PaletteBuffer, SectionBlocks, SectionBlocksMut,
    SectionBox, SparseVolume, VisitOrder, Volume,
};

fn stone() -> BlockStateId {
    crate::states(3)[1]
}

fn ore() -> BlockStateId {
    crate::states(3)[2]
}

fn sorted<I>(palette: I) -> Vec<(BlockStateId, u16)>
where
    I: Iterator<Item = (BlockStateId, u16)>,
{
    let mut entries: Vec<_> = palette.collect();
    entries.sort();
    entries
}

#[test]
fn a_paletted_section_fills_inclusive_boxes_and_counts_its_palette() {
    let at = SectionPos::new(0, 0, 0);
    let mut volume = SparseVolume::unbounded();
    volume.insert(at, BlockStateId::AIR);
    let mut section = volume.section_mut(at).expect("present");
    section.fill_box(LocalPos::new(0, 0, 0), LocalPos::new(15, 3, 15), stone());
    section.fill_box(LocalPos::new(2, 1, 2), LocalPos::new(3, 2, 3), ore());
    assert_eq!(
        section.set(LocalPos::new(15, 15, 15), ore()),
        BlockStateId::AIR
    );

    let section = volume.section(at).expect("present");
    assert_eq!(section.uniform(), None);
    let mut buffer = PaletteBuffer::default();
    let palette = section.palette(&mut buffer);
    assert_eq!(
        sorted(palette.iter()),
        vec![
            (BlockStateId::AIR, 4096 - 1024 - 1),
            (stone(), 1024 - 8),
            (ore(), 9)
        ]
    );
    assert!(palette.any(|state| state == ore()));
    assert_eq!(palette.count(|state| state != BlockStateId::AIR), 1024 + 1);

    let mut cells = [crate::states(4)[3]; SectionPos::VOLUME];
    section.copy_to(&mut cells);
    for at in LocalPos::all() {
        assert_eq!(cells[at.index()], section.get(at), "{at:?}");
    }
    assert_eq!(cells[LocalPos::new(2, 2, 3).index()], ore());

    volume.section_mut(at).expect("present").fill(stone());
    assert_eq!(
        volume.section(at).expect("present").uniform(),
        Some(stone())
    );
}

#[test]
fn a_dense_section_builds_the_palette_its_paletted_twin_keeps() {
    let bounds = BoundingBox {
        min: BlockPos::new(0, 0, 0),
        max: BlockPos::new(15, 15, 15),
    };
    let mut dense = BoxVolume::filled(bounds, BlockStateId::AIR).expect("one section");
    let mut paletted = SparseVolume::unbounded();
    let at = SectionPos::new(0, 0, 0);
    paletted.insert(at, BlockStateId::AIR);
    for (i, pos) in [(1, 2, 3), (4, 5, 6), (7, 8, 9), (1, 1, 1)]
        .into_iter()
        .enumerate()
    {
        let state = crate::states(5)[if i % 2 == 0 { 3 } else { 4 }];
        let pos = BlockPos::from(pos);
        dense.set_block(pos, state).expect("inside");
        paletted.set_block(pos, state).expect("present");
    }

    let mut buffer = PaletteBuffer::default();
    let dense_section = dense.section(at).expect("inside");
    let built = sorted(dense_section.palette(&mut buffer).iter());
    let mut unused = PaletteBuffer::default();
    let paletted_section = paletted.section(at).expect("present");
    let kept = sorted(paletted_section.palette(&mut unused).iter());
    assert_eq!(built, kept);
    assert_eq!(dense_section.uniform(), None);
}

#[test]
fn a_section_box_walks_section_major_or_column_major() {
    let area = SectionBox {
        min: SectionPos::new(0, -1, 0),
        max: SectionPos::new(1, 0, 1),
    };
    let section_major: Vec<_> = area
        .iter(VisitOrder::SectionMajor)
        .map(|s| (s.x, s.y, s.z))
        .collect();
    assert_eq!(&section_major[..3], &[(0, -1, 0), (1, -1, 0), (0, -1, 1)]);
    let column_major: Vec<_> = area
        .iter(VisitOrder::ColumnMajor)
        .map(|s| (s.x, s.y, s.z))
        .collect();
    assert_eq!(&column_major[..3], &[(0, -1, 0), (0, 0, 0), (0, -1, 1)]);
    assert_eq!(area.len(), 8);
    assert_eq!(section_major.len(), 8);

    let everything = SectionBox {
        min: SectionPos::new(i32::MIN, i32::MIN, i32::MIN),
        max: SectionPos::new(i32::MAX, i32::MAX, i32::MAX),
    };
    assert_eq!(everything.len(), u64::MAX);
    let inverted = SectionBox {
        min: SectionPos::new(1, 0, 0),
        max: SectionPos::new(0, 0, 0),
    };
    assert!(inverted.is_empty());
    assert_eq!(inverted.iter(VisitOrder::SectionMajor).count(), 0);
}

fn origin(at: SectionPos) -> BlockPos {
    BlockPos::new(at.x * 16, at.y * 16, at.z * 16)
}

fn numbered(bounds: BoundingBox) -> BoxVolume {
    let table = crate::states(10_000);
    let mut volume = BoxVolume::filled(bounds, BlockStateId::AIR).expect("small");
    let cells: Vec<BlockPos> = volume.iter().map(|(at, _)| at).collect();
    for (n, at) in cells.into_iter().enumerate() {
        volume.set_block(at, table[n + 1]).expect("inside");
    }
    volume
}

#[test]
fn a_box_section_reads_the_cells_the_box_reads_at_any_origin() {
    let corners = [
        BoundingBox {
            min: BlockPos::new(-20, -18, -33),
            max: BlockPos::new(3, 1, -15),
        },
        BoundingBox {
            min: BlockPos::new(i32::MAX - 20, i32::MAX - 17, i32::MAX - 5),
            max: BlockPos::new(i32::MAX, i32::MAX, i32::MAX),
        },
    ];
    for bounds in corners {
        let volume = numbered(bounds);
        for at in SectionBox::covering(bounds).iter(VisitOrder::SectionMajor) {
            let section = volume.section(at).expect("touched");
            let base = origin(at);
            for local in LocalPos::all() {
                let pos = BlockPos::new(
                    base.x + i32::from(local.x()),
                    base.y + i32::from(local.y()),
                    base.z + i32::from(local.z()),
                );
                let expected = volume.block(pos).unwrap_or(BlockStateId::AIR);
                assert_eq!(section.get(local), expected, "{pos}");
            }
        }
    }
}

#[test]
fn a_cut_box_section_reads_default_and_drops_writes_past_the_box() {
    let bounds = BoundingBox {
        min: BlockPos::new(-2, 0, 5),
        max: BlockPos::new(1, 3, 6),
    };
    let mut volume = BoxVolume::filled(bounds, BlockStateId::AIR).expect("small");
    let at = SectionPos::new(-1, 0, 0);
    let mut section = volume.section_mut(at).expect("touched");
    section.fill(stone());
    assert_eq!(
        section.set(LocalPos::new(0, 0, 0), ore()),
        BlockStateId::AIR
    );
    assert_eq!(section.set(LocalPos::new(14, 0, 5), ore()), stone());

    let section = volume.section(at).expect("touched");
    let mut inside = 0;
    for local in LocalPos::all() {
        let pos = BlockPos::new(
            -16 + i32::from(local.x()),
            i32::from(local.y()),
            i32::from(local.z()),
        );
        let state = section.get(local);
        if volume.contains(pos) {
            inside += 1;
            let want = if (local.x(), local.y(), local.z()) == (14, 0, 5) {
                ore()
            } else {
                stone()
            };
            assert_eq!(state, want, "{pos}");
        } else {
            assert_eq!(state, BlockStateId::AIR, "{pos}");
        }
    }
    assert_eq!(inside, 2 * 4 * 2);
}
