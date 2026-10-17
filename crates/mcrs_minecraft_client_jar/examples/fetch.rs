use mcrs_minecraft_client_jar::{ClientJar, Progress, vanilla_data};

fn main() {
    match vanilla_data(
        &ClientJar::InstalledOrDownloaded,
        None,
        &Progress::default(),
    ) {
        Ok(dir) => println!("{}", dir.display()),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
