use mcrs_minecraft_server::world::chunk::CancellationToken;
use mcrs_minecraft_server::world::generate::generate_column;
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_voxel_math::BlockPos;
use std::collections::BTreeMap;
use std::io::Write;

#[path = "../src/world/generate/tests/support.rs"]
mod support;

use support::{build_settings_router, corpus};

fn main() {
    let prefix = std::env::args().nth(1).expect("output prefix");
    let cancel = CancellationToken::new();
    let y_sections: Vec<i32> = (-4..20).collect();

    let (ox, oz) = std::env::args()
        .nth(2)
        .map(|s| {
            let (a, b) = s.split_once(',').expect("cx,cz");
            (a.parse::<i32>().unwrap(), b.parse::<i32>().unwrap())
        })
        .unwrap_or((0, 0));

    for label in ["overworld", "beta"] {
        let router = build_settings_router(label, 845);
        let mut ids = Vec::new();
        let mut legend = BTreeMap::new();
        for i in 0..64i32 {
            let results =
                generate_column(ox + i % 8, oz + i / 8, &y_sections, &router, None, &cancel);
            for (blocks, _) in results.iter().flatten() {
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let id = blocks.get(BlockPos::new(x, y, z));
                            ids.extend_from_slice(&(id.0 as u32).to_le_bytes());
                            legend
                                .entry(id.0 as u32)
                                .or_insert_with(|| corpus().owner(BlockStateId(id.0)).identifier.to_string());
                        }
                    }
                }
            }
        }
        std::fs::write(format!("{prefix}.{label}.bin"), &ids).unwrap();
        let mut f = std::fs::File::create(format!("{prefix}.{label}.legend")).unwrap();
        for (id, name) in legend {
            writeln!(f, "{id}\t{name}").unwrap();
        }
        eprintln!("{label}: {} positions", ids.len() / 4);
    }
}
