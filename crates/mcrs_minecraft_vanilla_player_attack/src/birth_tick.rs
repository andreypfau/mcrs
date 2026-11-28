use mcrs_minecraft_engine_world::birth_clock::BirthClock;

pub(crate) fn birth_tick(clock: Option<&BirthClock>) -> u64 {
    clock.map_or(0, |clock| clock.0)
}
