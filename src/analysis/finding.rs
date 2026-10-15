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
    /// A syntactic exception can never apply, since nothing its factor matches
    /// is also matched by the exception.
    IneffectiveException,
    /// A rule can match nothing and nothing else, so it never reads input.
    EmptyRule { meta_identifier: MetaIdentifier },
    /// An optional sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to take it.
    OptionalFollowConflict { characters: CharacterSet },
    /// A repeated sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to repeat it again.
    RepeatedFollowConflict { characters: CharacterSet },
    /// A syntactic exception matches everything its factor matches, so the
    /// term can never match.
    ExhaustiveException,
    /// Two rules have names that differ only in case.
    CaseOnlyNames {
        meta_identifier: MetaIdentifier,
        other: MetaIdentifier,
    },
}

/// The name of a kind of finding, in kebab case, as an expectation writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindingCode {
    LeftRecursion,
    IndirectLeftRecursion,
    UnproductiveRule,
    EmptyRepetition,
    OverlappingBranches,
    OptionalFollowConflict,
    RepeatedFollowConflict,
    AmbiguousRecursion,
    ShadowedBranch,
    AmbiguousEmptyBranches,
    UnreachableRule,
    DuplicateAlternative,
    IneffectiveException,
    EmptyRule,
    ExhaustiveException,
    CaseOnlyNames,
}

impl FindingCode {
    /// Every code, so a name can be looked up.
    pub(crate) const ALL: [Self; 16] = [
        Self::LeftRecursion,
        Self::IndirectLeftRecursion,
        Self::UnproductiveRule,
        Self::EmptyRepetition,
        Self::OverlappingBranches,
        Self::OptionalFollowConflict,
        Self::RepeatedFollowConflict,
        Self::AmbiguousRecursion,
        Self::ShadowedBranch,
        Self::AmbiguousEmptyBranches,
        Self::UnreachableRule,
        Self::DuplicateAlternative,
        Self::IneffectiveException,
        Self::EmptyRule,
        Self::ExhaustiveException,
        Self::CaseOnlyNames,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::LeftRecursion => "left-recursion",
            Self::IndirectLeftRecursion => "indirect-left-recursion",
            Self::UnproductiveRule => "unproductive-rule",
            Self::EmptyRepetition => "empty-repetition",
            Self::OverlappingBranches => "overlapping-branches",
            Self::OptionalFollowConflict => "optional-follow-conflict",
            Self::RepeatedFollowConflict => "repeated-follow-conflict",
            Self::AmbiguousRecursion => "ambiguous-recursion",
            Self::ShadowedBranch => "shadowed-branch",
            Self::AmbiguousEmptyBranches => "ambiguous-empty-branches",
            Self::UnreachableRule => "unreachable-rule",
            Self::DuplicateAlternative => "duplicate-alternative",
            Self::IneffectiveException => "ineffective-exception",
            Self::EmptyRule => "empty-rule",
            Self::ExhaustiveException => "exhaustive-exception",
            Self::CaseOnlyNames => "case-only-names",
        }
    }

    /// Returns the code with `name`, if any.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.name() == name)
    }

    pub(crate) fn severity(self) -> Severity {
        match self {
            Self::LeftRecursion
            | Self::IndirectLeftRecursion
            | Self::UnproductiveRule
            | Self::EmptyRepetition => Severity::Error,
            Self::OverlappingBranches
            | Self::OptionalFollowConflict
            | Self::RepeatedFollowConflict
            | Self::AmbiguousRecursion
            | Self::ShadowedBranch
            | Self::AmbiguousEmptyBranches
            | Self::UnreachableRule
            | Self::DuplicateAlternative
            | Self::IneffectiveException
            | Self::EmptyRule
            | Self::ExhaustiveException
            | Self::CaseOnlyNames => Severity::Warning,
        }
    }
}

