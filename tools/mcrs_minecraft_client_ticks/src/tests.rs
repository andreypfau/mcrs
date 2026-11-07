use super::*;

const TEXT: &str = "client-ticks 1
version 1.0
experiment E9
step walk_stone
instruction Hold W for 2 seconds.
entity 7
fill -4 99 -4 12 99 4 minecraft:oak_slab[type=bottom,waterlogged=false]
events
< abilities may_fly,instant_build 0.05 0.1
< attribute minecraft:movement_speed 0.1 minecraft:sprinting 0.30000001192092896 2
< teleport 3 0.5 100 0.5 0 0 0 -90.5 0 0
> accept_teleport 3 0.5 100 0.5 -90.5 0
> tick
> input forward,sprint
> command start_sprinting
> move 0.5,99.9216,0.5 - on_ground
> tick
< motion 0 -0.0784000015258789 0
> move - 12.25,-3.5 none
> tick
> input none
";

#[test]
fn a_recording_reads_back_as_written() {
    let recording: Recording = TEXT.parse().unwrap();
    assert_eq!(recording.header.fills[0].min, [-4, 99, -4]);
    assert_eq!(
        recording.header.fills[0].state,
        "minecraft:oak_slab[type=bottom,waterlogged=false]"
    );
    assert_eq!(recording.header.instruction, "Hold W for 2 seconds.");
    let ticks: Vec<&[Event]> = recording.ticks().collect();
    assert_eq!(
        ticks.len(),
        3,
        "the input after the last tick end is in no tick"
    );
    assert_eq!(
        ticks[1][2],
        Event::Move(Move {
            position: Some([0.5, 99.9216, 0.5]),
            rotation: None,
            on_ground: true,
            horizontal_collision: false,
        })
    );
    assert_eq!(ticks[2][0], Event::Motion([0.0, -0.0784000015258789, 0.0]));
    let Event::Attribute(attribute) = &ticks[0][1] else {
        panic!("the second event is the attribute");
    };
    assert_eq!(attribute.modifiers[0].amount, f64::from(0.3_f32));
    assert_eq!(recording.to_string(), TEXT);
}

#[test]
fn a_bad_line_names_its_number() {
    let text = TEXT.replace("> command start_sprinting", "> command start_flying");
    let error = text.parse::<Recording>().unwrap_err();
    assert_eq!(error.line, 15);
}
