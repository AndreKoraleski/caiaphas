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
        debug_assert!(!range.is_empty(), "{range:?} is empty");
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
        let mut ranges: Vec<RangeInclusive<char>> = Vec::new();
        let mut left = self.ranges.iter().peekable();
        let mut right = other.ranges.iter().peekable();
        // Both sets are sorted, so taking the range that starts first from
        // either keeps the result sorted.
        while let Some(range) = match (left.peek(), right.peek()) {
            (Some(a), Some(b)) if b.start() < a.start() => right.next(),
            (Some(_), _) => left.next(),
            (None, _) => right.next(),
        } {
            let range = range.clone();
            if let Some(last) = ranges.last_mut() {
                let touches = next(*last.end())
                    .is_none_or(|after| *range.start() <= after);
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
    ///
    /// The ranges of `other` never touch, so each range of this set has to fit
    /// inside one of them.
    pub(crate) fn is_subset(&self, other: &Self) -> bool {
        let mut position = 0;
        self.ranges.iter().all(|range| {
            while other
                .ranges
                .get(position)
                .is_some_and(|other| other.end() < range.start())
            {
                position += 1;
            }
            other.ranges.get(position).is_some_and(|other| {
                other.start() <= range.start() && range.end() <= other.end()
            })
        })
    }

    /// Returns the characters of this set that are not in `other`.
    pub(crate) fn difference(&self, other: &Self) -> Self {
        let mut ranges = Vec::new();
        for range in &self.ranges {
            let mut start = Some(*range.start());
            let cuts = other
                .ranges
                .iter()
                .take_while(|cut| cut.start() <= range.end());
            for cut in cuts {
                let Some(from) = start else {
                    break;
                };
                if cut.end() < &from {
                    continue;
                }
                if let Some(before) =
                    previous(*cut.start()).filter(|before| *before >= from)
                {
                    ranges.push(from..=before);
                }
                start = next(*cut.end()).filter(|after| after <= range.end());
            }
            if let Some(from) = start {
                ranges.push(from..=*range.end());
            }
        }
        Self { ranges }
    }
}

/// Returns the character right before `c`, skipping the surrogates.
fn previous(c: char) -> Option<char> {
    match c {
        '\0' => None,
        '\u{E000}' => Some('\u{D7FF}'),
        _ => char::from_u32(u32::from(c) - 1),
    }
}

/// Returns the character right after `c`, skipping the surrogates.
fn next(c: char) -> Option<char> {
    if c == '\u{D7FF}' {
        Some('\u{E000}')
    } else {
        char::from_u32(u32::from(c) + 1)
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
        assert_eq!(
            set(&['a'..='c', 'b'..='e']),
            CharacterSet::from_range('a'..='e')
        );
        assert_eq!(
            set(&['a'..='c', 'd'..='e']),
            CharacterSet::from_range('a'..='e')
        );
        assert_eq!(
            set(&['d'..='e', 'a'..='c']),
            CharacterSet::from_range('a'..='e')
        );
        assert_eq!(
            set(&['a'..='z', 'c'..='d']),
            CharacterSet::from_range('a'..='z')
        );
    }

    #[test]
    fn merges_ranges_that_touch_across_the_surrogates() {
        let all = CharacterSet::from_range('\0'..=char::MAX);
        let split = set(&['\0'..='\u{D7FF}', '\u{E000}'..=char::MAX]);
        assert_eq!(split, all);
        assert!(all.is_subset(&split));
        assert_eq!(
            set(&['\0'..='\u{D7FE}', '\u{E000}'..=char::MAX]).to_string(),
            "'\\0'..='\\u{d7fe}', '\\u{e000}'..='\\u{10ffff}'"
        );
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
        let right = CharacterSet::from_range('d'..='n');
        assert_eq!(left.intersection(&right), set(&['d'..='f', 'm'..='n']));
        assert!(
            left.intersection(&CharacterSet::from_range('x'..='z'))
                .is_empty()
        );
    }

    #[test]
    fn compares_subsets() {
        let letters = CharacterSet::from_range('a'..='z');
        let some = CharacterSet::from_range('b'..='c');
        assert!(some.is_subset(&letters));
        assert!(letters.is_subset(&letters));
        assert!(!letters.is_subset(&some));
        assert!(CharacterSet::default().is_subset(&CharacterSet::single('a')));
    }

    #[test]
    fn subtracts_sets() {
        let letters = CharacterSet::from_range('a'..='z');
        assert_eq!(
            letters.difference(&CharacterSet::single('x')),
            set(&['a'..='w', 'y'..='z'])
        );
        assert_eq!(
            letters.difference(&CharacterSet::from_range('a'..='c')),
            CharacterSet::from_range('d'..='z')
        );
        assert_eq!(
            letters.difference(&CharacterSet::from_range('x'..='z')),
            CharacterSet::from_range('a'..='w')
        );
        assert_eq!(
            letters.difference(&set(&['b'..='c', 'm'..='n', 'y'..='y'])),
            set(&['a'..='a', 'd'..='l', 'o'..='x', 'z'..='z'])
        );
        assert_eq!(
            letters.difference(&CharacterSet::from_range('0'..='9')),
            letters
        );
        assert!(
            letters
                .difference(&CharacterSet::from_range('\0'..=char::MAX))
                .is_empty()
        );
    }

    #[test]
    fn subtracts_across_ranges_and_the_surrogates() {
        let two = set(&['a'..='c', 'x'..='z']);
        assert_eq!(
            two.difference(&CharacterSet::from_range('b'..='y')),
            set(&['a'..='a', 'z'..='z'])
        );
        let around = CharacterSet::from_range('\u{D000}'..='\u{F000}');
        assert_eq!(
            around.difference(&CharacterSet::single('\u{E000}')),
            set(&['\u{D000}'..='\u{D7FF}', '\u{E001}'..='\u{F000}'])
        );
        assert_eq!(
            around.difference(&CharacterSet::single('\u{D7FF}')),
            set(&['\u{D000}'..='\u{D7FE}', '\u{E000}'..='\u{F000}'])
        );
        let all = CharacterSet::from_range('\0'..=char::MAX);
        assert_eq!(
            all.difference(&set(&['\0'..='\0', char::MAX..=char::MAX])),
            CharacterSet::from_range('\u{1}'..='\u{10FFFE}')
        );
    }

    #[test]
    fn handles_the_whole_range_of_characters() {
        let all = CharacterSet::from_range('\0'..=char::MAX);
        assert_eq!(all.union(&CharacterSet::single('a')), all);
        assert!(CharacterSet::single(char::MAX).is_subset(&all));
    }

    #[test]
    fn displays_single_characters_and_ranges() {
        assert_eq!(CharacterSet::single('x').to_string(), "'x'");
        assert_eq!(CharacterSet::default().to_string(), "");
    }
}
