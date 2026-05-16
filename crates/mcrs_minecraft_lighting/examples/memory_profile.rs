#[cfg(feature = "profile-memory")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use bevy_state::prelude::NextState;
use mcrs_core::AppState;
use mcrs_engine::world::column::{ColumnIndex, Heightmaps, ColumnChunks};
use mcrs_engine::world::dimension::HasSkyLight;
use mcrs_engine::world::lighting::LightTicket;
use mcrs_minecraft_lighting::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, BlockPendingEgress,
    ChunkNeedsInitialLight, IsAllAir, LightDirty, SkyEgress, SkyIncoming, SkyLight,
    SkyLightSeededAsTopmost, SkyLightWorkspace, SkyPendingEgress,
};
use mcrs_minecraft_lighting::storage::LightStorage;
use mcrs_minecraft_lighting::test_bench::bench_helpers::{
    build_warmed_vd12_app_in_place, install_lighting_plugins,
};
use serde::Serialize;
use smallvec::SmallVec;
use std::mem;

const MEMORY_BUDGET_BYTES: usize = 40 * 1024 * 1024;
// Identifier for this profile run; bench-helpers palettes are themselves deterministic.
const FIXTURE_SEED: u64 = 0x6d6372735f6c69;
const JSON_OUT_PATH: &str = "crates/mcrs_minecraft_lighting/benches/results/memory-profile.json";
const HTML_OUT_DIR: &str = "target/memory-profile";
const HTML_OUT_PATH: &str = "target/memory-profile/report.html";

#[derive(Serialize)]
struct CategoryBytes {
    name: String,
    bytes: usize,
}

#[derive(Serialize)]
struct MemorySnapshot {
    schema_version: String,
    git_commit_sha: String,
    fixture_seed: u64,
    timestamp_iso: String,
    categories: Vec<CategoryBytes>,
    total_bytes: usize,
    total_mb: f64,
    budget_mb: u32,
    exceeded_budget: bool,
    dhat_total_bytes: Option<usize>,
    dhat_in_process_delta_pct: Option<f64>,
    telemetry_iterations: u64,
}

fn main() {
    #[cfg(feature = "profile-memory")]
    let _profiler = dhat::Profiler::new_heap();

    let _ = FIXTURE_SEED;

    let mut app = bevy_app::App::new();
    let _stub_dim = install_lighting_plugins(&mut app);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);

    build_warmed_vd12_app_in_place(&mut app);

    // One extra tick to ensure downgrade_light_storage has fired (runs in EmitDirty
    // stage which executes after convergence; without this tick the Mixed→Uniform/Null
    // collapse has not yet run at the snapshot point)
    app.update();

    let snap = walk_ecs(&mut app);

    write_stdout_markdown(&snap);

    if let Err(e) = write_json(&snap, JSON_OUT_PATH) {
        eprintln!("memory_profile: failed to write JSON: {e}");
    }

    if let Err(e) = write_html(&snap, HTML_OUT_PATH) {
        eprintln!("memory_profile: failed to write HTML: {e}");
    }

    let code = if snap.exceeded_budget { 1 } else { 0 };
    std::process::exit(code);
}

fn light_storage_bytes(storage: &LightStorage) -> usize {
    mem::size_of_val(storage)
        + match storage {
            LightStorage::Mixed(_) => 2048,
            _ => 0,
        }
}

fn smallvec_bytes<T>(sv: &SmallVec<[T; 8]>) -> usize {
    mem::size_of_val(sv)
        + if sv.spilled() {
            sv.capacity() * mem::size_of::<T>()
        } else {
            0
        }
}

