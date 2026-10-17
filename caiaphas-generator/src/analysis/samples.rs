//! Samples are short texts an expression may match, found by a bounded search,
//! so that what two alternatives share can be told when neither matches only a
//! few fixed texts.
//!
//! Characters that no terminal string holds and no special sequence starts or
//! ends at behave alike, so the search tries one character of each such kind.
//! It ignores exceptions and greedy rules, so a sample is only a candidate,
//! and the matcher tells whether it matches.

use std::collections::{HashMap, HashSet};

use crate::ir::{Expression, ExpressionKind, Grammar, RuleId};

/// How many bytes a sample can hold.
const LENGTH: usize = 8;

/// How many samples an expression keeps, the shortest first.
const COUNT: usize = 64;

/// How many rules deep a sample can go.
const DEPTH: usize = 6;

/// Finds the samples of the expressions of one grammar.
pub(crate) struct Sampler<'a> {
    grammar: &'a Grammar,
    /// The first character of each kind, in order.
    kinds: Vec<char>,
    /// The samples of each rule, by its position and the depth left.
    rules: HashMap<(usize, usize), Vec<String>>,
}

impl<'a> Sampler<'a> {
    pub(crate) fn new(grammar: &'a Grammar) -> Self {
        let mut kinds = vec!['\0'];
        for rule in &grammar.syntax_rules {
            rule.expression
                .visit(&mut |expression| match &expression.kind {
                    ExpressionKind::TerminalString(text) => {
                        for c in text.chars() {
                            kinds.extend([c].into_iter().chain(after(c)));
                        }
                    }
                    ExpressionKind::SpecialSequence(range) => {
                        kinds.push(*range.start());
                        kinds.extend(after(*range.end()));
                    }
                    _ => {}
                });
        }
        kinds.sort_unstable();
        kinds.dedup();
        Self {
            grammar,
            kinds,
            rules: HashMap::new(),
        }
    }

    pub(crate) fn grammar(&self) -> &'a Grammar {
        self.grammar
    }

    /// Returns short texts `expression` may match, the shortest first.
    pub(crate) fn samples(&mut self, expression: &Expression) -> Vec<String> {
        self.within(expression, DEPTH)
    }

    fn within(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Vec<String> {
        match &expression.kind {
            ExpressionKind::TerminalString(text) => {
                bounded(vec![text.clone()])
            }
            ExpressionKind::SpecialSequence(range) => {
                let first = self.kinds.partition_point(|c| c < range.start());
                self.kinds[first..]
                    .iter()
                    .take_while(|c| range.contains(c))
                    .take(COUNT)
                    .map(char::to_string)
                    .collect()
            }
            ExpressionKind::Not(_) | ExpressionKind::EmptySequence => {
                vec![String::new()]
            }
            ExpressionKind::MetaIdentifier(RuleId(position)) => {
                let Some(depth) = depth.checked_sub(1) else {
                    return Vec::new();
                };
                if let Some(samples) = self.rules.get(&(*position, depth)) {
                    return samples.clone();
                }
                let rule = &self.grammar.syntax_rules[*position];
                let samples = self.within(&rule.expression, depth);
                self.rules.insert((*position, depth), samples.clone());
                samples
            }
            ExpressionKind::DefinitionsList(alternatives) => {
                let mut samples = Vec::new();
                for alternative in alternatives {
                    samples.extend(self.within(alternative, depth));
                }
                bounded(samples)
            }
            ExpressionKind::SingleDefinition(expressions) => {
                let mut samples = vec![String::new()];
                for expression in expressions {
                    let next = self.within(expression, depth);
                    samples = concatenated(&samples, &next);
                }
                samples
            }
            ExpressionKind::OptionalSequence(expression) => {
                let mut samples = self.within(expression, depth);
                samples.push(String::new());
                bounded(samples)
            }
            ExpressionKind::RepeatedSequence(expression) => {
                let once = self.within(expression, depth);
                let mut samples = vec![String::new()];
                // Only the samples the last round found can make new ones.
                let mut newest = samples.clone();
                loop {
                    let found: Vec<String> = concatenated(&newest, &once)
                        .into_iter()
                        .filter(|sample| !samples.contains(sample))
                        .collect();
                    if found.is_empty() {
                        return samples;
                    }
                    samples.extend(found.iter().cloned());
                    samples = bounded(samples);
                    newest = found
                        .into_iter()
                        .filter(|sample| samples.contains(sample))
                        .collect();
                }
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let once = self.within(syntactic_primary, depth);
                let mut samples = vec![String::new()];
                for _ in 0..*integer {
                    samples = concatenated(&samples, &once);
                    if samples.is_empty() {
                        break;
                    }
                }
                samples
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor, ..
            } => self.within(syntactic_factor, depth),
        }
    }
}

/// Returns the character after `c`, if there is one.
fn after(c: char) -> Option<char> {
    match c {
        '\u{D7FF}' => Some('\u{E000}'),
        _ => char::from_u32(u32::from(c) + 1),
    }
}

/// Returns the shortest of `firsts` followed by `seconds`, within bounds. Both
/// are sorted the shortest first, as samples are, so the samples come in order
/// of length and the search stops once it has enough.
fn concatenated(firsts: &[String], seconds: &[String]) -> Vec<String> {
    let mut samples = HashSet::new();
    for length in 0..=LENGTH {
        for first in firsts.iter().take_while(|first| first.len() <= length) {
            let wanted = length - first.len();
            let start =
                seconds.partition_point(|second| second.len() < wanted);
            let seconds = seconds[start..]
                .iter()
                .take_while(|second| second.len() == wanted);
            for second in seconds {
                samples.insert(first.clone() + second);
                if samples.len() == COUNT {
                    return bounded(samples.into_iter().collect());
                }
            }
        }
    }
    bounded(samples.into_iter().collect())
}

/// Returns the shortest of `samples` that are not too long, each once.
fn bounded(mut samples: Vec<String>) -> Vec<String> {
    samples.retain(|sample| sample.len() <= LENGTH);
    samples
        .sort_unstable_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    samples.dedup();
    samples.truncate(COUNT);
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    fn samples(source: &str) -> Vec<String> {
        let grammar = build_valid(source);
        let mut sampler = Sampler::new(&grammar);
        sampler.samples(&grammar.syntax_rules[0].expression)
    }

    #[test]
    fn tries_one_character_of_each_kind() {
        assert_eq!(
            samples("a = ? U+0061..U+007A ?, ['x'];"),
            ["a", "x", "y", "ax", "xx", "yx"]
        );
    }

    #[test]
    fn repeats_up_to_a_length() {
        let found = samples("a = {'ab'};");
        assert_eq!(found, ["", "ab", "abab", "ababab", "abababab"]);
    }

    #[test]
    fn stops_at_a_depth_of_rules() {
        let found = samples("a = 'x', [a];");
        assert_eq!(found.last().map(String::len), Some(DEPTH + 1));
    }
}
