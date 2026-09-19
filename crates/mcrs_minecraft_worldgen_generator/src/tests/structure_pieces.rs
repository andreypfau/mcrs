use std::collections::{BTreeSet, HashMap};
use std::io::Cursor;
use std::path::PathBuf;

use bevy_math::IVec3;
use bytes::Buf;
use mcrs_minecraft_core::{BoundingBox, ColumnPos, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::{Nbt, to_nbt_compound};
use mcrs_minecraft_worldgen_structure::piece::PieceContext;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};

use super::structure_sites::{build_index, dimension};
use super::structures::frozen_shared;
use super::template_parity::canonical;
use crate::structures::index::StructureIndex;

const MAGIC: &[u8; 8] = b"MCSTRPC0";

/// Every structure type whose layout is not ported: the oracle has starts for
/// them and this build produces none. The end city has no present case at the
/// dump's shared chunks and joins the list with its own cases.
const UNPORTED_PIECE_TYPES: [&str; 6] = [
    "minecraft:igloo",
    "minecraft:mineshaft",
    "minecraft:nether_fossil",
    "minecraft:stronghold",
    "minecraft:swamp_hut",
    "minecraft:woodland_mansion",
];

/// Fields the reference fills in at placement from the live world, which the
/// layout fixes here from the density heights instead: they are dropped from
/// both sides before a piece is compared, keyed by the piece's `id`.
const PLACEMENT_FIXED_FIELDS: [(&str, &[&str]); 5] = [
    ("minecraft:tedp", &["HPos"]),
    ("minecraft:tejp", &["HPos"]),
    ("minecraft:tesh", &["HPos"]),
    ("minecraft:shipwreck", &["TPY", "height_adjusted"]),
    ("minecraft:orp", &["TPY"]),
];

pub(super) struct DumpCase {
    pub seed: i64,
    pub dimension: String,
    pub structure: String,
    pub chunk: ColumnPos,
    pub start: Option<(BoundingBox, Vec<NbtCompound>)>,
}

pub(super) fn read_nbt(r: &mut bytes::Bytes, label: &str) -> NbtCompound {
    let len = r.get_u32_le() as usize;
    let bytes = r.copy_to_bytes(len);
    Nbt::read(&mut NbtReadHelper::new(Cursor::new(&bytes[..])))
        .unwrap_or_else(|e| panic!("{label}: {e}"))
        .root_tag
}

pub(super) fn read_box(r: &mut impl Buf) -> BoundingBox {
    let min = IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
    let max = IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
    BoundingBox {
        min: min.into(),
        max: max.into(),
    }
}

fn read_dump() -> Vec<DumpCase> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/structure_pieces.bin");
    let mut r = open_dump(&path, MAGIC);
    let cases = (0..r.get_u32_le())
        .map(|_| {
            let seed = r.get_i64_le();
            let dimension = dump_string(&mut r);
            let structure = dump_string(&mut r);
            let chunk = ColumnPos::new(r.get_i32_le(), r.get_i32_le());
            let start = (r.get_u8() == 1).then(|| {
                let bounds = read_box(&mut r);
                let pieces = (0..r.get_u32_le())
                    .map(|i| read_nbt(&mut r, &format!("{structure} piece #{i}")))
                    .collect();
                (bounds, pieces)
            });
            DumpCase {
                seed,
                dimension,
                structure,
                chunk,
                start,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    cases
}

pub(super) fn comparable(piece: &NbtCompound) -> NbtCompound {
    let id = piece.get_string("id").unwrap_or_default();
    let dropped = PLACEMENT_FIXED_FIELDS
        .iter()
        .find(|(piece_id, _)| *piece_id == id)
        .map_or(&[][..], |(_, fields)| fields);
    let mut piece = piece.clone();
    piece
        .child_tags
        .retain(|(key, _)| !dropped.contains(&key.as_str()));
    canonical(&piece)
}

#[test]
fn structure_pieces_serialise_as_the_reference_writes_them() {
    let dump = read_dump();
    let frozen = frozen_shared();
    let mut indices: HashMap<(i64, String), StructureIndex> = HashMap::new();
    let mut unported = BTreeSet::new();
    let mut compared = 0;
    let mut pieces = 0;
    for case in &dump {
        let id = frozen.structure_ids[&ResourceLocation::parse(&case.structure).unwrap()];
        let structure = &frozen.structures[id.0 as usize];
        let label = format!(
            "seed {} {} {} at {:?}",
            case.seed, case.dimension, case.structure, case.chunk
        );
        // The end has no multi-noise source to index; only the end city places there.
        if case.dimension == "minecraft:the_end" {
            assert!(case.start.is_none() || unported.insert(structure.kind.type_name()));
            continue;
        }
        let index = indices
            .entry((case.seed, case.dimension.clone()))
            .or_insert_with(|| build_index(&dimension(&case.dimension), case.seed));
        let ours = index.start_of(case.chunk, id);
        let Some((bounds, expected)) = &case.start else {
            assert!(
                ours.is_none(),
                "{label}: a start the reference does not have"
            );
            continue;
        };
        let Some(start) = ours else {
            unported.insert(structure.kind.type_name());
            continue;
        };
        assert_eq!(start.bounds, *bounds, "{label}: start box");
        assert_eq!(start.pieces.len(), expected.len(), "{label}: piece count");
        let context = PieceContext {
            frozen,
            structure: id,
        };
        for (i, (piece, want)) in start.pieces.iter().zip(expected).enumerate() {
            let got = to_nbt_compound(&piece.nbt(&context)).unwrap();
            assert_eq!(
                comparable(&got),
                comparable(want),
                "{label} piece #{i}: NBT"
            );
            pieces += 1;
        }
        compared += 1;
    }
    assert_eq!(unported, UNPORTED_PIECE_TYPES.into_iter().collect());
    assert_eq!((compared, pieces), (396, 10874));
}
