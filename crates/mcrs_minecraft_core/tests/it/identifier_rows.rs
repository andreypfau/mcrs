use mcrs_minecraft_core::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vanilla {
    Accepts(&'static str, &'static str),
    Refuses,
}
pub use Vanilla::{Accepts, Refuses};

pub struct Row {
    pub text: &'static str,
    pub parse: Vanilla,
    pub from_namespace_and_path: Option<Vanilla>,
}

pub const ROWS: &[Row] = &[
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

pub fn outcome<S: AsRef<str>, E>(result: Result<ResourceLocation<S>, E>) -> Vanilla {
    match result {
        Ok(location) => Accepts(
            Box::leak(location.namespace().to_owned().into_boxed_str()),
            Box::leak(location.path().to_owned().into_boxed_str()),
        ),
        Err(_) => Refuses,
    }
}
