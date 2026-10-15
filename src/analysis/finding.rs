//! Findings are what the analysis reports about a grammar.

use std::fmt;

use crate::{
    analysis::character_set::CharacterSet, lexer::MetaIdentifier, span::Span,
};

/// A finding is one problem the analysis found, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) kind: FindingKind,
    pub(crate) span: Span,
    /// Other places that explain the finding, such as another branch.
    pub(crate) labels: Vec<Label>,
}

/// A label points at another place a finding involves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: &'static str,
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    /// The grammar cannot become a parser that works.
    Error,
    /// The parser works, but may choose a branch the grammar did not mean.
    Warning,
}

/// What a [`Finding`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FindingKind {
    /// A rule can reach itself before reading anything, so its parser would
    /// never end.
    LeftRecursion { meta_identifier: MetaIdentifier },
    /// Two branches of a choice can start with some of the same characters.
    OverlappingBranches { characters: CharacterSet },
    /// An alternative starts and ends with its own rule, so it can group
    /// either way.
    AmbiguousRecursion { meta_identifier: MetaIdentifier },
    /// An optional sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to take it.
    OptionalFollowConflict { characters: CharacterSet },
    /// A repeated sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to repeat it again.
    RepeatedFollowConflict { characters: CharacterSet },
}

impl FindingKind {
    pub(crate) fn severity(&self) -> Severity {
        match self {
            Self::LeftRecursion { .. } => Severity::Error,
            Self::OverlappingBranches { .. } => Severity::Warning,
            Self::AmbiguousRecursion { .. } => Severity::Warning,
            Self::OptionalFollowConflict { .. }
            | Self::RepeatedFollowConflict { .. } => Severity::Warning,
        }
    }

    /// Returns how to fix the grammar, as the usual remedy for this kind of
    /// problem.
    pub(crate) fn help(&self) -> String {
        match self {
            Self::LeftRecursion { meta_identifier } => {
                let name = meta_identifier.as_str();
                format!(
                    "rewrite the recursion as a repetition, so \
                     `{name} = {name}, x | y;` becomes `{name} = y, {{x}};`"
                )
            }
            Self::AmbiguousRecursion { meta_identifier } => {
                let name = meta_identifier.as_str();
                format!(
                    "replace one end with a rule that does not start with \
                     `{name}`, so `{name} = {name}, '+', {name} | 'n';` \
                     becomes `{name} = {name}, '+', term | term;` to group \
                     from the left"
                )
            }
            Self::OverlappingBranches { .. } => {
                "move what the branches start with in common before the \
                 choice, or exclude it from one with `-`"
                    .to_owned()
            }
            Self::OptionalFollowConflict { .. } => {
                "if the sequence should be taken whenever it can, as usual \
                 for a dangling else, keep it; otherwise make what comes \
                 after it start differently"
                    .to_owned()
            }
            Self::RepeatedFollowConflict { .. } => {
                "end the repetition with something it cannot start with, or \
                 make what comes after it start differently"
                    .to_owned()
            }
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FindingKind::LeftRecursion { meta_identifier } => write!(
                f,
                "`{}` can reach itself before reading anything, so its parser \
                 would never end",
                meta_identifier.as_str()
            ),
            FindingKind::OverlappingBranches { characters } => {
                write!(f, "these branches can both start with {characters}")
            }
            FindingKind::AmbiguousRecursion { meta_identifier } => write!(
                f,
                "this alternative starts and ends with `{}`, so it can group \
                 either way",
                meta_identifier.as_str()
            ),
            FindingKind::OptionalFollowConflict { characters } => write!(
                f,
                "this optional sequence and what comes after it can both \
                 start with {characters}, so a parser cannot tell whether to \
                 take it"
            ),
            FindingKind::RepeatedFollowConflict { characters } => write!(
                f,
                "this repeated sequence and what comes after it can both \
                 start with {characters}, so a parser cannot tell whether to \
                 repeat it again"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grammar with a finding of one kind, how serious it is, its message
    /// and how to fix it.
    struct Case {
        source: &'static str,
        is_kind: fn(&FindingKind) -> bool,
        severity: Severity,
        message: &'static str,
        help: &'static str,
    }

    /// One case for every kind of finding. A new kind adds its case here.
    const CASES: &[Case] = &[
        Case {
            source: "a = a, 'x' | 'y';",
            is_kind: |kind| matches!(kind, FindingKind::LeftRecursion { .. }),
            severity: Severity::Error,
            message: "`a` can reach itself before reading anything, so its \
                      parser would never end",
            help: "rewrite the recursion as a repetition, so `a = a, x | y;` \
                   becomes `a = y, {x};`",
        },
        Case {
            source: "a = ? U+0061..U+0063 ? | ? U+0062..U+0064 ?;",
            is_kind: |kind| {
                matches!(kind, FindingKind::OverlappingBranches { .. })
            },
            severity: Severity::Warning,
            message: "these branches can both start with 'b'..='c'",
            help: "move what the branches start with in common before the \
                   choice, or exclude it from one with `-`",
        },
        Case {
            source: "e = e, '+', e | 'n';",
            is_kind: |kind| {
                matches!(kind, FindingKind::AmbiguousRecursion { .. })
            },
            severity: Severity::Warning,
            message: "this alternative starts and ends with `e`, so it can \
                      group either way",
            help: "replace one end with a rule that does not start with `e`, \
                   so `e = e, '+', e | 'n';` becomes `e = e, '+', term | \
                   term;` to group from the left",
        },
        Case {
            source: "a = ['x'], 'x';",
            is_kind: |kind| {
                matches!(kind, FindingKind::OptionalFollowConflict { .. })
            },
            severity: Severity::Warning,
            message: "this optional sequence and what comes after it can both \
                      start with 'x', so a parser cannot tell whether to take \
                      it",
            help: "if the sequence should be taken whenever it can, as usual \
                   for a dangling else, keep it; otherwise make what comes \
                   after it start differently",
        },
        Case {
            source: "a = {'x'}, 'x';",
            is_kind: |kind| {
                matches!(kind, FindingKind::RepeatedFollowConflict { .. })
            },
            severity: Severity::Warning,
            message: "this repeated sequence and what comes after it can both \
                      start with 'x', so a parser cannot tell whether to \
                      repeat it again",
            help: "end the repetition with something it cannot start with, or \
                   make what comes after it start differently",
        },
    ];

    #[test]
    fn describes_every_kind_of_finding() {
        for case in CASES {
            let finding = crate::analysis::tests::findings(case.source)
                .into_iter()
                .find(|finding| (case.is_kind)(&finding.kind))
                .unwrap_or_else(|| {
                    panic!("no such finding in {:?}", case.source)
                });
            assert_eq!(
                finding.kind.severity(),
                case.severity,
                "{:?}",
                case.source
            );
            assert_eq!(finding.to_string(), case.message, "{:?}", case.source);
            assert_eq!(finding.kind.help(), case.help, "{:?}", case.source);
        }
    }
}
