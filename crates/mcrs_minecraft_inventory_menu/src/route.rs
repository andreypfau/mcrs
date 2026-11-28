use crate::MenuRange;
use crate::RouteWhen;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Route {
    pub from: MenuRange,
    pub when: RouteWhen,
    pub to: MenuRange,
}
