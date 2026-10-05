use std::borrow::Cow;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Vanilla {
    Accepts(&'static str, &'static str),
    Refuses,
}
use Vanilla::{Accepts, Refuses};

struct Row {
    text: &'static str,
    parse: Vanilla,
    from_namespace_and_path: Option<Vanilla>,
}

const ROWS: &[Row] = &[
    Row {
        text: "stone",
        parse: Accepts("minecraft", "stone"),
        from_namespace_and_path: None,
    },
    Row {
        text: "minecraft:stone",
        parse: Accepts("minecraft", "stone"),
        from_namespace_and_path: Some(Accepts("minecraft", "stone")),
    },
    Row {
        text: "a:b/c.d-e_f",
        parse: Accepts("a", "b/c.d-e_f"),
        from_namespace_and_path: Some(Accepts("a", "b/c.d-e_f")),
    },
    Row {
        text: "a:b:c",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "x::y",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "",
        parse: Accepts("minecraft", ""),
        from_namespace_and_path: None,
    },
    Row {
        text: ":",
        parse: Accepts("minecraft", ""),
        from_namespace_and_path: Some(Accepts("", "")),
    },
    Row {
        text: "ns:",
        parse: Accepts("ns", ""),
        from_namespace_and_path: Some(Accepts("ns", "")),
    },
    Row {
        text: ":path",
        parse: Accepts("minecraft", "path"),
        from_namespace_and_path: Some(Accepts("", "path")),
    },
    Row {
        text: "Minecraft:stone",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "minecraft:Stone",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "Stone",
        parse: Refuses,
        from_namespace_and_path: None,
    },
    Row {
        text: "é:x",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "x:é",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "mod.name:path",
        parse: Accepts("mod.name", "path"),
        from_namespace_and_path: Some(Accepts("mod.name", "path")),
    },
    Row {
        text: "a/b:c",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "..:x",
        parse: Refuses,
        from_namespace_and_path: Some(Refuses),
    },
    Row {
        text: "..",
        parse: Accepts("minecraft", ".."),
        from_namespace_and_path: None,
    },
];

fn outcome<S: AsRef<str>, E>(result: Result<ResourceLocation<S>, E>) -> Vanilla {
    match result {
        Ok(location) => Accepts(
            Box::leak(location.namespace().to_owned().into_boxed_str()),
            Box::leak(location.path().to_owned().into_boxed_str()),
        ),
        Err(_) => Refuses,
    }
}

fn wire(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    text.encode(&mut bytes).unwrap();
    bytes
}

#[test]
fn every_identifier_constructor_agrees_with_vanilla() {
    for row in ROWS {
        let text = row.text;
        let expected = row.parse;
        let json = serde_json::to_string(text).unwrap();
        let bytes = wire(text);

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
        assert_eq!(
            outcome(ResourceLocation::<Arc<str>>::decode(&mut bytes.as_slice())),
            expected,
            "wire decode {text:?}"
        );
        assert_eq!(
            outcome(ResourceLocation::<Cow<str>>::decode(&mut bytes.as_slice())),
            expected,
            "wire decode borrowed {text:?}"
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
