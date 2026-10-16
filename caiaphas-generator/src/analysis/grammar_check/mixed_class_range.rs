//! A mixed class range is a special sequence whose ends are letters or digits
//! of different kinds, such as `? U+0041..U+007A ?`, from `A` to `z`. The
//! range also takes the characters between the kinds, here `[` to the
//! backtick, though a range written that way seldom means them.

use crate::{
    analysis::{
        character_set::CharacterSet,
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{ExpressionKind, Grammar},
};

/// Reports every special sequence whose ends are letters or digits of
/// different kinds.
pub(super) fn check(
    grammar: &Grammar,
    _: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    grammar.visit(&mut |expression| {
        let ExpressionKind::SpecialSequence(range) = &expression.kind else {
            return;
        };
        let (first, last) = (*range.start(), *range.end());
        let (Some(first_kind), Some(last_kind)) = (kind(first), kind(last))
        else {
            return;
        };
        if first_kind == last_kind {
            return;
        }
        let kinds = ['0'..='9', 'A'..='Z', 'a'..='z']
            .into_iter()
            .fold(CharacterSet::default(), |set, kind| {
                set.union(&CharacterSet::from_range(kind))
            });
        let between =
            CharacterSet::from_range(range.clone()).difference(&kinds);
        findings.push(Finding {
            kind: FindingKind::MixedClassRange {
                first,
                last,
                between,
            },
            span: expression.span,
            labels: Vec::new(),
        });
    });
}

/// Returns which kind of letter or digit `c` is, if it is one: `0` for a
/// digit, `A` for an upper case letter and `a` for a lower case one.
fn kind(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some('0'),
        'A'..='Z' => Some('A'),
        'a'..='z' => Some('a'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn ranges(source: &str) -> Vec<String> {
        findings_with(source, FindingCode::MixedClassRange)
            .into_iter()
            .map(|finding| finding.kind.to_string())
            .collect()
    }

    #[test]
    fn finds_a_range_from_upper_to_lower_case() {
        assert_eq!(
            ranges("a = ? U+0041..U+007A ?;"),
            [
                "this range from `A` to `z` also takes '['..='`', though they \
                 are neither letters nor digits"
            ]
        );
    }

    #[test]
    fn finds_a_range_from_digits_to_letters() {
        assert_eq!(ranges("a = ? U+0030..U+007A ?;").len(), 1);
    }

    #[test]
    fn accepts_ranges_of_one_kind_or_of_other_characters() {
        assert_eq!(
            ranges(
                "a = ? U+0061..U+007A ? | ? U+0030..U+0039 ? \
                 | ? U+0020..U+007E ?;"
            ),
            Vec::<String>::new()
        );
    }
}
