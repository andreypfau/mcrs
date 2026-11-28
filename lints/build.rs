use std::env;
use std::process::Command;

fn main() {
    #[expect(
        clippy::disallowed_methods,
        reason = "cargo names the compiler of the build in `RUSTC`"
    )]
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let Ok(output) = Command::new(rustc).args(["--print", "sysroot"]).output() else {
        return;
    };
    let sysroot = String::from_utf8_lossy(&output.stdout);
    println!(
        "cargo:rustc-link-arg-bins=-Wl,-rpath,{}/lib",
        sysroot.trim()
    );
}
