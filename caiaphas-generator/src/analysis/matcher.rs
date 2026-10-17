//! The matcher runs expressions on texts the way a generated parser does, to
//! tell which texts a branch matches.
//!
//! It follows every way through a rule, as a parser that tries every way does.
//! In a greedy rule it takes one way as it comes instead: the branch that
//! matches the most, or the first that matches with a first match, and as much
//! of an optional or repeated sequence as it can. Recursion can make matching
//! run long or deep, so it gives up after a fixed number of steps or levels.

use crate::ir::{Expression, ExpressionKind, Grammar, RuleId};

/// How many expressions matching visits before it gives up.
const STEPS: usize = 10_000;

/// How deeply matching nests before it gives up.
const DEPTH: usize = 200;

/// How many texts a branch can match for the analysis to list them all.
const TEXTS: usize = 64;

/// Returns whether `expression`, written in a rule that takes the longest
/// match and is `greedy` or not, matches all of `text`, or `None` if matching
/// gave up first.
pub(crate) fn matches_all(
    grammar: &Grammar,
    expression: &Expression,
    text: &str,
    greedy: bool,
) -> Option<bool> {
    Some(ends(grammar, expression, text, 0, greedy)?.contains(&text.len()))
}

/// Returns where the matches of `expression` in `text` from `position` can
/// end, as [`matches_all`] reads it, or `None` if matching gave up first.
pub(crate) fn ends(
    grammar: &Grammar,
    expression: &Expression,
    text: &str,
    position: usize,
    greedy: bool,
) -> Option<Vec<usize>> {
    let mut matcher = Matcher {
        grammar,
        text,
        steps: STEPS,
        depth: DEPTH,
        first_match: false,
        greedy,
    };
    matcher.ends(expression, position)
}

/// Returns every text `expression` can match, when it can only match a few
/// fixed ones.
pub(crate) fn texts(
    grammar: &Grammar,
    expression: &Expression,
) -> Option<Vec<String>> {
    texts_within(grammar, expression, &mut Vec::new())
}

/// Returns the texts of `expression`, where `entered` holds the rules being
/// listed, so recursion gives no answer.
fn texts_within(
    grammar: &Grammar,
    expression: &Expression,
    entered: &mut Vec<RuleId>,
) -> Option<Vec<String>> {
    let mut texts = |expression| texts_within(grammar, expression, entered);
    let found = match &expression.kind {
        ExpressionKind::TerminalString(string) => vec![string.clone()],
        ExpressionKind::EmptySequence => vec![String::new()],
        ExpressionKind::SpecialSequence(range) => {
            range.clone().map(String::from).take(TEXTS + 1).collect()
        }
        ExpressionKind::DefinitionsList(expressions) => {
            let mut found = Vec::new();
            for expression in expressions {
                found.extend(texts(expression)?);
            }
            found
        }
        ExpressionKind::SingleDefinition(expressions) => {
            let mut found = vec![String::new()];
            for expression in expressions {
                found = concatenate(&found, &texts(expression)?)?;
            }
            found
        }
        ExpressionKind::OptionalSequence(expression) => {
            let mut found = texts(expression)?;
            found.push(String::new());
            found
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => {
            // Only a lone text repeats that often without passing `TEXTS`, and
            // it would be too long to be worth listing.
            if usize::try_from(*integer).ok()? > TEXTS {
                return None;
            }
            let once = texts(syntactic_primary)?;
            let mut found = vec![String::new()];
            for _ in 0..*integer {
                found = concatenate(&found, &once)?;
            }
            found
        }
        ExpressionKind::MetaIdentifier(rule_id) => {
            let RuleId(position) = *rule_id;
            let rule = &grammar.syntax_rules[position];
            // A greedy rule can match less than its expression describes, and
            // a rule entered again could list texts without end, so neither is
            // listed.
            if rule.greedy || entered.contains(rule_id) {
                return None;
            }
            entered.push(*rule_id);
            let found = texts_within(grammar, &rule.expression, entered);
            entered.pop();
            found?
        }
        ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::SyntacticTerm { .. } => return None,
    };
    (found.len() <= TEXTS).then_some(found)
}

/// Returns every text that is one of `firsts` followed by one of `seconds`,
/// unless there are too many to list.
fn concatenate(firsts: &[String], seconds: &[String]) -> Option<Vec<String>> {
    if firsts.len() * seconds.len() > TEXTS {
        return None;
    }
    Some(
        firsts
            .iter()
            .flat_map(|first| {
                seconds.iter().map(move |second| first.clone() + second)
            })
            .collect(),
    )
}

/// Runs expressions on a text, counting down the steps left.
#[derive(Clone, Copy)]
struct Matcher<'a> {
    grammar: &'a Grammar,
    text: &'a str,
    steps: usize,
    /// How many more levels matching can nest.
    depth: usize,
    /// Whether the choices being matched take their first branch that matches.
    first_match: bool,
    /// Whether the rule being matched takes each way as it comes.
    greedy: bool,
}

