#[cfg(feature = "profile-memory")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "profile-memory")]
    let _profiler = dhat::Profiler::new_heap();

    eprintln!("memory_profile: placeholder — real implementation in a later plan");
    std::process::exit(0);
}
