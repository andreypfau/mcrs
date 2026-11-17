use std::ops::Range;

use bytes::Bytes;

use mcrs_minecraft_entity_key::EntityId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileBlob {
    pub bytes: Bytes,
    pub ranges: Vec<SubjectRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectRange {
    pub subject: EntityId,
    pub range: Range<usize>,
}

impl TileBlob {
    pub(crate) fn append_kept(&self, chunks: &mut Vec<Bytes>, keep: impl Fn(EntityId) -> bool) {
        let len = self.bytes.len();
        let mut checked = 0;
        let mut run: Option<Range<usize>> = None;
        for SubjectRange { subject, range } in &self.ranges {
            let in_bounds = checked <= range.start && range.start <= range.end && range.end <= len;
            if !in_bounds {
                continue;
            }
            checked = range.end;
            if !keep(*subject) {
                flush(chunks, &self.bytes, run.take());
                continue;
            }
            match &mut run {
                Some(open) if open.end == range.start => open.end = range.end,
                _ => {
                    flush(chunks, &self.bytes, run.take());
                    run = Some(range.clone());
                }
            }
        }
        flush(chunks, &self.bytes, run);
    }
}

fn flush(chunks: &mut Vec<Bytes>, bytes: &Bytes, run: Option<Range<usize>>) {
    if let Some(range) = run.filter(|range| !range.is_empty()) {
        chunks.push(bytes.slice(range));
    }
}
