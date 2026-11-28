use mcrs_minecraft_client_jar::Directory;

use mcrs_minecraft_font::inflated_file::InflatedFile;

pub fn inflate_zip(archive: &[u8]) -> Result<Vec<InflatedFile>, String> {
    let directory = Directory::of(archive)?;
    directory
        .inflated(archive, |_| true)
        .map(|file| {
            let (name, bytes) = file?;
            Ok(InflatedFile {
                name: name.to_owned(),
                bytes,
            })
        })
        .collect()
}
