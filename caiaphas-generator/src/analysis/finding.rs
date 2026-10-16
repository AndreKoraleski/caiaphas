//! Findings are what the analysis reports about a grammar.

use std::fmt;

use crate::{
    analysis::character_set::CharacterSet,
    diagnostic::{Diagnostic, Label, Severity},
    lexer::MetaIdentifier,
    span::Span,
};

/// A finding is one problem the analysis found, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) kind: FindingKind,
    pub(crate) span: Span,
    /// Other places that explain the finding, such as another branch.
    pub(crate) labels: Vec<Label>,
}

/// What a [`Finding`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FindingKind {
    /// A rule can reach itself before reading anything, so its parser would
    /// never end.
    LeftRecursion { meta_identifier: MetaIdentifier },
    /// A rule can reach itself through other rules before reading anything, so
    /// its parser would never end.
    IndirectLeftRecursion {
        meta_identifier: MetaIdentifier,
        through: MetaIdentifier,
    },
    /// No input can ever finish matching a rule.
    UnproductiveRule { meta_identifier: MetaIdentifier },
    /// A repeated sequence can match nothing, so it could repeat forever.
    NullableRepeatedSequence,
    /// Two alternatives can match the same text, so a parser can only take the
    /// one written first.
    TiedBranches { matched: String },
    /// An alternative goes on from where another ends with characters that can
    /// also come after the choice, so a parser cannot tell which to take.
    LongerBranchConflict { characters: CharacterSet },
    /// An optional sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to take it.
    OptionalFollowConflict { characters: CharacterSet },
    /// A repeated sequence can start with characters that can also come right
    /// after it, so a parser cannot tell whether to repeat it again.
    RepeatedFollowConflict { characters: CharacterSet },
    /// An optional sequence takes every character that can come after it, so
    /// skipping it never leads to a match.
    RequiredOptionalSequence,
    /// A repeated sequence takes every character that can come after it, so
    /// nothing after it can ever match.
    DeadEndRepeatedSequence,
    /// More than one branch of a choice can match nothing.
    AmbiguousNullableBranches,
    /// No start rule can reach a rule, so no parser uses it.
    UnreachableRule { meta_identifier: MetaIdentifier },
    /// An alternative is written the same way as an earlier alternative of the
    /// same choice.
    DuplicateAlternative,
    /// A syntactic exception can never apply, since nothing its factor matches
    /// is also matched by the exception.
    IneffectiveException,
    /// A rule can match nothing and nothing else, so it never reads input.
    EmptyRule { meta_identifier: MetaIdentifier },
    /// A syntactic exception matches everything its factor matches, so the
    /// term can never match.
    ExhaustiveException,
    /// Two rules have names that differ only in case.
    CaseOnlyNames {
        meta_identifier: MetaIdentifier,
        other: MetaIdentifier,
    },
    /// An expectation names no kind of finding.
    UnknownExpectation { code: String },
    /// An expectation names a kind of finding that cannot be intended, such as
    /// an error.
    UnexpectableFinding { code: FindingCode },
    /// An expectation finds no finding of its kind in its rule.
    UnfulfilledExpectation { code: FindingCode },
}

/// Defines [`FindingCode`], its [`ALL`](FindingCode::ALL) and the name of each
/// code from one list, so `ALL` cannot miss a code.
macro_rules! finding_codes {
    ($($code:ident => $name:literal,)*) => {
        /// The kind of a finding without its details, which an expectation
        /// names in kebab case.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum FindingCode {
            $($code,)*
        }

        impl FindingCode {
            /// Every code, so a name can be looked up.
            pub(crate) const ALL: &[Self] = &[$(Self::$code,)*];

            pub(crate) fn name(self) -> &'static str {
                match self {
                    $(Self::$code => $name,)*
                }
            }
        }
    };
}