impl FindingKind {
    pub(crate) fn code(&self) -> FindingCode {
        match self {
            Self::LeftRecursion { .. } => FindingCode::LeftRecursion,
            Self::IndirectLeftRecursion { .. } => {
                FindingCode::IndirectLeftRecursion
            }
            Self::UnproductiveRule { .. } => FindingCode::UnproductiveRule,
            Self::EmptyRepetition => FindingCode::EmptyRepetition,
            Self::OverlappingBranches { .. } => {
                FindingCode::OverlappingBranches
            }
            Self::OptionalFollowConflict { .. } => {
                FindingCode::OptionalFollowConflict
            }
            Self::RepeatedFollowConflict { .. } => {
                FindingCode::RepeatedFollowConflict
            }
            Self::AmbiguousRecursion { .. } => FindingCode::AmbiguousRecursion,
            Self::ShadowedBranch => FindingCode::ShadowedBranch,
            Self::AmbiguousEmptyBranches => {
                FindingCode::AmbiguousEmptyBranches
            }
            Self::UnreachableRule { .. } => FindingCode::UnreachableRule,
            Self::DuplicateAlternative => FindingCode::DuplicateAlternative,
            Self::IneffectiveException => FindingCode::IneffectiveException,
            Self::EmptyRule { .. } => FindingCode::EmptyRule,
            Self::ExhaustiveException => FindingCode::ExhaustiveException,
            Self::CaseOnlyNames { .. } => FindingCode::CaseOnlyNames,
        }
    }

