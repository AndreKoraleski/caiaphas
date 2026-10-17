//! An ambiguous split is a text that a sequence can share out between its
//! parts in more than one way, as `{'x'}, ['x']` can with `x`, or that a
//! repetition can cut into rounds in more than one way, as `{'a' | 'aa'}` can
//! with `aa`. A parser gives each part the most text it can, from the first,
//! so it takes one of the splits, which may not be the one meant.
//!
//! Short texts each sequence may match are tried, so a split found is always
//! real, but one that needs a longer text can go unreported. Splits inside a
//! part are left to the checks of that part, and ties between branches to the
//! tied branches check. Greedy rules are left to the checks about them. Rules
//! that take the first match, and parts that hold one, choose where they end
//! as written, and exceptions never use how they split what they match, so
//! those are left alone too.

use crate::{
    analysis::{
        CharacterSet, character_set,
        facts::Facts,
        finding::{Finding, FindingKind},
        matcher,
        samples::Sampler,
    },
    ir::{Expression, ExpressionKind, Grammar, RuleId},
};

/// Reports every sequence and repetition of a rule that is not greedy that can
/// split a text it matches in more than one way.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let mut sampler = Sampler::new(grammar);
    let plain = grammar
        .syntax_rules
        .iter()
        .filter(|rule| !rule.greedy && !rule.first_match);
    for rule in plain {
        visit(&rule.expression, &mut |expression| {
            let (parts, repeated) = match &expression.kind {
                ExpressionKind::SingleDefinition(parts)
                    if (0..parts.len().saturating_sub(1)).any(|index| {
                        can_overlap(facts, &parts[index], &parts[index + 1..])
                    }) =>
                {
                    (parts.as_slice(), false)
                }
                ExpressionKind::RepeatedSequence(round)
                    if can_overlap(
                        facts,
                        round,
                        std::slice::from_ref(round),
                    ) =>
                {
                    (std::slice::from_ref(&**round), true)
                }
                _ => return,
            };
            let split = sampler.samples(expression).into_iter().find(|text| {
                !text.is_empty() && splits(grammar, parts, repeated, text) > 1
            });
            if let Some(text) = split {
                findings.push(Finding {
                    kind: FindingKind::AmbiguousSplit {
                        matched: format!("`{}`", character_set::shown(&text)),
                        repeated,
                    },
                    span: expression.span,
                    labels: Vec::new(),
                });
            }
        });
    }
}

/// Returns whether `part` can end at two places of a text that `rest` goes on
/// to match. For that, `rest` has to start where `part` could have gone on,
/// with a character `part` can hold, unless `part` always ends at one place.
fn can_overlap(
    facts: &Facts<'_>,
    part: &Expression,
    rest: &[Expression],
) -> bool {
    if facts.ends_once(part) || takes_first_match(facts.grammar(), part) {
        return false;
    }
    let mut held = facts.inner(part);
    if facts.is_nullable(part) {
        held = held.union(&facts.first(part));
    }
    let mut starts = CharacterSet::default();
    for next in rest {
        starts = starts.union(&facts.first(next));
        if !facts.is_nullable(next) {
            break;
        }
    }
    !held.intersection(&starts).is_empty()
}

/// Returns whether `expression` holds a rule that takes the first match.
fn takes_first_match(grammar: &Grammar, expression: &Expression) -> bool {
    let mut found = false;
    expression.visit_meta_identifiers(&mut |RuleId(position), _| {
        found |= grammar.syntax_rules[position].first_match;
    });
    found
}

/// Visits `expression` and each expression in it, but for exceptions.
fn visit(expression: &Expression, visit_one: &mut impl FnMut(&Expression)) {
    visit_one(expression);
    if let ExpressionKind::SyntacticTerm {
        syntactic_factor, ..
    } = &expression.kind
    {
        visit(syntactic_factor, visit_one);
        return;
    }
    for child in expression.children() {
        visit(child, visit_one);
    }
}

/// Returns in how many ways, counting up to two, `text` splits between `parts`
/// one after another, or into rounds of the one part when `repeated`.
fn splits(
    grammar: &Grammar,
    parts: &[Expression],
    repeated: bool,
    text: &str,
) -> usize {
    let ends = |part, position| {
        matcher::ends(grammar, part, text, position, false).unwrap_or_default()
    };
    // How many ways reach each place in the text.
    let mut ways = vec![0; text.len() + 1];
    ways[0] = 1;
    if repeated {
        for position in 0..text.len() {
            if ways[position] == 0 {
                continue;
            }
            for end in ends(&parts[0], position) {
                if end > position {
                    ways[end] = (ways[end] + ways[position]).min(2);
                }
            }
        }
    } else {
        for part in parts {
            let mut next = vec![0; text.len() + 1];
            for (position, &reached) in ways.iter().enumerate() {
                if reached == 0 {
                    continue;
                }
                for end in ends(part, position) {
                    next[end] = (next[end] + reached).min(2);
                }
            }
            ways = next;
        }
    }
    ways[text.len()]
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn splits(source: &str) -> Vec<String> {
        findings_with(source, FindingCode::AmbiguousSplit)
            .into_iter()
            .map(|finding| finding.kind.to_string())
            .collect()
    }

    #[test]
    fn finds_a_text_two_parts_can_share_out_either_way() {
        assert_eq!(
            splits("a = {'x'}, ['x'];"),
            [
                "this sequence can split `x` between its parts in more than \
              one way, so the parser gives each part the most it can, from \
              the first"
            ]
        );
        let csv = "table = record, {newline, record}, [newline]; \
                   record = {'x'}; newline = ? U+000A ?;";
        assert_eq!(splits(csv).len(), 1);
        assert!(splits(csv)[0].contains("`\\n`"), "{:?}", splits(csv));
    }

    #[test]
    fn finds_a_text_a_repetition_can_cut_into_rounds_either_way() {
        assert_eq!(
            splits("a = {'a' | 'aa'};"),
            ["this repetition can cut `aa` into rounds in more than one \
              way, so the parser makes each round the most it can, from the \
              first"]
        );
    }

    #[test]
    fn accepts_sequences_that_split_one_way() {
        assert_eq!(splits("a = {'x'}, 'y';"), Vec::<String>::new());
        assert_eq!(splits("a = {'x'}, 'x';"), Vec::<String>::new());
        assert_eq!(
            splits(
                "a = name, '.', ext; name = c, {c}; ext = l, {l}; \
                    c = l | '.'; l = ? U+0061..U+007A ?;"
            ),
            Vec::<String>::new()
        );
        assert_eq!(splits("a = {'x'} | 'y', {'x'};"), Vec::<String>::new());
    }

    #[test]
    fn leaves_exceptions_and_greedy_rules_alone() {
        let comment = "comment = '/*', ({c} - ({c}, '*/', {c})), '*/'; \
                       c = ? U+0020..U+007E ?;";
        assert_eq!(splits(comment), Vec::<String>::new());
        assert_eq!(
            splits("(* greedy *) a = {'x'}, ['x'];"),
            Vec::<String>::new()
        );
    }
}