impl Matcher<'_> {
    /// Returns where the matches of `expression` at `position` can end, at
    /// most one in a greedy rule, or `None` once there are no steps or levels
    /// left.
    fn ends(
        &mut self,
        expression: &Expression,
        position: usize,
    ) -> Option<Vec<usize>> {
        self.steps = self.steps.checked_sub(1)?;
        self.depth = self.depth.checked_sub(1)?;
        let ends = self.match_expression(expression, position);
        self.depth += 1;
        ends
    }

    fn match_expression(
        &mut self,
        expression: &Expression,
        position: usize,
    ) -> Option<Vec<usize>> {
        let rest = &self.text[position..];
        Some(match &expression.kind {
            ExpressionKind::TerminalString(string) => rest
                .starts_with(string.as_str())
                .then(|| position + string.len())
                .into_iter()
                .collect(),
            ExpressionKind::SpecialSequence(range) => rest
                .chars()
                .next()
                .filter(|c| range.contains(c))
                .map(|c| position + c.len_utf8())
                .into_iter()
                .collect(),
            ExpressionKind::EmptySequence => vec![position],
            ExpressionKind::MetaIdentifier(RuleId(rule)) => {
                let rule = &self.grammar.syntax_rules[*rule];
                let around = (self.first_match, self.greedy);
                (self.first_match, self.greedy) =
                    (rule.first_match, rule.greedy);
                let ends = self.ends(&rule.expression, position);
                (self.first_match, self.greedy) = around;
                let ends = ends?;
                // A greedy rule takes the end the rule it reads prefers.
                if !self.greedy || ends.len() < 2 {
                    return Some(ends);
                }
                if rule.first_match {
                    vec![ends[0]]
                } else {
                    ends.into_iter().max().into_iter().collect()
                }
            }
            ExpressionKind::SingleDefinition(expressions) => {
                return self.sequence(expressions.iter(), position);
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let mut ends = vec![position];
                for _ in 0..*integer {
                    let mut next = Vec::new();
                    for &start in &ends {
                        add_all(
                            &mut next,
                            self.ends(syntactic_primary, start)?,
                        );
                    }
                    next.sort_unstable();
                    // Once a round ends where the one before it did, so does
                    // every later round, and once none matches, none will.
                    let done = next == ends || next.is_empty();
                    ends = next;
                    if done {
                        break;
                    }
                }
                return Some(ends);
            }
            ExpressionKind::DefinitionsList(expressions) => {
                let mut ends = Vec::new();
                for expression in expressions {
                    let found = self.ends(expression, position)?;
                    if self.greedy && self.first_match && !found.is_empty() {
                        return Some(found);
                    }
                    add_all(&mut ends, found);
                }
                if self.greedy {
                    ends.into_iter().max().into_iter().collect()
                } else {
                    ends
                }
            }
            ExpressionKind::OptionalSequence(expression) => {
                let mut ends = self.ends(expression, position)?;
                if !self.greedy || ends.is_empty() {
                    add_all(&mut ends, [position]);
                }
                ends
            }
            ExpressionKind::RepeatedSequence(expression) => {
                return self.rounds(expression, position);
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                let mut ends = Vec::new();
                for end in self.ends(syntactic_factor, position)? {
                    // The exception only reads what the factor matched.
                    let mut within = Matcher {
                        text: &self.text[..end],
                        ..*self
                    };
                    let excluded = within
                        .ends(syntactic_exception, position)?
                        .contains(&end);
                    self.steps = within.steps;
                    if !excluded {
                        ends.push(end);
                    }
                }
                ends
            }
        })
    }

    /// Returns where `expressions` can end when matched one after another.
    fn sequence<'e>(
        &mut self,
        expressions: impl Iterator<Item = &'e Expression>,
        position: usize,
    ) -> Option<Vec<usize>> {
        let mut ends = vec![position];
        for expression in expressions {
            let mut next = Vec::new();
            for start in ends {
                add_all(&mut next, self.ends(expression, start)?);
            }
            ends = next;
        }
        Some(ends)
    }

    /// Returns where `expression`, repeated any number of times from
    /// `position`, can end. A greedy rule repeats it as long as it matches.
    fn rounds(
        &mut self,
        expression: &Expression,
        position: usize,
    ) -> Option<Vec<usize>> {
        let mut ends = vec![position];
        let mut index = 0;
        while let Some(&start) = ends.get(index) {
            index += 1;
            let found = self.ends(expression, start)?;
            if self.greedy {
                match found.first() {
                    Some(&next) if next != start => ends = vec![next],
                    _ => return Some(vec![start]),
                }
                index = 0;
            } else {
                add_all(&mut ends, found);
            }
        }
        Some(ends)
    }
}

