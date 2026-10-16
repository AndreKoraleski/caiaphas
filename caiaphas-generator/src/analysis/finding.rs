//! Findings are what the analysis reports about a grammar.

mod code;

use std::fmt;

use crate::{
    analysis::character_set::CharacterSet,
    diagnostic::{Diagnostic, Label, Severity},
    lexer::MetaIdentifier,
    span::Span,
};

pub(crate) use code::FindingCode;

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
    /// An alternative of a rule that takes the first match comes after one
    /// that matches wherever it would, so a parser never takes it.
    UnreachableAlternative,
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
    /// A visible rule only ever matches one character and a repetition reads
    /// it, so every character it matches builds a value of its own.
    SingleCharacterRule { meta_identifier: MetaIdentifier },
    /// An alternative starts the same way as an earlier one of the same
    /// choice, so a parser reads that start twice.
    SharedStart,
    /// An expectation names no kind of finding.
    UnknownExpectation { code: String },
    /// An expectation names a kind of finding that cannot be intended, such as
    /// an error.
    UnexpectableFinding { code: FindingCode },
    /// An expectation finds no finding of its kind in its rule.
    UnfulfilledExpectation { code: FindingCode },
}

impl FindingKind {
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
            Self::UnreachableAlternative => {
                "move it before the alternative that matches first, or remove \
                 it"
                .to_owned()
            }
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
            Self::SingleCharacterRule { meta_identifier } => format!(
                "hide it with `(* hide *)` unless a type needs `{}`, since \
                 the rules that read it keep its text",
                meta_identifier.as_str()
            ),
            Self::SharedStart => {
                "write the shared start once, before a choice of what follows \
                 it, so `a, x | a, y` becomes `a, (x | y)`"
                    .to_owned()
            }
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
            Self::UnreachableAlternative => f.write_str(
                "an earlier alternative matches wherever this one would, so a \
                 parser that takes the first match never takes this one",
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
            Self::SingleCharacterRule { meta_identifier } => write!(
                f,
                "`{}` only ever matches one character, and a repetition reads \
                 it, so every character it matches builds a value of its own",
                meta_identifier.as_str()
            ),
            Self::SharedStart => f.write_str(
                "this alternative starts the same way as an earlier one, so a \
                 parser reads that start twice",
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
mod tests;
