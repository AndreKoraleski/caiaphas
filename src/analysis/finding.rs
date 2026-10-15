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
    /// Every character a branch can start with, another branch of the same
    /// choice can start with too.
    ShadowedBranch,
    /// More than one branch of a choice can match nothing.
    AmbiguousEmptyBranches,
    /// A repeated sequence can match nothing, so it could repeat forever.
    EmptyRepetition,
    /// No input can ever finish matching a rule.
    UnproductiveRule { meta_identifier: MetaIdentifier },
    /// No start rule can reach a rule, so no parser uses it.
    UnreachableRule { meta_identifier: MetaIdentifier },
    /// A rule can reach itself through other rules before reading anything, so
    /// its parser would never end.
    IndirectLeftRecursion {
        meta_identifier: MetaIdentifier,
        through: MetaIdentifier,
    },
    /// An alternative is written the same way as an earlier alternative of the
    /// same choice.
    DuplicateAlternative,
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
            Self::ShadowedBranch => Severity::Warning,
            Self::AmbiguousEmptyBranches => Severity::Warning,
            Self::EmptyRepetition => Severity::Error,
            Self::UnproductiveRule { .. } => Severity::Error,
            Self::UnreachableRule { .. } => Severity::Warning,
            Self::IndirectLeftRecursion { .. } => Severity::Error,
            Self::DuplicateAlternative => Severity::Warning,
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
            Self::IndirectLeftRecursion {
                meta_identifier,
                through,
            } => format!(
                "replace `{}` in `{}` with its definition until the recursion \
                 is direct, then rewrite it as a repetition",
                through.as_str(),
                meta_identifier.as_str()
            ),
            Self::UnproductiveRule { meta_identifier } => format!(
                "give `{}` an alternative that only reads characters, or make \
                 the recursive part optional with `[...]` or `{{...}}`",
                meta_identifier.as_str()
            ),
            Self::UnreachableRule { meta_identifier } => format!(
                "remove `{}`, or reference it from a rule a parser uses",
                meta_identifier.as_str()
            ),
            Self::AmbiguousRecursion { meta_identifier } => {
                let name = meta_identifier.as_str();
                format!(
                    "replace one end with a rule that does not start with \
                     `{name}`, so `{name} = {name}, '+', {name} | 'n';` \
                     becomes `{name} = {name}, '+', term | term;` to group \
                     from the left"
                )
            }
            Self::DuplicateAlternative => "remove this alternative".to_owned(),
            Self::OverlappingBranches { .. } => {
                "move what the branches start with in common before the \
                 choice, or exclude it from one with `-`"
                    .to_owned()
            }
            Self::ShadowedBranch => {
                "remove this branch if the other already matches what it \
                 does, or exclude it from the other with `-`"
                    .to_owned()
            }
            Self::AmbiguousEmptyBranches => {
                "make all but one of these branches read something, such as \
                 by removing `[...]` around one of them"
                    .to_owned()
            }
            Self::EmptyRepetition => {
                "make what repeats read something, such as by removing \
                 `[...]` inside the `{...}`"
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
            FindingKind::ShadowedBranch => f.write_str(
                "another branch can start with every character this one can",
            ),
            FindingKind::AmbiguousEmptyBranches => {
                f.write_str("more than one branch can match nothing")
            }
            FindingKind::EmptyRepetition => f.write_str(
                "this repeated sequence can match nothing, so it could repeat \
                 forever",
            ),
            FindingKind::UnproductiveRule { meta_identifier } => write!(
                f,
                "`{}` can never finish matching, since every way through it \
                 needs a rule that cannot either",
                meta_identifier.as_str()
            ),
            FindingKind::UnreachableRule { meta_identifier } => write!(
                f,
                "`{}` cannot be reached from any start rule, so no parser \
                 uses it",
                meta_identifier.as_str()
            ),
            FindingKind::IndirectLeftRecursion {
                meta_identifier,
                through,
            } => write!(
                f,
                "`{}` can reach itself through `{}` before reading anything, \
                 so its parser would never end",
                meta_identifier.as_str(),
                through.as_str()
            ),
            FindingKind::DuplicateAlternative => f.write_str(
                "this alternative is written the same way as an earlier one, \
                 so both always match the same input",
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
            source: "a = ? U+0061..U+007A ? | 'x';",
            is_kind: |kind| *kind == FindingKind::ShadowedBranch,
            severity: Severity::Warning,
            message: "another branch can start with every character this one \
                      can",
            help: "remove this branch if the other already matches what it \
                   does, or exclude it from the other with `-`",
        },
        Case {
            source: "a = ['x'] | ['y'];",
            is_kind: |kind| *kind == FindingKind::AmbiguousEmptyBranches,
            severity: Severity::Warning,
            message: "more than one branch can match nothing",
            help: "make all but one of these branches read something, such as \
                   by removing `[...]` around one of them",
        },
        Case {
            source: "a = {['x']};",
            is_kind: |kind| *kind == FindingKind::EmptyRepetition,
            severity: Severity::Error,
            message: "this repeated sequence can match nothing, so it could \
                      repeat forever",
            help: "make what repeats read something, such as by removing \
                   `[...]` inside the `{...}`",
        },
        Case {
            source: "a = 'x', a;",
            is_kind: |kind| {
                matches!(kind, FindingKind::UnproductiveRule { .. })
            },
            severity: Severity::Error,
            message: "`a` can never finish matching, since every way through \
                      it needs a rule that cannot either",
            help: "give `a` an alternative that only reads characters, or \
                   make the recursive part optional with `[...]` or `{...}`",
        },
        Case {
            source: "s = 'x'; a = 'y', [b]; b = 'z', a;",
            is_kind: |kind| {
                matches!(kind, FindingKind::UnreachableRule { .. })
            },
            severity: Severity::Warning,
            message: "`a` cannot be reached from any start rule, so no parser \
                      uses it",
            help: "remove `a`, or reference it from a rule a parser uses",
        },
        Case {
            source: "s = a; a = b, 'x' | 'y'; b = a, 'z' | 'w';",
            is_kind: |kind| {
                matches!(kind, FindingKind::IndirectLeftRecursion { .. })
            },
            severity: Severity::Error,
            message: "`a` can reach itself through `b` before reading \
                      anything, so its parser would never end",
            help: "replace `b` in `a` with its definition until the recursion \
                   is direct, then rewrite it as a repetition",
        },
        Case {
            source: "a = 'x' | 'x';",
            is_kind: |kind| *kind == FindingKind::DuplicateAlternative,
            severity: Severity::Warning,
            message: "this alternative is written the same way as an earlier \
                      one, so both always match the same input",
            help: "remove this alternative",
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
