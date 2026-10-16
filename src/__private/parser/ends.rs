//! Where the matches of an expression can end. The functions of a rule that
//! tries every way through it work with them.

use rustc_hash::FxHashSet;

/// Where the matches of an expression at a position can end, each once, in the
/// order a parser prefers them.
#[derive(Debug, Clone)]
pub struct Ends(Held);

/// How [`Ends`] holds its ends. Most expressions have few, so those are held
/// without allocating.
#[derive(Debug, Clone)]
enum Held {
    Few {
        count: usize,
        ends: [usize; FEW_ENDS],
    },
    Many {
        ends: Vec<usize>,
        /// The same ends once there are more than [`LISTED_ENDS`], to tell
        /// quickly whether one is there.
        seen: Option<FxHashSet<usize>>,
    },
}

/// How many ends [`Ends`] holds without allocating.
const FEW_ENDS: usize = 4;

/// How many ends [`Ends`] looks through one by one.
const LISTED_ENDS: usize = 16;

impl Default for Ends {
    fn default() -> Self {
        Self(Held::Few {
            count: 0,
            ends: [0; FEW_ENDS],
        })
    }
}

impl Ends {
    /// Returns no ends, as of an expression that does not match.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn one(end: usize) -> Self {
        let mut ends = Self::none();
        ends.add(end);
        ends
    }

    /// Returns `end` alone, if there is one.
    pub fn of(end: Option<usize>) -> Self {
        end.map_or_else(Self::none, Self::one)
    }

    pub(super) fn as_slice(&self) -> &[usize] {
        match &self.0 {
            Held::Few { count, ends } => &ends[..*count],
            Held::Many { ends, .. } => ends,
        }
    }

    #[must_use]
    pub fn contains(&self, end: usize) -> bool {
        match &self.0 {
            Held::Many {
                seen: Some(seen), ..
            } => seen.contains(&end),
            _ => self.as_slice().contains(&end),
        }
    }

    /// Returns the end a parser prefers, if any.
    #[must_use]
    pub fn first(&self) -> Option<usize> {
        self.as_slice().first().copied()
    }

    /// Returns the ends in order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.as_slice().iter().copied()
    }

    /// Adds `end` after the others, unless it is there already.
    pub fn add(&mut self, end: usize) {
        match &mut self.0 {
            Held::Few { count, ends } if ends[..*count].contains(&end) => {}
            Held::Few { count, ends } if *count < FEW_ENDS => {
                ends[*count] = end;
                *count += 1;
            }
            Held::Few { ends, .. } => {
                let mut many = Vec::with_capacity(FEW_ENDS * 2);
                many.extend_from_slice(ends);
                many.push(end);
                self.0 = Held::Many {
                    ends: many,
                    seen: None,
                };
            }
            Held::Many { ends, seen } => {
                let there = match seen {
                    Some(seen) => seen.contains(&end),
                    None => ends.contains(&end),
                };
                if there {
                    return;
                }
                ends.push(end);
                match seen {
                    Some(seen) => {
                        seen.insert(end);
                    }
                    None if ends.len() > LISTED_ENDS => {
                        *seen = Some(ends.iter().copied().collect());
                    }
                    None => {}
                }
            }
        }
    }

    /// Adds each of `other` after these, in its order.
    pub fn extend(&mut self, other: Self) {
        if self.as_slice().is_empty() {
            *self = other;
            return;
        }
        for &end in other.as_slice() {
            self.add(end);
        }
    }

    /// Keeps only the ends `keep` holds for.
    pub fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        let kept = Self::none();
        let kept = self.iter().filter(|&end| keep(end)).fold(
            kept,
            |mut kept, end| {
                kept.add(end);
                kept
            },
        );
        *self = kept;
    }

    /// Returns the ends with the end of the most text first.
    #[must_use]
    pub fn longest_first(mut self) -> Self {
        let ends = match &mut self.0 {
            Held::Few { count, ends } => &mut ends[..*count],
            Held::Many { ends, .. } => ends,
        };
        ends.sort_unstable_by(|left, right| right.cmp(left));
        self
    }

    /// Returns the only end of an expression that has at most one, and that
    /// the rest of a match goes on from.
    #[must_use]
    pub fn only(&self) -> usize {
        let ends = self.as_slice();
        debug_assert_eq!(ends.len(), 1, "the expression has one end");
        ends[0]
    }
}