fn walk_ecs(app: &mut bevy_app::App) -> MemorySnapshot {
    let world = app.world_mut();

    // "light_nibbles": per-section BlockLight + SkyLight storage
    let mut light_nibbles: usize = 0;
    for block_light in world.query::<&BlockLight>().iter(world) {
        light_nibbles += light_storage_bytes(&block_light.0);
    }
    for sky_light in world.query::<&SkyLight>().iter(world) {
        light_nibbles += light_storage_bytes(&sky_light.0);
    }

    // "wavefront_buffers": all six per-section egress/incoming SmallVec buffers
    let mut wavefront_buffers: usize = 0;
    for c in world.query::<&BlockEgress>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }
    for c in world.query::<&SkyEgress>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }
    for c in world.query::<&BlockIncoming>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }
    for c in world.query::<&SkyIncoming>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }
    for c in world.query::<&BlockPendingEgress>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }
    for c in world.query::<&SkyPendingEgress>().iter(world) {
        wavefront_buffers += smallvec_bytes(&c.0);
    }

    // "workspaces": BlockLightWorkspace and SkyLightWorkspace BFS queues
    let mut workspaces: usize = 0;
    for ws in world.query::<&BlockLightWorkspace>().iter(world) {
        workspaces += mem::size_of_val(ws)
            + ws.increase_queue.capacity() * 8
            + ws.decrease_queue.capacity() * 8;
    }
    for ws in world.query::<&SkyLightWorkspace>().iter(world) {
        workspaces += mem::size_of_val(ws)
            + ws.increase_queue.capacity() * 8
            + ws.decrease_queue.capacity() * 8;
    }

    // "heightmaps": per-column Heightmaps (two PackedBitStorage backing Vec<u64>)
    let mut heightmaps: usize = 0;
    for hm in world.query::<&Heightmaps>().iter(world) {
        heightmaps += mem::size_of_val(hm)
            + hm.world_surface.raw_longs().len() * 8
            + hm.motion_blocking.raw_longs().len() * 8;
    }

    // "section_indexes": per-column ColumnChunks (Box<[Option<Entity>]>)
    let mut section_indexes: usize = 0;
    for idx in world.query::<&ColumnChunks>().iter(world) {
        section_indexes +=
            mem::size_of_val(idx) + idx.sections.len() * mem::size_of::<Option<bevy_ecs::prelude::Entity>>();
    }

    // "column_indexes": per-dimension ColumnIndex (FxHashMap)
    let mut column_indexes: usize = 0;
    for idx in world.query::<&ColumnIndex>().iter(world) {
        use mcrs_engine::world::column::{ColumnPos, ColumnSlot};
        column_indexes += mem::size_of_val(idx)
            + idx.len() * (mem::size_of::<ColumnPos>() + mem::size_of::<ColumnSlot>());
    }

    // "sparse_markers": six sparse-set marker components (8 bytes per entry approximation)
    let dirty_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<LightDirty>>()
        .iter(world)
        .count();
    let ticket_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<LightTicket>>()
        .iter(world)
        .count();
    let needs_initial_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<ChunkNeedsInitialLight>>()
        .iter(world)
        .count();
    let all_air_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<IsAllAir>>()
        .iter(world)
        .count();
    let sky_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<HasSkyLight>>()
        .iter(world)
        .count();
    let topmost_count = world
        .query_filtered::<(), bevy_ecs::prelude::With<SkyLightSeededAsTopmost>>()
        .iter(world)
        .count();
    let sparse_markers = (dirty_count
        + ticket_count
        + needs_initial_count
        + all_air_count
        + sky_count
        + topmost_count)
        * 8;

    let categories = vec![
        CategoryBytes { name: "light_nibbles".into(), bytes: light_nibbles },
        CategoryBytes { name: "wavefront_buffers".into(), bytes: wavefront_buffers },
        CategoryBytes { name: "workspaces".into(), bytes: workspaces },
        CategoryBytes { name: "heightmaps".into(), bytes: heightmaps },
        CategoryBytes { name: "section_indexes".into(), bytes: section_indexes },
        CategoryBytes { name: "column_indexes".into(), bytes: column_indexes },
        CategoryBytes { name: "sparse_markers".into(), bytes: sparse_markers },
    ];

    let total_bytes: usize = categories.iter().map(|c| c.bytes).sum();
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let exceeded_budget = total_bytes > MEMORY_BUDGET_BYTES;

    let git_commit_sha = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".into());

    let timestamp_iso = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (y, mo, d, h, mi, s) = unix_secs_to_ymd_hms(secs);
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    };

    let telemetry_iterations = mcrs_minecraft_lighting::telemetry::snapshot().iterations;

    #[cfg(feature = "profile-memory")]
    let (dhat_total_bytes, dhat_in_process_delta_pct) = {
        let stats = dhat::HeapStats::get();
        let dhat_total = stats.curr_bytes;
        let delta_pct = if total_bytes > 0 {
            Some((dhat_total as f64 - total_bytes as f64) / total_bytes as f64 * 100.0)
        } else {
            None
        };
        (Some(dhat_total), delta_pct)
    };

    #[cfg(not(feature = "profile-memory"))]
    let (dhat_total_bytes, dhat_in_process_delta_pct) = (None, None);

    MemorySnapshot {
        schema_version: "1.0".into(),
        git_commit_sha,
        fixture_seed: FIXTURE_SEED,
        timestamp_iso,
        categories,
        total_bytes,
        total_mb,
        budget_mb: 40,
        exceeded_budget,
        dhat_total_bytes,
        dhat_in_process_delta_pct,
        telemetry_iterations,
    }
}

