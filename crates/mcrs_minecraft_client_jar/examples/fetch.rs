use mcrs_minecraft_client_jar::{Progress, resolve};

fn main() {
    let (_, remainder) = resolve(&Progress::default(), |_| false, drop);
    if let Some(remainder) = remainder {
        remainder.finish();
    }
}
