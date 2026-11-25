use mcrs_minecraft_lang::Language;

pub trait TextHost {
    fn translation<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        let _ = key;
        None
    }

    fn keybind_label<'a>(&'a self, name: &'a str) -> Option<&'a str> {
        let _ = name;
        None
    }

    fn translates_keybind_name(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoHost;

impl TextHost for NoHost {}

impl TextHost for Language {
    fn translation<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        self.find(key)
    }
}
