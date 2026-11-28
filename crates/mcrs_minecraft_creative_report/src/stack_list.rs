use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::ResourceLocation;

use crate::Group;
use crate::GroupSpan;
use crate::ListKind;
use crate::ReportError;

#[derive(Clone, Debug)]
pub struct StackList {
    stacks: Vec<NbtCompound>,
    spans: Vec<GroupSpan>,
}

impl StackList {
    pub(crate) fn locate(
        tab: ResourceLocation<'_>,
        list: ListKind,
        stacks: Vec<NbtCompound>,
    ) -> Result<Self, ReportError> {
        let mut spans: Vec<GroupSpan> = Vec::new();
        for group in Group::of_list(tab, list) {
            let mut members = stacks
                .iter()
                .enumerate()
                .filter(|(_, saved)| group.holds(saved))
                .map(|(index, _)| index);
            let Some(start) = members.next() else {
                continue;
            };
            let mut end = start;
            for index in members {
                if Some(index) != end.checked_add(1) {
                    return Err(ReportError::SplitGroup {
                        tab: tab.to_buf(),
                        list,
                        group,
                    });
                }
                end = index;
            }
            spans.push(GroupSpan {
                group,
                stacks: start..end.saturating_add(1),
            });
        }
        spans.sort_by_key(|span| span.stacks.start);
        Ok(Self { stacks, spans })
    }

    #[must_use]
    pub fn stacks(&self) -> &[NbtCompound] {
        &self.stacks
    }

    #[must_use]
    pub fn spans(&self) -> &[GroupSpan] {
        &self.spans
    }
}
