/// Whether a bone pattern of `materials` or `part_visibility` matches `bone`. `*` matches any
/// sequence of characters; letter case does not count.
#[must_use]
pub fn matches_bone(pattern: &str, bone: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let bone = bone.to_ascii_lowercase();
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return bone.is_empty();
    };
    let Some(mut rest) = bone.strip_prefix(first) else {
        return false;
    };
    let tail: Vec<&str> = parts.collect();
    let Some((last, middle)) = tail.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => match rest.get(at + part.len()..) {
                Some(after) => rest = after,
                None => return false,
            },
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_star_matches_any_run_and_case_does_not_count() {
        assert!(matches_bone("*", "body"));
        assert!(matches_bone("Croaking_Body", "croaking_body"));
        assert!(matches_bone("leg*", "leg_left"));
        assert!(matches_bone("*_leg", "left_leg"));
        assert!(matches_bone("l*g", "leg"));
        assert!(!matches_bone("l*g*x", "leg"));
        assert!(!matches_bone("body", "bodyx"));
    }
}
