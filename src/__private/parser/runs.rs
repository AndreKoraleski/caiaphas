//! Runs of characters from a set end at every place inside them, and a parser
//! reads each run once to tell those places.

use std::collections::BTreeMap;

use super::{Ends, Parser, Width};
use crate::__private::Expected;

impl<const REPORTS: bool, W: Width> Parser<'_, REPORTS, W> {
    /// Returns every place where a run of characters from `position` that
    /// `contains` holds for can end, the furthest first, and notes each of
    /// `expected` at the furthest.
    pub fn span_ends(
        &mut self,
        position: usize,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> Ends {
        let end = self.span(position, contains, expected);
        let mut ends = Ends::one(end);
        for (offset, _) in self.input[position..end].char_indices().rev() {
            ends.add(position + offset);
        }
        ends
    }

    /// Returns what [`then`](Self::then) does with a step of
    /// [`span_ends`](Self::span_ends), in the same order, but reads each run
    /// of characters once. Every start in a run already read ends where that
    /// run does, and what it adds is there already.
    pub fn then_span(
        &mut self,
        ends: &Ends,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> Ends {
        if let &[end] = ends.as_slice() {
            return self.span_ends(end, &contains, expected);
        }
        let mut after = Ends::none();
        // Each run read, by where its reading started, with where it ends. No
        // two of them overlap.
        let mut runs: BTreeMap<usize, usize> = BTreeMap::new();
        for &start in ends.as_slice() {
            if runs
                .range(..=start)
                .next_back()
                .is_some_and(|(_, &end)| start <= end)
            {
                continue;
            }
            // Read up to the end of the run, or to a run read already.
            let mut stop = start;
            let mut known = None;
            for (offset, c) in self.input[start..].char_indices() {
                stop = start + offset;
                if let Some(&end) = runs.get(&stop).filter(|_| offset > 0) {
                    known = Some(end);
                    break;
                }
                if !contains(c) {
                    break;
                }
                stop = start + offset + c.len_utf8();
            }
            let end = match known {
                Some(end) => end,
                None => self.span(stop, &contains, expected),
            };
            after.add(end);
            for (offset, _) in self.input[start..stop].char_indices().rev() {
                after.add(start + offset);
            }
            let inside: Vec<usize> =
                runs.range(start..=end).map(|(&start, _)| start).collect();
            for start in inside {
                runs.remove(&start);
            }
            runs.insert(start, end);
        }
        after
    }
}
