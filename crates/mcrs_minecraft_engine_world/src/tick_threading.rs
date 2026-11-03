use bevy_app::{App, SubApp, SubApps};
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy_ecs::world::World;
use rayon::ThreadPool;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// How the game logic of a tick uses threads. World generation, lighting and IO keep their own
/// pools in either mode.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickThreading {
    /// The dimensions tick one after another on the server thread and each schedule runs one
    /// system at a time. `par_iter` runs on the compute pool's single thread when the app owns
    /// the task pools, and across the host's compute pool when it does not.
    Single,
    /// The dimensions tick at the same time, and a schedule runs in parallel the systems whose
    /// access does not conflict.
    Parallel,
}

impl Default for TickThreading {
    /// [`TickThreading::Parallel`] with the `parallel-tick` feature, [`TickThreading::Single`]
    /// without it.
    fn default() -> Self {
        if cfg!(feature = "parallel-tick") {
            Self::Parallel
        } else {
            Self::Single
        }
    }
}

impl FromStr for TickThreading {
    type Err = ParseTickThreadingError;

    /// Accepts `single` and `parallel` in any ASCII case, with surrounding whitespace.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("single") {
            Ok(Self::Single)
        } else if trimmed.eq_ignore_ascii_case("parallel") {
            Ok(Self::Parallel)
        } else {
            Err(ParseTickThreadingError(value.to_owned()))
        }
    }
}

/// A [`TickThreading`] name that is neither `single` nor `parallel`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseTickThreadingError(pub String);

impl fmt::Display for ParseTickThreadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is neither single nor parallel", self.0)
    }
}

impl Error for ParseTickThreadingError {}

/// The threads that the dimensions tick on in [`TickThreading::Parallel`].
///
/// A dimension's executor blocks its thread in a scope on the compute pool. Were the dimensions
/// tasks of that same pool, as many dimensions as compute threads would block every thread.
#[derive(Resource)]
pub(crate) struct DimTickPool(pub(crate) ThreadPool);

/// [`App::update`], except that every sub-app extracts first and the sub-apps then update at the
/// same time when the app has a dimension tick pool.
pub fn update(app: &mut App) {
    let SubApps { main, sub_apps } = app.sub_apps_mut();
    main.run_default_schedule();
    for sub_app in sub_apps.values_mut() {
        sub_app.extract(main.world_mut());
    }
    run_each(
        main.world().get_resource::<DimTickPool>(),
        sub_apps.len(),
        sub_apps.values_mut(),
        SubApp::update,
    );
    main.world_mut().clear_trackers();
}

/// Gives every schedule of `world` a single-threaded executor.
pub fn run_schedules_on_one_thread(world: &mut World) {
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return;
    };
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

fn run_each<'a>(
    pool: Option<&DimTickPool>,
    count: usize,
    sub_apps: impl Iterator<Item = &'a mut SubApp> + Send,
    run: fn(&mut SubApp),
) {
    match pool {
        // A lone dimension gains nothing from the pool but the handoff to it and back.
        Some(pool) if count > 1 => pool.0.install(|| join_each(sub_apps, run)),
        _ => sub_apps.for_each(run),
    }
}

// `rayon::join` keeps both of its jobs on the stack, so fanning the sub-apps out allocates nothing.
fn join_each<'a>(mut sub_apps: impl Iterator<Item = &'a mut SubApp> + Send, run: fn(&mut SubApp)) {
    let Some(first) = sub_apps.next() else {
        return;
    };
    rayon::join(|| run(first), || join_each(sub_apps, run));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::AppLabel;
    use bevy_ecs::schedule::ScheduleLabel;
    use rayon::ThreadPoolBuilder;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const DIMENSIONS: usize = 3;

    #[derive(AppLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Dimension(usize);

    #[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct Tick;

    #[derive(Resource, Clone, Copy)]
    struct HostTick(u32);

    #[derive(Resource, Default)]
    struct Seen {
        host_tick: Option<u32>,
        met_every_dimension: bool,
    }

    static ARRIVED: AtomicUsize = AtomicUsize::new(0);

    // Exclusive, so it runs on the dimension's own thread rather than on the compute pool.
    fn meet_the_other_dimensions(world: &mut World) {
        ARRIVED.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(2);
        while ARRIVED.load(Ordering::SeqCst) < DIMENSIONS && Instant::now() < deadline {
            thread::yield_now();
        }
        let host_tick = world.get_resource::<HostTick>().map(|tick| tick.0);
        let mut seen = world.resource_mut::<Seen>();
        seen.host_tick = host_tick;
        seen.met_every_dimension = ARRIVED.load(Ordering::SeqCst) >= DIMENSIONS;
    }

    #[test]
    fn the_dimensions_of_a_parallel_tick_run_at_the_same_time() {
        ARRIVED.store(0, Ordering::SeqCst);
        let mut app = App::new();
        app.insert_resource(HostTick(7));
        let pool = ThreadPoolBuilder::new()
            .num_threads(DIMENSIONS)
            .build()
            .unwrap();
        app.insert_resource(DimTickPool(pool));
        for index in 0..DIMENSIONS {
            let mut sub_app = SubApp::new();
            sub_app.init_resource::<Seen>();
            sub_app.update_schedule = Some(Tick.intern());
            sub_app.add_systems(Tick, meet_the_other_dimensions);
            sub_app.set_extract(|host, dimension| {
                if let Some(tick) = host.get_resource::<HostTick>() {
                    dimension.insert_resource(*tick);
                }
            });
            app.insert_sub_app(Dimension(index), sub_app);
        }

        update(&mut app);

        for index in 0..DIMENSIONS {
            let seen = app.sub_app(Dimension(index)).world().resource::<Seen>();
            assert_eq!(
                seen.host_tick,
                Some(7),
                "dimension {index} extracts before it updates"
            );
            assert!(
                seen.met_every_dimension,
                "dimension {index} waits for the others, which only arrive if they run at the same time"
            );
        }
    }
}
