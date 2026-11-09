//! A slot buffer that grows in a frame keeps the records that the same frame writes.
//!
//! ```sh
//! cargo test -p mcrs_minecraft_entity_render --test slot_grow -- --ignored
//! ```
#![expect(
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

#[expect(dead_code, reason = "the test uses a part of the test support")]
mod support;

use mcrs_minecraft_entity_render::{
    Capacities, EntityLibrary, RecordRange, SlotUpload, StateRecord,
};

fn upload(slot_capacity: u32, flags: u32) -> SlotUpload {
    SlotUpload {
        slot_capacity,
        slot_count: 1,
        ticks: Vec::new(),
        culls: Vec::new(),
        states: vec![RecordRange {
            first: 0,
            records: vec![StateRecord {
                flags,
                ..StateRecord::default()
            }],
        }],
        inputs: Vec::new(),
    }
}

#[test]
#[ignore = "needs a GPU"]
fn a_record_written_in_the_frame_of_a_grow_survives_the_copy_of_the_old_buffer() {
    let Some(gpu) = support::Gpu::new() else {
        eprintln!("skipped: no adapter, the slot grow test needs a GPU");
        return;
    };
    let pack = support::pack();
    let library = EntityLibrary::build(&pack.sources(), &[support::white_texture()], 2048);
    let capacities = Capacities::for_cap(&library, 1024);
    let mut entity_gpu = gpu.entity_gpu(&library, capacities, 64);

    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload(64, 1));
    gpu.queue.submit([encoder.finish()]);

    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload(4096, 2));
    gpu.queue.submit([encoder.finish()]);
    assert!(entity_gpu.slot_capacity() >= 4096);

    let bytes = gpu.read(entity_gpu.slot_states());
    let first: &[StateRecord] = bytemuck::cast_slice(&bytes[..size_of::<StateRecord>()]);
    assert_eq!(first[0].flags, 2);
}
