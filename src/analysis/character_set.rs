//! Character sets are what the analysis compares, kept as ranges so that a
//! special sequence of a million characters stays small.

use std::{fmt, ops::RangeInclusive};

/// A set of characters, as sorted ranges that neither overlap nor touch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CharacterSet {
    ranges: Vec<RangeInclusive<char>>,
}

impl CharacterSet {
    /// Returns the set of every character in `range`. The range must not be
    /// empty.
    pub(crate) fn from_range(range: RangeInclusive<char>) -> Self {
        Self {
            ranges: vec![range],
        }
    }

    pub(crate) fn single(c: char) -> Self {
        Self::from_range(c..=c)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub(crate) fn union(&self, other: &Self) -> Self {
        let mut all: Vec<RangeInclusive<char>> =
            self.ranges.iter().chain(&other.ranges).cloned().collect();
        all.sort_by_key(|range| *range.start());
        let mut ranges: Vec<RangeInclusive<char>> = Vec::new();
        for range in all {
            if let Some(last) = ranges.last_mut() {
                let touches =
                    u32::from(*range.start()) <= u32::from(*last.end()) + 1;
                if touches {
                    *last = *last.start()..=*last.end().max(range.end());
                    continue;
                }
            }
            ranges.push(range);
        }
        Self { ranges }
    }

    pub(crate) fn intersection(&self, other: &Self) -> Self {
        let mut ranges = Vec::new();
        let (mut left, mut right) = (0, 0);
        while let (Some(a), Some(b)) =
            (self.ranges.get(left), other.ranges.get(right))
        {
            let start = *a.start().max(b.start());
            let end = *a.end().min(b.end());
            if start <= end {
                ranges.push(start..=end);
            }
            if a.end() < b.end() {
                left += 1;
            } else {
                right += 1;
            }
        }
        Self { ranges }
    }

    /// Returns whether every character of this set is in `other`.
    pub(crate) fn is_subset(&self, other: &Self) -> bool {
        self.intersection(other) == *self
    }
}

impl fmt::Display for CharacterSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, range) in self.ranges.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            if range.start() == range.end() {
                write!(f, "{:?}", range.start())?;
            } else {
                write!(f, "{:?}..={:?}", range.start(), range.end())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ranges: &[RangeInclusive<char>]) -> CharacterSet {
        ranges.iter().fold(CharacterSet::default(), |set, range| {
            set.union(&CharacterSet::from_range(range.clone()))
        })
    }

    #[test]
    fn merges_ranges_that_overlap_or_touch() {
        assert_eq!(set(&['a'..='c', 'b'..='e']), set(&['a'..='e']));
        assert_eq!(set(&['a'..='c', 'd'..='e']), set(&['a'..='e']));
        assert_eq!(set(&['d'..='e', 'a'..='c']), set(&['a'..='e']));
        assert_eq!(set(&['a'..='z', 'c'..='d']), set(&['a'..='z']));
    }

    #[test]
    fn keeps_ranges_apart_when_they_do_not_touch() {
        assert_eq!(
            set(&['a'..='b', 'd'..='e']).to_string(),
            "'a'..='b', 'd'..='e'"
        );
    }

    #[test]
    fn intersects_sets() {
        let left = set(&['a'..='f', 'm'..='p']);
        let right = set(&['d'..='n']);
        assert_eq!(left.intersection(&right), set(&['d'..='f', 'm'..='n']));
        assert!(left.intersection(&set(&['x'..='z'])).is_empty());
    }

    #[test]
    fn compares_subsets() {
        assert!(set(&['b'..='c']).is_subset(&set(&['a'..='z'])));
        assert!(set(&['a'..='z']).is_subset(&set(&['a'..='z'])));
        assert!(!set(&['a'..='z']).is_subset(&set(&['b'..='c'])));
        assert!(CharacterSet::default().is_subset(&set(&['a'..='a'])));
    }

    #[test]
    fn handles_the_whole_range_of_characters() {
        let all = set(&['\0'..=char::MAX]);
        assert_eq!(all.union(&CharacterSet::single('a')), all);
        assert!(CharacterSet::single(char::MAX).is_subset(&all));
    }

    #[test]
    fn displays_single_characters_and_ranges() {
        assert_eq!(CharacterSet::single('x').to_string(), "'x'");
        assert_eq!(CharacterSet::default().to_string(), "");
    }
}
