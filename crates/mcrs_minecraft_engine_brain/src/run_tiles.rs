use bevy_tasks::TaskPool;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_goal_world::GoalWorld;

pub(crate) fn run_tiles<B, W>(
    pool: &TaskPool,
    work: &mut [TileWork<'_, B>],
    cx: &TickContext<'_, W>,
) where
    B: MobBrain<W> + Send,
    W: GoalWorld + Sync,
{
    let per_task = work
        .len()
        .div_ceil(pool.thread_num().saturating_mul(4).max(1))
        .max(1);
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
