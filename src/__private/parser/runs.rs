//! Runs of characters from a set end at every place inside them, and a parser
//! reads each run once to tell those places.

use std::collections::BTreeMap;

use super::{Ends, Parser, Width};
use crate::__private::{Expected, Rule};

impl<'i, const REPORTS: bool, W: Width> Parser<'i, REPORTS, W> {
    /// Returns what [`span_ends`](Self::span_ends) does, less every end from
    /// the first place where a match of `middle` inside the run ends. This is
    /// what an exception that takes away any run holding such a match leaves,
    /// such as `{c} - ({c}, '*)', {c})`. The run is read once, up to that
    /// place, and `middle` is tried at each place before it, in a parser that
    /// notes nothing.
    ///
    /// A parse that notes why it fails notes what reading the whole run and
    /// taking those ends away for `rule` from `position` would. It reads each
    /// run once for each `site`, a number of the place in the grammar.
    #[allow(clippy::too_many_arguments)]
    pub fn span_ends_before(
        &mut self,
        rule: &Rule,
        site: usize,
        position: usize,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
        mut middle: impl FnMut(&mut Parser<'i, false, W>, usize) -> Ends,
    ) -> Ends {
        let input = self.input;
        let run_end = REPORTS.then(|| {
            // A run ends at the same place from anywhere inside it.
            match self.runs.get(&site) {
                Some(&(start, end)) if (start..=end).contains(&position) => {
                    self.fail(end, expected);
                    end
                }
                _ => {
                    let end = self.span(position, &contains, expected);
                    self.runs.insert(site, (position, end));
                    end
                }
            }
        });
        let mut within = self.within(input.len());
        // The first place a match of `middle` inside the run ends, and where
        // the run ends when no such match ends before.
        let mut first = usize::MAX;
        let mut end = None;
        let mut at = position;
        while at < first {
            for found in middle(&mut within, at).iter() {
                if found < first && input[at..found].chars().all(&contains) {
                    first = found;
                }
            }
            match input[at..].chars().next() {
                Some(c) if contains(c) => at += c.len_utf8(),
                _ => {
                    end = Some(at);
                    break;
                }
            }
        }
        // Taking every end from `first` on away notes what taking the furthest
        // away does, since that comes first and the rest add nothing new.
        if let Some(run_end) = run_end.filter(|&run_end| first <= run_end) {
            self.excluded(rule, position, run_end);
        }
        let mut ends = Ends::none();
        let last = match end {
            Some(end) if first == usize::MAX => {
                ends.add(end);
                end
            }
            _ => first,
        };
        for (offset, _) in input[position..last].char_indices().rev() {
            ends.add(position + offset);
        }
        ends
    }

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
