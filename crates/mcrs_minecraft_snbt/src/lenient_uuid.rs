use alloc::vec::Vec;

/// Five dash-separated hex groups, each group masked to its field width, at
/// most 36 characters in total. Returns the most and least significant halves.
pub fn parse_lenient_uuid(text: &str) -> Option<(i64, i64)> {
    if text.len() > 36 {
        return None;
    }
    let groups: Vec<&str> = text.split('-').collect();
    let [a, b, c, d, e] = groups.as_slice() else {
        return None;
    };
    let hex = |group: &str| i64::from_str_radix(group, 16).ok();
    let most = ((hex(a)? & 0xffff_ffff) << 32) | ((hex(b)? & 0xffff) << 16) | (hex(c)? & 0xffff);
    let least = ((hex(d)? & 0xffff) << 48) | (hex(e)? & 0xffff_ffff_ffff);
    Some((most, least))
}
