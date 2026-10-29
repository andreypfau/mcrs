use mcrs_command::Span;
use mcrs_command::SyntaxError;
use mcrs_minecraft_command::{KeywordError, error_feedback};
use mcrs_minecraft_command_coordinates::CoordinatesError;
use mcrs_minecraft_command_vanilla::ArgumentError;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};

#[test]
fn an_error_shows_the_ten_characters_before_it() {
    let input = "gamemode creativ";
    let error = SyntaxError::argument(
        ArgumentError::Keyword(KeywordError::GameModeInvalid(Span::new(9, 16))),
        16,
    );
    let [message, _] = error_feedback(input, &error);
    assert_eq!(
        message,
        Text::translate("argument.gamemode.invalid", vec!["creativ".into()]).color(Color::RED)
    );

    let cases = [
        ("gamemode creativ", 16, "...de creativ", None),
        ("gamemode creativ", 9, "gamemode ", Some("creativ")),
        ("0123456789", 10, "0123456789", None),
        ("a\u{1f600}bcdefghij", 14, "...\u{1f600}bcdefghij", None),
        (
            "a\u{1f600}bcdefghij x",
            14,
            "...\u{1f600}bcdefghij",
            Some(" x"),
        ),
    ];
    for (input, position, before, rest) in cases {
        let error = SyntaxError::argument(
            ArgumentError::Coordinates(CoordinatesError::PosMixed),
            position,
        );
        let [_, context] = error_feedback(input, &error);
        let mut expected = before.into_text();
        if let Some(rest) = rest {
            expected = expected.add_child(rest.color(Color::RED).underlined());
        }
        let expected = expected
            .add_child(
                Text::translate("command.context.here", vec![])
                    .color(Color::RED)
                    .italic(),
            )
            .color(Color::GRAY)
            .on_click_suggest_command(format!("/{input}"));
        assert_eq!(context, expected, "{input} at {position}");
    }
}
