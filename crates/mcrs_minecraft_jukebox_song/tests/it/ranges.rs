use mcrs_minecraft_jukebox_song::JukeboxSong;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::ItemHover;

fn read(length_in_seconds: &str, comparator_output: &str) -> serde_json::Result<()> {
    let entry = format!(
        r#"{{"sound_event":"minecraft:music_disc.13","description":"x","length_in_seconds":{length_in_seconds},"comparator_output":{comparator_output}}}"#
    );
    serde_json::from_str::<JukeboxSong<ItemHover, Named>>(&entry).map(|_| ())
}

#[test]
fn a_value_out_of_its_range_is_an_error_of_the_entry() {
    assert!(read("178.0", "15").is_ok(), "the base entry is valid");

    let refused: [(&str, &str); 4] = [
        ("0.0", "1"),
        ("3.5e38", "1"),
        ("178.0", "16"),
        ("178.0", "-1"),
    ];
    for (length_in_seconds, comparator_output) in refused {
        assert!(
            read(length_in_seconds, comparator_output).is_err(),
            "{length_in_seconds} {comparator_output}"
        );
    }
}