/// Adds each of `found` to `ends` that is not there already.
fn add_all(ends: &mut Vec<usize>, found: impl IntoIterator<Item = usize>) {
    for end in found {
        if !ends.contains(&end) {
            ends.push(end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    /// Returns whether the first rule of `source` matches all of `text`.
    fn first_rule_matches(source: &str, text: &str) -> Option<bool> {
        let grammar = build_valid(source);
        let rule = &grammar.syntax_rules[0];
        matches_all(&grammar, &rule.expression, text, rule.greedy)
    }

    /// Returns the texts the first rule of `source` can match, sorted.
    fn first_rule_texts(source: &str) -> Option<Vec<String>> {
        let grammar = build_valid(source);
        let mut found = texts(&grammar, &grammar.syntax_rules[0].expression)?;
        found.sort();
        Some(found)
    }

    #[test]
    fn matches_a_rule_with_its_references() {
        let source = "word = letter, {letter}; letter = ? U+0061..U+007A ?;";
        assert_eq!(first_rule_matches(source, "if"), Some(true));
        assert_eq!(first_rule_matches(source, "if2"), Some(false));
        assert_eq!(first_rule_matches(source, ""), Some(false));
    }

    #[test]
    fn repeats_a_count_only_while_it_changes_where_matches_end() {
        let most = "s = 4294967295 * ['x'];";
        assert_eq!(first_rule_matches(most, "xx"), Some(true));
        let every = "s = 4294967295 * 'x';";
        assert_eq!(first_rule_matches(every, "xx"), Some(false));
        assert_eq!(first_rule_texts("s = 4294967295 * 'x';"), None);
    }

    #[test]
    fn takes_the_longest_branch() {
        let source = "s = 'a' | 'a', 'b' | 'a', 'b', 'c';";
        assert_eq!(first_rule_matches(source, "abc"), Some(true));
        assert_eq!(first_rule_matches(source, "ab"), Some(true));
    }

    #[test]
    fn follows_every_way_outside_a_greedy_rule() {
        let source = "s = k; (* first match *) k = 'i' | 'if';";
        assert_eq!(first_rule_matches(source, "if"), Some(true));
        assert_eq!(first_rule_matches("s = {'x'}, 'x';", "xx"), Some(true));
        assert_eq!(first_rule_matches("s = ['x'], 'x';", "x"), Some(true));
        assert_eq!(
            first_rule_texts(source),
            Some(vec!["i".to_owned(), "if".to_owned()])
        );
    }

    #[test]
    fn takes_the_first_branch_that_matches_in_a_greedy_rule() {
        let source = "s = k; (* greedy *) (* first match *) k = 'i' | 'if';";
        assert_eq!(first_rule_matches(source, "i"), Some(true));
        assert_eq!(first_rule_matches(source, "if"), Some(false));
        assert_eq!(first_rule_texts(source), None);
    }

    #[test]
    fn takes_as_much_as_it_can_without_giving_back_in_a_greedy_rule() {
        let matches = |source: &str, text| {
            first_rule_matches(&format!("(* greedy *) {source}"), text)
        };
        assert_eq!(matches("s = {'x'}, 'x';", "xx"), Some(false));
        assert_eq!(matches("s = ['x'], 'x';", "x"), Some(false));
        assert_eq!(matches("s = ['x'], 'y';", "y"), Some(true));
    }

    #[test]
    fn repeats_a_factor_its_number_of_times() {
        assert_eq!(first_rule_matches("s = 3 * 'x';", "xxx"), Some(true));
        assert_eq!(first_rule_matches("s = 3 * 'x';", "xx"), Some(false));
    }

    #[test]
    fn leaves_out_what_the_exception_matches() {
        let source = "s = letter - 'if'; letter = ? U+0061..U+007A ?, \
                      {? U+0061..U+007A ?};";
        assert_eq!(first_rule_matches(source, "in"), Some(true));
        assert_eq!(first_rule_matches(source, "if"), Some(false));
        assert_eq!(first_rule_matches(source, "iff"), Some(true));
        let source = "s = (letter, letter) - 'if'; letter = 'i' | 'f';";
        assert_eq!(first_rule_matches(source, "fi"), Some(true));
        assert_eq!(first_rule_matches(source, "if"), Some(false));
    }

    #[test]
    fn goes_on_through_a_long_but_shallow_match() {
        assert_eq!(
            first_rule_matches("s = {'x'};", &"x".repeat(300)),
            Some(true)
        );
    }

    #[test]
    fn gives_up_on_recursion_that_never_ends() {
        assert_eq!(first_rule_matches("s = s, 'x' | 'y';", "y"), None);
    }

    #[test]
    fn lists_the_texts_of_fixed_branches() {
        assert_eq!(
            first_rule_texts("s = ('i', ('f' | 'n')) | k, ['!']; k = 'do';"),
            Some(vec![
                "do".to_owned(),
                "do!".to_owned(),
                "if".to_owned(),
                "in".to_owned(),
            ])
        );
        assert_eq!(
            first_rule_texts("s = 2 * ? U+0061..U+0062 ?;"),
            Some(vec![
                "aa".to_owned(),
                "ab".to_owned(),
                "ba".to_owned(),
                "bb".to_owned(),
            ])
        );
    }

    #[test]
    fn lists_up_to_its_limit_of_texts() {
        let found = first_rule_texts("s = 6 * ('a' | 'b');");
        assert_eq!(found.map(|texts| texts.len()), Some(64));
        assert_eq!(first_rule_texts("s = 30 * ('a' | 'b');"), None);
    }

    #[test]
    fn lists_no_texts_for_repetition_recursion_or_too_many() {
        assert_eq!(first_rule_texts("s = {'x'};"), None);
        assert_eq!(first_rule_texts("s = 'x', [s];"), None);
        assert_eq!(first_rule_texts("s = 'x' - 'y';"), None);
        assert_eq!(first_rule_texts("s = ? U+0000..U+00FF ?;"), None);
        assert_eq!(
            first_rule_texts("s = 4 * ('a' | 'b' | 'c'); t = s;"),
            None
        );
    }
}
