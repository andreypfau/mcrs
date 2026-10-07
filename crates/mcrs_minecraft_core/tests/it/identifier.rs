use std::borrow::Cow;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use crate::identifier_rows::{Accepts, ROWS, Refuses, outcome};

#[test]
fn every_identifier_constructor_agrees_with_vanilla() {
    for row in ROWS {
        let text = row.text;
        let expected = row.parse;
        let json = serde_json::to_string(text).unwrap();

        assert_eq!(
            outcome(text.parse::<ResourceLocation>()),
            expected,
            "FromStr {text:?}"
        );
        assert_eq!(
            outcome(ResourceLocation::read(text)),
            expected,
            "read {text:?}"
        );
        assert_eq!(
            outcome(serde_json::from_str::<ResourceLocation<Arc<str>>>(&json)),
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
                outcome(ResourceLocation::new(namespace, path)),
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
        let built =
            std::panic::catch_unwind(|| outcome::<_, ()>(Ok(ResourceLocation::new_static(text))))
                .unwrap_or(Refuses);
        assert_eq!(built, literal, "new_static {text:?}");
    }
}
