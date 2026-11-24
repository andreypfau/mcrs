use mcrs_minecraft_fixtures::assert_vanilla_round_trip;
use mcrs_minecraft_instrument::Instrument;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::ItemHover;

#[test]
fn every_vanilla_instrument_writes_back_as_its_file() {
    assert_vanilla_round_trip::<Instrument<ItemHover, Named>>("instrument");
}
