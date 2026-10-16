//! Spelling finds the name that a misspelled one was meant to be.

/// Returns the one of `names` closest to `name` when few enough edits make one
/// into the other, ignoring case, as with a misspelling.
pub(crate) fn closest<'n>(
    name: &str,
    names: impl IntoIterator<Item = &'n str>,
) -> Option<&'n str> {
    // At least one character has to stay, so `a` is not close to `b`.
    let length = name.chars().count();
    let most = (length.max(3) / 3).min(length.saturating_sub(1));
    let name = name.to_lowercase();
    names
        .into_iter()
        .map(|other| (edits(&name, &other.to_lowercase()), other))
        .filter(|&(distance, _)| distance <= most)
        .min_by_key(|&(distance, _)| distance)
        .map(|(_, other)| other)
}

/// Returns how many characters have to be added, removed, replaced or swapped
/// with the next to make `from` into `to`.
fn edits(from: &str, to: &str) -> usize {
    let from: Vec<char> = from.chars().collect();
    let to: Vec<char> = to.chars().collect();
    // Each row holds the edits from a start of `from` to every start of `to`.
    let mut before: Vec<usize> = Vec::new();
    let mut row: Vec<usize> = (0..=to.len()).collect();
    for index in 1..=from.len() {
        let mut next = vec![index; to.len() + 1];
        for column in 1..=to.len() {
            let same = from[index - 1] == to[column - 1];
            next[column] = (row[column - 1] + usize::from(!same))
                .min(row[column] + 1)
                .min(next[column - 1] + 1);
            if index > 1
                && column > 1
                && from[index - 1] == to[column - 2]
                && from[index - 2] == to[column - 1]
            {
                next[column] = next[column].min(before[column - 2] + 1);
            }
        }
        before = std::mem::replace(&mut row, next);
    }
    row[to.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_edits_between_two_names() {
        assert_eq!(edits("name", "name"), 0);
        assert_eq!(edits("nmae", "name"), 1);
        assert_eq!(edits("kitten", "sitting"), 3);
        assert_eq!(edits("tied-branch", "tied-branches"), 2);
        assert_eq!(edits("", "abc"), 3);
    }

    #[test]
    fn finds_the_closest_name_within_a_few_edits() {
        let names = ["name", "number", "tied-branches"];
        assert_eq!(closest("nmae", names), Some("name"));
        assert_eq!(closest("Number", names), Some("number"));
        assert_eq!(closest("tied-branch", names), Some("tied-branches"));
        assert_eq!(closest("value", names), None);
        assert_eq!(closest("a", ["b"]), None);
    }
}
