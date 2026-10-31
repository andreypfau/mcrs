use std::borrow::Cow;

use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf};

use crate::identifier_rows::{Accepts, ROWS, Refuses, outcome};

#[test]
fn every_identifier_constructor_agrees_with_vanilla() {
    for row in ROWS {
        let text = row.text;
        let expected = row.parse;
        let json = serde_json::to_string(text).unwrap();

        assert_eq!(
            outcome(text.parse::<ResourceLocationBuf>()),
            expected,
            "FromStr {text:?}"
        );
        assert_eq!(
            outcome(ResourceLocationBuf::read(text)),
            expected,
            "read {text:?}"
        );
        assert_eq!(
            outcome(serde_json::from_str::<ResourceLocationBuf>(&json)),
            expected,
            "Deserialize {text:?}"
        );
        assert_eq!(
            outcome(serde_json::from_str::<ResourceLocation<Cow<'static, str>>>(
                &json
            )),
            expected,
            "Deserialize Cow {text:?}"
        );

        if let (Some(expected), Some((namespace, path))) =
            (row.from_namespace_and_path, text.split_once(':'))
        {
            assert_eq!(
                outcome(ResourceLocationBuf::new(namespace, path)),
                expected,
                "new {namespace:?} {path:?}"
            );
        }

        let literal = match expected {
            Accepts(namespace, path)
                if !namespace.is_empty() && format!("{namespace}:{path}") == text =>
            {
                expected
            }
            _ => Refuses,
        };
        assert_eq!(
            outcome(ResourceLocation::new_static(text)),
            literal,
            "new_static {text:?}"
        );
    }
}
