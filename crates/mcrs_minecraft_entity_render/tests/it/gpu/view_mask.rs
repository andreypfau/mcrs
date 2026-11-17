use crate::support;
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, CullRecord, EntityFrameCounts, EntityLibrary, EntityViews,
    FrameUniform, SlotArena, StateRecord, TickRecord, f16_up,
};

const TICK: u32 = 1000;

#[test]
fn a_view_draws_only_the_slots_whose_view_set_holds_its_bit() {
    let Some(gpu) = support::Gpu::new() else {
        eprintln!("skipped: no adapter, the view mask test needs a GPU");
        return;
    };
    let pack = support::pack();
    let library = EntityLibrary::build(&pack.sources(), &[support::white_texture()], 2048);
    let (definition, _) = pack.entity("minecraft:pig").unwrap();
    let model = library
        .definitions
        .get(definition)
        .and_then(|tables| tables.models.iter().find_map(|model| *model))
        .unwrap();
    let mut arena = SlotArena::new();
    let mut place = |views: EntityViews| {
        let slot = arena.allocate();
        arena.write_state(
            slot,
            StateRecord {
                definition: definition as u16,
                layer_mask: 1,
                layers: [[model, 0], [0; 2], [0; 2], [0; 2]],
                hurt_start: -1e9,
                scale: f16_up(1.0),
                section: u32::MAX,
                clock_offset: TICK as i32,
                input: u32::MAX,
                ..StateRecord::default()
            },
        );
        arena.write_tick(
            slot,
            TICK,
            TickRecord {
                block: [0, 64, 0],
                ..TickRecord::default()
            },
            true,
        );
        arena.write_cull(
            slot,
            CullRecord {
                block: [0, 64, 0],
                radius: f16_up(2.0),
                flags: CULL_VALID | views.cull_flags(),
            },
        );
        slot
    };
    let world = place(EntityViews::WORLD);
    let figure = place(EntityViews::FIGURE);

    let capacities = Capacities::for_cap(&library, 64);
    let mut entity_gpu = gpu.entity_gpu(&library, capacities, arena.capacity());
    let upload = arena.take_upload();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
    gpu.queue.submit([encoder.finish()]);

    for (view, drawn) in [(EntityViews::WORLD, world), (EntityViews::FIGURE, figure)] {
        let frame = FrameUniform {
            frustum: [[0.0, 0.0, 0.0, 1.0]; 5],
            tick: TICK,
            viewport: [854.0, 480.0],
            view: view.frame_bit(),
            ..FrameUniform::default()
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        entity_gpu.write_frame(&gpu.queue, &frame, upload.slot_count);
        entity_gpu.encode_compute(&mut encoder, None);
        gpu.queue.submit([encoder.finish()]);

        let counts = EntityFrameCounts::of(bytemuck::cast_slice(&gpu.read(entity_gpu.counters())));
        assert_eq!(
            counts.full_rigs, 1,
            "{view:?}: one of the two slots is visible"
        );
        let visible: Vec<[u32; 2]> = bytemuck::cast_slice(&gpu.read(entity_gpu.visible())).to_vec();
        let first = definition * capacities.full_rigs as usize;
        assert_eq!(
            visible.get(first).map(|entry| entry.first()),
            Some(Some(&drawn)),
            "{view:?}: the visible slot is the one of the view"
        );
    }
}
