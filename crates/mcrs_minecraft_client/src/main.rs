#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(target_family = "wasm")]
fn main() {
    mcrs_minecraft_client::web::run();
}

#[cfg(not(target_family = "wasm"))]
fn main() -> bevy::app::AppExit {
    native::run()
}