fn unix_secs_to_ymd_hms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Gregorian calendar derivation from days since epoch (1970-01-01)
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y as u32, mo as u32, d as u32, h as u32, m as u32, s as u32)
}

fn write_stdout_markdown(snap: &MemorySnapshot) {
    println!("| Category | Bytes | MB | % of total |");
    println!("|----------|-------|----|------------|");
    for cat in &snap.categories {
        let mb = cat.bytes as f64 / (1024.0 * 1024.0);
        let pct = if snap.total_bytes > 0 {
            cat.bytes as f64 / snap.total_bytes as f64 * 100.0
        } else {
            0.0
        };
        println!("| {} | {} | {:.3} | {:.1}% |", cat.name, cat.bytes, mb, pct);
    }
    println!("| **Total** | {} | {:.3} | 100% |", snap.total_bytes, snap.total_mb);
    println!();
    println!("Budget: {} MB", snap.budget_mb);
    if snap.exceeded_budget {
        let overspend = snap.total_mb - snap.budget_mb as f64;
        println!("Status: FAIL — overspend by {:.3} MB", overspend);
    } else {
        println!("Status: PASS");
    }
}

fn write_json(snap: &MemorySnapshot, path: &str) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(snap)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

fn write_html(snap: &MemorySnapshot, path: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(HTML_OUT_DIR)?;

    let rows = snap.categories.iter().map(|cat| {
        let mb = cat.bytes as f64 / (1024.0 * 1024.0);
        let pct = if snap.total_bytes > 0 {
            (cat.bytes as f64 / snap.total_bytes as f64 * 100.0) as u32
        } else {
            0
        };
        format!(
            "<tr><td>{}</td><td>{}</td><td>{:.3}</td><td>\
             <div style=\"width:{}%;background:#4a90d9;height:12px\"></div>{pct}%\
             </td></tr>",
            cat.name, cat.bytes, mb, pct
        )
    }).collect::<Vec<_>>().join("\n");

    let status_class = if snap.exceeded_budget { "fail" } else { "pass" };
    let status_text = if snap.exceeded_budget {
        format!("FAIL — overspend by {:.3} MB", snap.total_mb - snap.budget_mb as f64)
    } else {
        "PASS".into()
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Memory Profile</title>
<style>
  body{{font-family:sans-serif;margin:2em}}
  table{{border-collapse:collapse;width:100%}}
  th,td{{text-align:left;padding:6px 12px;border-bottom:1px solid #ddd}}
  th{{background:#f4f4f4}}
  .pass{{color:green;font-weight:bold}}
  .fail{{color:red;font-weight:bold}}
</style>
</head>
<body>
<h1>Memory Profile Report</h1>
<p>Commit: <code>{sha}</code> | Generated: {ts}</p>
<table>
<tr><th>Category</th><th>Bytes</th><th>MB</th><th>% of total</th></tr>
{rows}
<tr><td><strong>Total</strong></td><td>{tb}</td><td>{tmb:.3}</td><td>100%</td></tr>
</table>
<p>Budget: {budget} MB</p>
<p class="{sc}">Status: {st}</p>
</body>
</html>"#,
        sha = snap.git_commit_sha,
        ts = snap.timestamp_iso,
        rows = rows,
        tb = snap.total_bytes,
        tmb = snap.total_mb,
        budget = snap.budget_mb,
        sc = status_class,
        st = status_text,
    );

    std::fs::write(path, html)
}