finding_codes! {
    LeftRecursion => "left-recursion",
    IndirectLeftRecursion => "indirect-left-recursion",
    UnproductiveRule => "unproductive-rule",
    NullableRepeatedSequence => "nullable-repeated-sequence",
    TiedBranches => "tied-branches",
    LongerBranchConflict => "longer-branch-conflict",
    OptionalFollowConflict => "optional-follow-conflict",
    RepeatedFollowConflict => "repeated-follow-conflict",
    RequiredOptionalSequence => "required-optional-sequence",
    DeadEndRepeatedSequence => "dead-end-repeated-sequence",
    AmbiguousNullableBranches => "ambiguous-nullable-branches",
    UnreachableRule => "unreachable-rule",
    DuplicateAlternative => "duplicate-alternative",
    IneffectiveException => "ineffective-exception",
    EmptyRule => "empty-rule",
    ExhaustiveException => "exhaustive-exception",
    CaseOnlyNames => "case-only-names",
    UnknownExpectation => "unknown-expectation",
    UnexpectableFinding => "unexpectable-finding",
    UnfulfilledExpectation => "unfulfilled-expectation",
}

impl FindingCode {
    /// Returns the code with `name`, if any.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|code| code.name() == name)
    }

    pub(crate) fn severity(self) -> Severity {
        match self {
            Self::LeftRecursion
            | Self::IndirectLeftRecursion
            | Self::UnproductiveRule
            | Self::NullableRepeatedSequence
            | Self::UnknownExpectation
            | Self::UnexpectableFinding => Severity::Error,
            Self::TiedBranches
            | Self::LongerBranchConflict
            | Self::OptionalFollowConflict
            | Self::RepeatedFollowConflict
            | Self::RequiredOptionalSequence
            | Self::DeadEndRepeatedSequence
            | Self::AmbiguousNullableBranches
            | Self::UnreachableRule
            | Self::DuplicateAlternative
            | Self::IneffectiveException
            | Self::EmptyRule
            | Self::ExhaustiveException
            | Self::CaseOnlyNames
            | Self::UnfulfilledExpectation => Severity::Warning,
        }
    }

    /// Returns whether an expectation can mark a finding with this code as
    /// intended. Only warnings about the grammar itself can be.
    pub(crate) fn is_expectable(self) -> bool {
        self.intended_behaviour().is_some()
    }

    /// Returns what happens when an expectation marks a finding with this code
    /// as intended, if one can.
    fn intended_behaviour(self) -> Option<&'static str> {
        match self {
            Self::TiedBranches
            | Self::DuplicateAlternative
            | Self::AmbiguousNullableBranches => Some(
                "the parser takes the branch that matches the most, and the \
                 first written of those that tie",
            ),
            Self::LongerBranchConflict => {
                Some("the parser takes the longer branch whenever it matches")
            }
            Self::OptionalFollowConflict => Some(
                "the parser takes the sequence whenever it matches, as usual \
                 for a dangling else",
            ),
            Self::RepeatedFollowConflict => Some(
                "the parser repeats the sequence as long as it matches, so it \
                 extends as far as it can",
            ),
            Self::RequiredOptionalSequence
            | Self::DeadEndRepeatedSequence
            | Self::UnreachableRule
            | Self::IneffectiveException
            | Self::EmptyRule
            | Self::ExhaustiveException
            | Self::CaseOnlyNames => Some("the grammar stays as it is"),
            Self::LeftRecursion
            | Self::IndirectLeftRecursion
            | Self::UnproductiveRule
            | Self::NullableRepeatedSequence
            | Self::UnknownExpectation
            | Self::UnexpectableFinding
            | Self::UnfulfilledExpectation => None,
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
            Self::NullableRepeatedSequence => {
                FindingCode::NullableRepeatedSequence
            }
            Self::TiedBranches { .. } => FindingCode::TiedBranches,
            Self::LongerBranchConflict { .. } => {
                FindingCode::LongerBranchConflict
            }
            Self::OptionalFollowConflict { .. } => {
                FindingCode::OptionalFollowConflict
            }
            Self::RepeatedFollowConflict { .. } => {
                FindingCode::RepeatedFollowConflict
            }
            Self::RequiredOptionalSequence => {
                FindingCode::RequiredOptionalSequence
            }
            Self::DeadEndRepeatedSequence => {
                FindingCode::DeadEndRepeatedSequence
            }
            Self::AmbiguousNullableBranches => {
                FindingCode::AmbiguousNullableBranches
            }
            Self::UnreachableRule { .. } => FindingCode::UnreachableRule,
            Self::DuplicateAlternative => FindingCode::DuplicateAlternative,
            Self::IneffectiveException => FindingCode::IneffectiveException,
            Self::EmptyRule { .. } => FindingCode::EmptyRule,
            Self::ExhaustiveException => FindingCode::ExhaustiveException,
            Self::CaseOnlyNames { .. } => FindingCode::CaseOnlyNames,
            Self::UnknownExpectation { .. } => FindingCode::UnknownExpectation,
            Self::UnexpectableFinding { .. } => {
                FindingCode::UnexpectableFinding
            }
            Self::UnfulfilledExpectation { .. } => {
                FindingCode::UnfulfilledExpectation
            }
        }
    }

    pub(crate) fn severity(&self) -> Severity {
        self.code().severity()
    }

    /// Returns how to fix the grammar, and how to mark the finding as intended
    /// when it can be.
    pub(crate) fn help(&self) -> String {
        let code = self.code();
        let remedy = self.remedy();
        match code.intended_behaviour() {
            Some(behaviour) => format!(
                "{remedy}; if this is intended, write `(* expect {}: reason \
                 *)` right before the rule, and {behaviour}",
                code.name()
            ),
            None => remedy,
        }
    }

    /// Returns the usual remedy for this kind of problem.
    fn remedy(&self) -> String {
        match self {
            Self::LeftRecursion { meta_identifier } => {
                let name = meta_identifier.as_str();
                format!(
                    "rewrite the recursion as a repetition, so `{name} = \
                     {name}, x | y;` becomes `{name} = y, {{x}};`"
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
            Self::NullableRepeatedSequence => {
                "make what repeats read something, such as by removing \
                 `[...]` inside the `{...}`"
                    .to_owned()
            }
            Self::TiedBranches { .. } => {
                "exclude what they share from one of them with `-`, such as \
                 `identifier - keyword`"
                    .to_owned()
            }
            Self::LongerBranchConflict { .. } => {
                "make what comes after the choice start differently from how \
                 this branch goes on"
                    .to_owned()
            }
            Self::OptionalFollowConflict { .. } => {
                "make what comes after the sequence start differently"
                    .to_owned()
            }
            Self::RepeatedFollowConflict { .. } => {
                "end the repetition with something it cannot start with, or \
                 make what comes after it start differently"
                    .to_owned()
            }
            Self::RequiredOptionalSequence => {
                "remove the `[...]` if the sequence is always needed, or make \
                 what comes after it start with a character it cannot take"
                    .to_owned()
            }
            Self::DeadEndRepeatedSequence => {
                "move what comes after the repetition before it, so `{'x'}, \
                 'x'` becomes `'x', {'x'}`, or make what comes after it start \
                 with a character it cannot take"
                    .to_owned()
            }
            Self::AmbiguousNullableBranches => {
                "make all but one of these branches read something, such as \
                 by removing `[...]` around one of them"
                    .to_owned()
            }
            Self::UnreachableRule { meta_identifier } => format!(
                "remove `{}`, or reference it from a rule a parser uses",
                meta_identifier.as_str()
            ),
            Self::DuplicateAlternative => "remove this alternative".to_owned(),
            Self::IneffectiveException => {
                "remove the exception, or write the one that was meant"
                    .to_owned()
            }
            Self::EmptyRule { meta_identifier } => format!(
                "give `{}` something to read, or remove it and every \
                 reference to it",
                meta_identifier.as_str()
            ),
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
            Self::UnknownExpectation { .. } => {
                "write the code a finding reports, such as \
                 `repeated-follow-conflict`"
                    .to_owned()
            }
            Self::UnexpectableFinding { .. } => {
                "fix the grammar instead, since no parser can work with an \
                 error in it"
                    .to_owned()
            }
            Self::UnfulfilledExpectation { .. } => {
                "remove the expectation, since the rule no longer has that \
                 finding"
                    .to_owned()
            }
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftRecursion { meta_identifier } => write!(
                f,
                "`{}` can reach itself before reading anything, so its parser \
                 would never end",
                meta_identifier.as_str()
            ),
            Self::IndirectLeftRecursion {
                meta_identifier,
                through,
            } => write!(
                f,
                "`{}` can reach itself through `{}` before reading anything, \
                 so its parser would never end",
                meta_identifier.as_str(),
                through.as_str()
            ),
            Self::UnproductiveRule { meta_identifier } => write!(
                f,
                "`{}` can never finish matching, since every way through it \
                 needs a rule that cannot either",
                meta_identifier.as_str()
            ),
            Self::NullableRepeatedSequence => f.write_str(
                "this repeated sequence can match nothing, so it could repeat \
                 forever",
            ),
            Self::TiedBranches { matched } => write!(
                f,
                "these branches can both match {matched}, so the parser takes \
                 the one written first"
            ),
            Self::LongerBranchConflict { characters } => write!(
                f,
                "this branch goes on from where another ends with \
                 {characters}, which can also come after the choice, so a \
                 parser cannot tell which to take"
            ),
            Self::OptionalFollowConflict { characters } => write!(
                f,
                "this optional sequence and what comes after it can both \
                 start with {characters}, so a parser cannot tell whether to \
                 take it"
            ),
            Self::RepeatedFollowConflict { characters } => write!(
                f,
                "this repeated sequence and what comes after it can both \
                 start with {characters}, so a parser cannot tell whether to \
                 repeat it again"
            ),
            Self::RequiredOptionalSequence => f.write_str(
                "this optional sequence takes every character that can come \
                 after it, so skipping it never leads to a match",
            ),
            Self::DeadEndRepeatedSequence => f.write_str(
                "this repeated sequence takes every character that can come \
                 after it, so nothing after it can ever match",
            ),
            Self::AmbiguousNullableBranches => {
                f.write_str("more than one branch can match nothing")
            }
            Self::UnreachableRule { meta_identifier } => write!(
                f,
                "`{}` cannot be reached from any start rule, so no parser \
                 uses it",
                meta_identifier.as_str()
            ),
            Self::DuplicateAlternative => f.write_str(
                "this alternative is written the same way as an earlier one, \
                 so both always match the same input",
            ),
            Self::IneffectiveException => f.write_str(
                "this exception can never apply, since nothing before the `-` \
                 matches the same input",
            ),
            Self::EmptyRule { meta_identifier } => write!(
                f,
                "`{}` can only match nothing, so it never reads any input",
                meta_identifier.as_str()
            ),
            Self::ExhaustiveException => f.write_str(
                "this term can never match, since the exception matches \
                 everything before the `-` does",
            ),
            Self::CaseOnlyNames {
                meta_identifier,
                other,
            } => write!(
                f,
                "`{}` differs from `{}` only in case, which is easy to \
                 mistake",
                meta_identifier.as_str(),
                other.as_str()
            ),
            Self::UnknownExpectation { code } => {
                write!(f, "`{code}` is not the code of any finding")
            }
            Self::UnexpectableFinding { code } => {
                let reason = if code.severity() == Severity::Error {
                    "an error"
                } else {
                    "about expectations"
                };
                write!(
                    f,
                    "`{}` is {reason}, so it cannot be expected",
                    code.name()
                )
            }
            Self::UnfulfilledExpectation { code } => write!(
                f,
                "this expectation finds no `{}` in its rule",
                code.name()
            ),
        }
    }
}

impl From<Finding> for Diagnostic {
    fn from(finding: Finding) -> Self {
        Self {
            severity: finding.kind.severity(),
            message: finding.kind.to_string(),
            help: Some(finding.kind.help()),
            span: finding.span,
            labels: finding.labels,
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
            source: "s = a; a = b, 'x' | 'y'; b = a, 'z' | 'w';",
            code: FindingCode::IndirectLeftRecursion,
            severity: Severity::Error,
            message: "`a` can reach itself through `b` before reading \
                      anything, so its parser would never end",
            help: "replace `b` in `a` with its definition until the recursion \
                   is direct, then rewrite it as a repetition",
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
            source: "a = {['x']};",
            code: FindingCode::NullableRepeatedSequence,
            severity: Severity::Error,
            message: "this repeated sequence can match nothing, so it could \
                      repeat forever",
            help: "make what repeats read something, such as by removing \
                   `[...]` inside the `{...}`",
        },
        Case {
            source: "a = ? U+0061..U+0063 ? | ? U+0062..U+0064 ?;",
            code: FindingCode::TiedBranches,
            severity: Severity::Warning,
            message: "these branches can both match 'b'..='c', so the parser \
                      takes the one written first",
            help: "exclude what they share from one of them with `-`, such as \
                   `identifier - keyword`; if this is intended, write `(* \
                   expect tied-branches: reason *)` right before the rule, \
                   and the parser takes the branch that matches the most, and \
                   the first written of those that tie",
        },
        Case {
            source: "s = ('x' | 'x', 'y'), 'y';",
            code: FindingCode::LongerBranchConflict,
            severity: Severity::Warning,
            message: "this branch goes on from where another ends with 'y', \
                      which can also come after the choice, so a parser \
                      cannot tell which to take",
            help: "make what comes after the choice start differently from \
                   how this branch goes on; if this is intended, write `(* \
                   expect longer-branch-conflict: reason *)` right before the \
                   rule, and the parser takes the longer branch whenever it \
                   matches",
        },
        Case {
            source: "a = ['xy'], 'x';",
            code: FindingCode::OptionalFollowConflict,
            severity: Severity::Warning,
            message: "this optional sequence and what comes after it can both \
                      start with 'x', so a parser cannot tell whether to take \
                      it",
            help: "make what comes after the sequence start differently; if \
                   this is intended, write `(* expect \
                   optional-follow-conflict: reason *)` right before the \
                   rule, and the parser takes the sequence whenever it \
                   matches, as usual for a dangling else",
        },
        Case {
            source: "a = {'xy'}, 'x';",
            code: FindingCode::RepeatedFollowConflict,
            severity: Severity::Warning,
            message: "this repeated sequence and what comes after it can both \
                      start with 'x', so a parser cannot tell whether to \
                      repeat it again",
            help: "end the repetition with something it cannot start with, or \
                   make what comes after it start differently; if this is \
                   intended, write `(* expect repeated-follow-conflict: \
                   reason *)` right before the rule, and the parser repeats \
                   the sequence as long as it matches, so it extends as far \
                   as it can",
        },
        Case {
            source: "a = ['x'], 'x';",
            code: FindingCode::RequiredOptionalSequence,
            severity: Severity::Warning,
            message: "this optional sequence takes every character that can \
                      come after it, so skipping it never leads to a match",
            help: "remove the `[...]` if the sequence is always needed, or \
                   make what comes after it start with a character it cannot \
                   take; if this is intended, write `(* expect \
                   required-optional-sequence: reason *)` right before the \
                   rule, and the grammar stays as it is",
        },
        Case {
            source: "a = {'x'}, 'x';",
            code: FindingCode::DeadEndRepeatedSequence,
            severity: Severity::Warning,
            message: "this repeated sequence takes every character that can \
                      come after it, so nothing after it can ever match",
            help: "move what comes after the repetition before it, so `{'x'}, \
                   'x'` becomes `'x', {'x'}`, or make what comes after it \
                   start with a character it cannot take; if this is \
                   intended, write `(* expect dead-end-repeated-sequence: \
                   reason *)` right before the rule, and the grammar stays as \
                   it is",
        },
        Case {
            source: "a = ['x'] | ['y'];",
            code: FindingCode::AmbiguousNullableBranches,
            severity: Severity::Warning,
            message: "more than one branch can match nothing",
            help: "make all but one of these branches read something, such as \
                   by removing `[...]` around one of them; if this is \
                   intended, write `(* expect ambiguous-nullable-branches: \
                   reason *)` right before the rule, and the parser takes the \
                   branch that matches the most, and the first written of \
                   those that tie",
        },
        Case {
            source: "s = 'x'; a = 'y', [b]; b = 'z', a;",
            code: FindingCode::UnreachableRule,
            severity: Severity::Warning,
            message: "`a` cannot be reached from any start rule, so no parser \
                      uses it",
            help: "remove `a`, or reference it from a rule a parser uses; if \
                   this is intended, write `(* expect unreachable-rule: \
                   reason *)` right before the rule, and the grammar stays as \
                   it is",
        },
        Case {
            source: "a = 'x' | 'x';",
            code: FindingCode::DuplicateAlternative,
            severity: Severity::Warning,
            message: "this alternative is written the same way as an earlier \
                      one, so both always match the same input",
            help: "remove this alternative; if this is intended, write `(* \
                   expect duplicate-alternative: reason *)` right before the \
                   rule, and the parser takes the branch that matches the \
                   most, and the first written of those that tie",
        },
        Case {
            source: "a = 'x' - 'y';",
            code: FindingCode::IneffectiveException,
            severity: Severity::Warning,
            message: "this exception can never apply, since nothing before \
                      the `-` matches the same input",
            help: "remove the exception, or write the one that was meant; if \
                   this is intended, write `(* expect ineffective-exception: \
                   reason *)` right before the rule, and the grammar stays as \
                   it is",
        },
        Case {
            source: "s = a, 'x'; a = ;",
            code: FindingCode::EmptyRule,
            severity: Severity::Warning,
            message: "`a` can only match nothing, so it never reads any input",
            help: "give `a` something to read, or remove it and every \
                   reference to it; if this is intended, write `(* expect \
                   empty-rule: reason *)` right before the rule, and the \
                   grammar stays as it is",
        },
        Case {
            source: "a = 'x' - ? U+0061..U+007A ?;",
            code: FindingCode::ExhaustiveException,
            severity: Severity::Warning,
            message: "this term can never match, since the exception matches \
                      everything before the `-` does",
            help: "remove this term, or narrow the exception so something is \
                   left; if this is intended, write `(* expect \
                   exhaustive-exception: reason *)` right before the rule, \
                   and the grammar stays as it is",
        },
        Case {
            source: "s = digit, Digit; digit = '0'; Digit = '1';",
            code: FindingCode::CaseOnlyNames,
            severity: Severity::Warning,
            message: "`Digit` differs from `digit` only in case, which is \
                      easy to mistake",
            help: "rename `Digit` or `digit`, or merge them if they mean the \
                   same; if this is intended, write `(* expect \
                   case-only-names: reason *)` right before the rule, and the \
                   grammar stays as it is",
        },
        Case {
            source: "(* expect no-such-finding: none *) a = 'x';",
            code: FindingCode::UnknownExpectation,
            severity: Severity::Error,
            message: "`no-such-finding` is not the code of any finding",
            help: "write the code a finding reports, such as \
                   `repeated-follow-conflict`",
        },
        Case {
            source: "(* expect left-recursion: wanted *) a = a, 'x' | 'y';",
            code: FindingCode::UnexpectableFinding,
            severity: Severity::Error,
            message: "`left-recursion` is an error, so it cannot be expected",
            help: "fix the grammar instead, since no parser can work with an \
                   error in it",
        },
        Case {
            source: "(* expect empty-rule: kept *) a = 'x';",
            code: FindingCode::UnfulfilledExpectation,
            severity: Severity::Warning,
            message: "this expectation finds no `empty-rule` in its rule",
            help: "remove the expectation, since the rule no longer has that \
                   finding",
        },
    ];

    #[test]
    fn has_a_case_for_every_code() {
        for &code in FindingCode::ALL {
            assert!(
                CASES.iter().any(|case| case.code == code),
                "no case for {}",
                code.name()
            );
        }
    }

    #[test]
    fn names_every_code_once_in_kebab_case() {
        for &code in FindingCode::ALL {
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
            assert_eq!(
                finding.kind.to_string(),
                case.message,
                "{:?}",
                case.source
            );
            assert_eq!(finding.kind.help(), case.help, "{:?}", case.source);
        }
    }

    #[test]
    fn turns_a_finding_into_a_diagnostic() {
        let finding =
            crate::analysis::tests::findings("a = ['x'] | ['y'];").remove(0);
        assert!(!finding.labels.is_empty());
        assert_eq!(
            Diagnostic::from(finding.clone()),
            Diagnostic {
                severity: finding.kind.severity(),
                message: finding.kind.to_string(),
                span: finding.span,
                labels: finding.labels,
                help: Some(finding.kind.help()),
            }
        );
    }
}
