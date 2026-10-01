use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_light_color_bench::corpus::Corpus;
use mcrs_minecraft_light_color_bench::fixture::{Fixture, fixtures_dir};

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

#[test]
fn every_fixture_state_lights_like_the_corpus() {
    let corpus = Corpus::get();
    let mut paths: Vec<_> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no committed fixture");

    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let fixture = Fixture::read(&path);
        let scene = fixture.scene(&name);

        let mut states: Vec<(String, VoxelId, VoxelId)> = fixture
            .palette
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let id = corpus.resolve(&entry.state);
                assert_eq!(
                    corpus.name(id),
                    entry.state,
                    "{name}: `{}` is not the corpus's own name for it",
                    entry.state
                );
                (entry.state.to_string(), VoxelId(i as u16), id)
            })
            .collect();

        for (state, local, id) in &states {
            assert_eq!(
                scene.registry.emission(*local),
                corpus.registry.emission(*id),
                "{name}: emission of `{state}`"
            );
            let (t, want) = (
                scene.colours.light_type(*local),
                corpus.colours.light_type(*id),
            );
            assert_eq!(t, want, "{name}: light type of `{state}`");
            assert_eq!(
                scene.colours.rgb(t),
                corpus.colours.rgb(want),
                "{name}: colour of `{state}`"
            );
        }

        states.push((
            "the unloaded filler".into(),
            scene.registry.unloaded(),
            corpus.registry.unloaded(),
        ));
        states.push((
            "the outside filler".into(),
            scene.registry.outside(),
            corpus.registry.outside(),
        ));
        for (from, from_local, from_id) in &states {
            for (to, to_local, to_id) in &states {
                for dir in DIRECTIONS {
                    assert_eq!(
                        scene.registry.attenuation(*from_local, *to_local, dir),
                        corpus.registry.attenuation(*from_id, *to_id, dir),
                        "{name}: `{from}` to `{to}` facing {dir:?}"
                    );
                }
            }
        }
    }
}
