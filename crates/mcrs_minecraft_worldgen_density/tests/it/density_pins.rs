use crate::vanilla_parity::overworld_router;
use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::FINAL_DENSITY;

mod exhaustive {
    use super::*;

    #[test]
    fn the_branch_schedule_skips_a_real_share_of_an_overworld_chunk() {
        let router = overworld_router(42);
        let volume = SampleGrid::dense(IVec3::new(16, 384, 16), IVec3::new(0, -64, 0));
        let mut out = vec![f32::NAN; volume.len()];
        let mut ws = Workspace::new();
        ws.take_count();
        router
            .program()
            .fill(&mut ws, &volume, FINAL_DENSITY, &mut out);

        let count = ws.take_count();
        let total = count.evaluated + count.skipped;
        let share = count.skipped as f64 / total as f64;
        println!(
            "final_density over 16x384x16: {} of {total} node evaluations skipped ({:.1}%)",
            count.skipped,
            share * 100.0
        );
        assert!(
            share > 0.15,
            "the schedule skipped only {:.1}% of {total} evaluations",
            share * 100.0
        );
    }
}
