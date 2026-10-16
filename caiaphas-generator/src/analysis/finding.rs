//! Findings are what the analysis reports about a grammar.

mod code;
mod message;

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
    TiedBranches {
        matched: String,
        /// How to write the later branch without what the earlier matches,
        /// when both are rules or terminal strings.
        exclusion: Option<String>,
    },
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
    /// A special sequence goes from a letter or digit to one of another kind,
    /// so it also takes the characters between the kinds.
    MixedClassRange {
        first: char,
        last: char,
        between: CharacterSet,
    },
    /// A `(* greedy *)` or `(* first match *)` changes nothing in its rule.
    IneffectiveAnnotation {
        meta_identifier: MetaIdentifier,
        /// The annotation, `greedy` or `first match`.
        annotation: &'static str,
    },
    /// A visible rule only ever matches one character and a repetition reads
    /// it, so every character it matches builds a value of its own.
    SingleCharacterRule { meta_identifier: MetaIdentifier },
    /// An alternative starts the same way as an earlier one of the same
    /// choice, so a parser reads that start twice.
    SharedStart,
    /// A parser cannot tell from what comes next which way a choice takes, so
    /// its rule tries every way.
    NondeterministicChoice {
        meta_identifier: MetaIdentifier,
        characters: CharacterSet,
    },
    /// An expectation names no kind of finding, though maybe a misspelling of
    /// the `similar` one.
    UnknownExpectation {
        code: String,
        similar: Option<FindingCode>,
    },
    /// An expectation names a kind of finding that cannot be intended, such as
    /// an error.
    UnexpectableFinding { code: FindingCode },
    /// An expectation finds no finding of its kind in its rule.
    UnfulfilledExpectation { code: FindingCode },
}

impl Finding {
    /// Returns whether rearranging the grammar takes care of the finding, so a
    /// module generated with the grammar rearranged leaves it out.
    pub(crate) fn is_rearranged_away(&self) -> bool {
        self.kind == FindingKind::SharedStart
    }
}

impl FindingKind {
    pub(crate) fn severity(&self) -> Severity {
        self.code().severity()
    }

    /// Returns how to fix the grammar.
    pub(crate) fn help(&self) -> String {
        let mut remedy = self.remedy();
        if self.code().is_greedy_only() {
            remedy.push_str(
                ", or remove `(* greedy *)` so the parser tries every way",
            );
        }
        remedy
    }

    /// Returns how to mark the finding as intended, when it can be.
    pub(crate) fn note(&self) -> Option<String> {
        let code = self.code();
        if !code.is_expectable() {
            return None;
        }
        let expect = format!(
            "if this is intended, write `(* expect {}: reason *)` right \
             before the rule",
            code.name()
        );
        Some(match code.intended_behaviour() {
            Some(behaviour) => format!("{expect}, and {behaviour}"),
            None => expect,
        })
    }
}

impl From<Finding> for Diagnostic {
    fn from(finding: Finding) -> Self {
        Self {
            severity: finding.kind.severity(),
            code: Some(finding.kind.code().name()),
            message: finding.kind.to_string(),
            help: Some(finding.kind.help()),
            note: finding.kind.note(),
            span: finding.span,
            labels: finding.labels,
        }
    }
}

#[cfg(test)]
mod tests;