    pub(crate) fn severity(&self) -> Severity {
        self.code().severity()
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
            Self::EmptyRule { meta_identifier } => format!(
                "give `{}` something to read, or remove it and every \
                 reference to it",
                meta_identifier.as_str()
            ),
            Self::IneffectiveException => {
                "remove the exception, or write the one that was meant"
                    .to_owned()
            }
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
            Self::ExhaustiveException => {
                "remove this term, or narrow the exception so something is \
                 left"
                    .to_owned()
            }
            Self::CaseOnlyNames {
                meta_identifier,
                other,
            } => format!(
                "rename `{}` or `{}`, or merge them if they mean the same",
                meta_identifier.as_str(),
                other.as_str()
            ),
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
            FindingKind::IneffectiveException => f.write_str(
                "this exception can never apply, since nothing before the `-` \
                 matches the same input",
            ),
            FindingKind::EmptyRule { meta_identifier } => write!(
                f,
                "`{}` can only match nothing, so it never reads any input",
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
            FindingKind::ExhaustiveException => f.write_str(
                "this term can never match, since the exception matches \
                 everything before the `-` does",
            ),
            FindingKind::CaseOnlyNames {
                meta_identifier,
                other,
            } => write!(
                f,
                "`{}` differs from `{}` only in case, which is easy to \
                 mistake",
                meta_identifier.as_str(),
                other.as_str()
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
        code: FindingCode,
        severity: Severity,
        message: &'static str,
        help: &'static str,
    }

    /// One case for every kind of finding. A new kind adds its case here.
    const CASES: &[Case] = &[
        Case {
            source: "a = a, 'x' | 'y';",
            code: FindingCode::LeftRecursion,
            severity: Severity::Error,
            message: "`a` can reach itself before reading anything, so its \
                      parser would never end",
            help: "rewrite the recursion as a repetition, so `a = a, x | y;` \
                   becomes `a = y, {x};`",
        },
        Case {
            source: "a = ? U+0061..U+0063 ? | ? U+0062..U+0064 ?;",
            code: FindingCode::OverlappingBranches,
            severity: Severity::Warning,
            message: "these branches can both start with 'b'..='c'",
            help: "move what the branches start with in common before the \
                   choice, or exclude it from one with `-`",
        },
        Case {
            source: "e = e, '+', e | 'n';",
            code: FindingCode::AmbiguousRecursion,
            severity: Severity::Warning,
            message: "this alternative starts and ends with `e`, so it can \
                      group either way",
            help: "replace one end with a rule that does not start with `e`, \
                   so `e = e, '+', e | 'n';` becomes `e = e, '+', term | \
                   term;` to group from the left",
        },
        Case {
            source: "a = ? U+0061..U+007A ? | 'x';",
            code: FindingCode::ShadowedBranch,
            severity: Severity::Warning,
            message: "another branch can start with every character this one \
                      can",
            help: "remove this branch if the other already matches what it \
                   does, or exclude it from the other with `-`",
        },
        Case {
            source: "a = ['x'] | ['y'];",
            code: FindingCode::AmbiguousEmptyBranches,
            severity: Severity::Warning,
            message: "more than one branch can match nothing",
            help: "make all but one of these branches read something, such as \
                   by removing `[...]` around one of them",
        },
        Case {
            source: "a = {['x']};",
            code: FindingCode::EmptyRepetition,
            severity: Severity::Error,
            message: "this repeated sequence can match nothing, so it could \
                      repeat forever",
            help: "make what repeats read something, such as by removing \
                   `[...]` inside the `{...}`",
        },
        Case {
            source: "a = 'x', a;",
            code: FindingCode::UnproductiveRule,
            severity: Severity::Error,
            message: "`a` can never finish matching, since every way through \
                      it needs a rule that cannot either",
            help: "give `a` an alternative that only reads characters, or \
                   make the recursive part optional with `[...]` or `{...}`",
        },
        Case {
            source: "s = 'x'; a = 'y', [b]; b = 'z', a;",
            code: FindingCode::UnreachableRule,
            severity: Severity::Warning,
            message: "`a` cannot be reached from any start rule, so no parser \
                      uses it",
            help: "remove `a`, or reference it from a rule a parser uses",
        },
        Case {
            source: "s = a; a = b, 'x' | 'y'; b = a, 'z' | 'w';",
            code: FindingCode::IndirectLeftRecursion,
            severity: Severity::Error,
            message: "`a` can reach itself through `b` before reading \
                      anything, so its parser would never end",
            help: "replace `b` in `a` with its definition until the recursion \
                   is direct, then rewrite it as a repetition",
        },
        Case {
            source: "a = 'x' | 'x';",
            code: FindingCode::DuplicateAlternative,
            severity: Severity::Warning,
            message: "this alternative is written the same way as an earlier \
                      one, so both always match the same input",
            help: "remove this alternative",
        },
        Case {
            source: "a = 'x' - 'y';",
            code: FindingCode::IneffectiveException,
            severity: Severity::Warning,
            message: "this exception can never apply, since nothing before \
                      the `-` matches the same input",
            help: "remove the exception, or write the one that was meant",
        },
        Case {
            source: "s = a, 'x'; a = ;",
            code: FindingCode::EmptyRule,
            severity: Severity::Warning,
            message: "`a` can only match nothing, so it never reads any input",
            help: "give `a` something to read, or remove it and every \
                   reference to it",
        },
        Case {
            source: "a = ['x'], 'x';",
            code: FindingCode::OptionalFollowConflict,
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
            code: FindingCode::RepeatedFollowConflict,
            severity: Severity::Warning,
            message: "this repeated sequence and what comes after it can both \
                      start with 'x', so a parser cannot tell whether to \
                      repeat it again",
            help: "end the repetition with something it cannot start with, or \
                   make what comes after it start differently",
        },
        Case {
            source: "a = 'x' - ? U+0061..U+007A ?;",
            code: FindingCode::ExhaustiveException,
            severity: Severity::Warning,
            message: "this term can never match, since the exception matches \
                      everything before the `-` does",
            help: "remove this term, or narrow the exception so something is \
                   left",
        },
        Case {
            source: "s = digit, Digit; digit = '0'; Digit = '1';",
            code: FindingCode::CaseOnlyNames,
            severity: Severity::Warning,
            message: "`Digit` differs from `digit` only in case, which is \
                      easy to mistake",
            help: "rename `Digit` or `digit`, or merge them if they mean the \
                   same",
        },
    ];

    #[test]
    fn has_a_case_for_every_code() {
        for code in FindingCode::ALL {
            assert!(
                CASES.iter().any(|case| case.code == code),
                "no case for {}",
                code.name()
            );
        }
    }

    #[test]
    fn names_every_code_once_in_kebab_case() {
        for code in FindingCode::ALL {
            let name = code.name();
            assert!(
                name.split('-').all(|word| {
                    !word.is_empty()
                        && word.chars().all(|c| c.is_ascii_lowercase())
                }),
                "{name}"
            );
            assert_eq!(FindingCode::from_name(name), Some(code));
        }
        assert_eq!(FindingCode::from_name("left_recursion"), None);
    }

    #[test]
    fn describes_every_kind_of_finding() {
        for case in CASES {
            let finding = crate::analysis::tests::findings(case.source)
                .into_iter()
                .find(|finding| finding.kind.code() == case.code)
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
