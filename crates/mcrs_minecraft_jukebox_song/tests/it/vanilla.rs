use mcrs_minecraft_fixtures::assert_vanilla_round_trip;
use mcrs_minecraft_jukebox_song::JukeboxSong;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::ItemHover;

#[test]
fn every_vanilla_jukebox_song_writes_back_as_its_file() {
    assert_vanilla_round_trip::<JukeboxSong<ItemHover, Named>>("jukebox_song");
}
