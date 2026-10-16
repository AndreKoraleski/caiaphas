//! The matcher runs expressions on texts the way a generated parser does, to
//! tell which texts a branch matches.
//!
//! Each choice takes the branch that matches the most text, unless its rule
//! takes the first match, when it takes the first branch that matches.
//! Optional and repeated sequences take as much as they can. Recursion can
//! make matching run long or deep, so it gives up after a fixed number of
//! steps or levels.

use crate::ir::{Expression, ExpressionKind, Grammar, RuleId};

/// How many expressions matching visits before it gives up.
const STEPS: usize = 10_000;

/// How deeply matching nests before it gives up.
const DEPTH: usize = 200;

/// How many texts a branch can match for the analysis to list them all.
const TEXTS: usize = 64;

/// Returns whether `expression`, written in a rule that takes the longest
/// match, matches all of `text`, or `None` if matching gave up first.
pub(crate) fn matches_all(
    grammar: &Grammar,
    expression: &Expression,
    text: &str,
) -> Option<bool> {
    let mut matcher = Matcher {
        grammar,
        text,
        steps: STEPS,
        depth: DEPTH,
        first_match: false,
    };
    let end = matcher.end(expression, 0)?;
    Some(end == Some(text.len()))
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
            // A first match can leave out what later branches match, so the
            // texts of its rule are not listed.
            if rule.first_match || entered.contains(rule_id) {
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
struct Matcher<'a> {
    grammar: &'a Grammar,
    text: &'a str,
    steps: usize,
    /// How many more levels matching can nest.
    depth: usize,
    /// Whether the choices being matched take their first branch that
    /// matches.
    first_match: bool,
}

impl Matcher<'_> {
    /// Returns where the match of `expression` at `position` ends, if it
    /// matches, or `None` once there are no steps or levels left.
    fn end(
        &mut self,
        expression: &Expression,
        position: usize,
    ) -> Option<Option<usize>> {
        self.steps = self.steps.checked_sub(1)?;
        self.depth = self.depth.checked_sub(1)?;
        let end = self.match_expression(expression, position);
        self.depth += 1;
        end
    }

    fn match_expression(
        &mut self,
        expression: &Expression,
        position: usize,
    ) -> Option<Option<usize>> {
        let rest = &self.text[position..];
        Some(match &expression.kind {
            ExpressionKind::TerminalString(string) => rest
                .starts_with(string.as_str())
                .then(|| position + string.len()),
            ExpressionKind::SpecialSequence(range) => rest
                .chars()
                .next()
                .filter(|c| range.contains(c))
                .map(|c| position + c.len_utf8()),
            ExpressionKind::EmptySequence => Some(position),
            ExpressionKind::MetaIdentifier(RuleId(rule)) => {
                let rule = &self.grammar.syntax_rules[*rule];
                let around =
                    std::mem::replace(&mut self.first_match, rule.first_match);
                let end = self.end(&rule.expression, position);
                self.first_match = around;
                return end;
            }
            ExpressionKind::SingleDefinition(expressions) => {
                return self.sequence(expressions.iter(), position);
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let times = (0..*integer).map(|_| &**syntactic_primary);
                return self.sequence(times, position);
            }
            ExpressionKind::DefinitionsList(expressions) => {
                let mut longest = None;
                for expression in expressions {
                    let end = self.end(expression, position)?;
                    if self.first_match && end.is_some() {
                        return Some(end);
                    }
                    longest = longest.max(end);
                }
                longest
            }
            ExpressionKind::OptionalSequence(expression) => {
                Some(self.end(expression, position)?.unwrap_or(position))
            }
            ExpressionKind::RepeatedSequence(expression) => {
                let mut end = position;
                while let Some(next) = self.end(expression, end)? {
                    if next == end {
                        break;
                    }
                    end = next;
                }
                Some(end)
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                let Some(end) = self.end(syntactic_factor, position)? else {
                    return Some(None);
                };
                // The exception only reads what the factor matched.
                let mut within = Matcher {
                    grammar: self.grammar,
                    text: &self.text[..end],
                    steps: self.steps,
                    depth: self.depth,
                    first_match: self.first_match,
                };
                let excluded =
                    within.end(syntactic_exception, position)? == Some(end);
                self.steps = within.steps;
                (!excluded).then_some(end)
            }
        })
    }

    /// Returns where `expressions` end when matched one after another.
    fn sequence<'e>(
        &mut self,
        expressions: impl Iterator<Item = &'e Expression>,
        position: usize,
    ) -> Option<Option<usize>> {
        let mut end = position;
        for expression in expressions {
            match self.end(expression, end)? {
                Some(next) => end = next,
                None => return Some(None),
            }
        }
        Some(Some(end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    /// Returns whether the first rule of `source` matches all of `text`.
    fn first_rule_matches(source: &str, text: &str) -> Option<bool> {
        let grammar = build_valid(source);
        matches_all(&grammar, &grammar.syntax_rules[0].expression, text)
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
    fn takes_the_longest_branch() {
        let source = "s = 'a' | 'a', 'b' | 'a', 'b', 'c';";
        assert_eq!(first_rule_matches(source, "abc"), Some(true));
        assert_eq!(first_rule_matches(source, "ab"), Some(true));
    }

    #[test]
    fn takes_the_first_branch_that_matches_where_the_rule_asks() {
        let source = "s = k; (* first match *) k = 'i' | 'if';";
        assert_eq!(first_rule_matches(source, "i"), Some(true));
        assert_eq!(first_rule_matches(source, "if"), Some(false));
        assert_eq!(first_rule_texts(source), None);
    }

    #[test]
    fn takes_as_much_as_it_can_without_giving_back() {
        assert_eq!(first_rule_matches("s = {'x'}, 'x';", "xx"), Some(false));
        assert_eq!(first_rule_matches("s = ['x'], 'x';", "x"), Some(false));
        assert_eq!(first_rule_matches("s = ['x'], 'y';", "y"), Some(true));
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
