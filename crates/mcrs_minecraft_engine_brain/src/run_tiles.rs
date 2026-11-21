use bevy_tasks::TaskPool;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_goal_world::GoalWorld;

pub(crate) fn run_tiles<B, W>(
    pool: &TaskPool,
    work: &mut [TileWork<'_, B>],
    cx: &TickContext<'_, W>,
) where
    B: MobBrain<W> + Send,
    W: GoalWorld + Sync,
{
    let per_task = tiles_per_task(pool, work.len());
    pool.scope(|scope| {
        for chunk in work.chunks_mut(per_task) {
            scope.spawn(async move {
                for task in chunk {
                    task.run(cx);
                }
            });
        }
    });
}
