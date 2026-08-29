use bevy::prelude::*;

use super::DebugEntryGroup;

/// The lines one frame's debug entries produced, in the shape
/// `DebugScreenOverlay` splits into its two columns.
#[derive(Resource, Default)]
pub struct DebugScreenDisplayer {
    left: Vec<String>,
    right: Vec<String>,
    groups: Vec<(DebugEntryGroup, Vec<String>)>,
}

impl DebugScreenDisplayer {
    pub fn add_priority_line(&mut self, line: String) {
        if self.left.len() > self.right.len() {
            self.right.push(line);
        } else {
            self.left.push(line);
        }
    }

    pub fn add_to_group(
        &mut self,
        group: DebugEntryGroup,
        lines: impl IntoIterator<Item = String>,
    ) {
        match self.groups.iter_mut().find(|(id, _)| *id == group) {
            Some((_, existing)) => existing.extend(lines),
            None => self.groups.push((group, lines.into_iter().collect())),
        }
    }

    pub(super) fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
        self.groups.clear();
    }

    /// The left and right columns, blank strings included: each one separates a
    /// block from the next and renders as an empty row.
    pub fn columns(&self) -> (Vec<String>, Vec<String>) {
        let mut left = self.left.clone();
        let mut right = self.right.clone();

        if !left.is_empty() {
            left.push(String::new());
        }
        if !right.is_empty() {
            right.push(String::new());
        }

        let middle = self.groups.len().div_ceil(2);
        for (index, (_, lines)) in self.groups.iter().enumerate() {
            if lines.is_empty() {
                continue;
            }
            let column = if index < middle { &mut left } else { &mut right };
            column.extend_from_slice(lines);
            column.push(String::new());
        }

        (left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::resource_location::ResourceLocation;

    const FIRST: DebugEntryGroup = ResourceLocation::new_static("minecraft:first");
    const SECOND: DebugEntryGroup = ResourceLocation::new_static("minecraft:second");

    fn lines(of: &[&str]) -> Vec<String> {
        of.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn priority_lines_alternate_between_the_columns() {
        let mut displayer = DebugScreenDisplayer::default();
        for line in ["a", "b", "c"] {
            displayer.add_priority_line(line.to_owned());
        }
        let (left, right) = displayer.columns();
        assert_eq!(left, lines(&["a", "c", ""]));
        assert_eq!(right, lines(&["b", ""]));
    }

    #[test]
    fn the_first_half_of_the_groups_goes_left_and_the_rest_right() {
        let mut displayer = DebugScreenDisplayer::default();
        displayer.add_to_group(FIRST, lines(&["one"]));
        displayer.add_to_group(SECOND, lines(&["two"]));
        displayer.add_to_group(FIRST, lines(&["one again"]));
        let (left, right) = displayer.columns();
        assert_eq!(left, lines(&["one", "one again", ""]));
        assert_eq!(right, lines(&["two", ""]));
    }
}
