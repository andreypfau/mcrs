use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    TabList,
    SearchList,
}

impl fmt::Display for ListKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TabList => "tab_list",
            Self::SearchList => "search_list",
        })
    }
}
