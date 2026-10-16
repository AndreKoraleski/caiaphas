//! Codes name the kinds of findings, so an expectation can name the kind it
//! marks as intended.

use crate::{analysis::finding::FindingKind, diagnostic::Severity};

/// Defines [`FindingCode`], its [`ALL`](FindingCode::ALL), the name of each
/// code and the code of each [`FindingKind`] from one list, so `ALL` cannot
/// miss a code. Each code has the name of its kind.
macro_rules! finding_codes {
    ($($code:ident => $name:literal,)*) => {
        /// The kind of a finding without its details. An expectation names it
        /// in kebab case.
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

        impl FindingKind {
            pub(crate) fn code(&self) -> FindingCode {
                match self {
                    $(Self::$code { .. } => FindingCode::$code,)*
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
    UnreachableAlternative => "unreachable-alternative",
    DuplicateAlternative => "duplicate-alternative",
    IneffectiveException => "ineffective-exception",
    EmptyRule => "empty-rule",
    ExhaustiveException => "exhaustive-exception",
    CaseOnlyNames => "case-only-names",
    MixedClassRange => "mixed-class-range",
    IneffectiveAnnotation => "ineffective-annotation",
    SingleCharacterRule => "single-character-rule",
    SharedStart => "shared-start",
    NondeterministicChoice => "nondeterministic-choice",
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
            | Self::UnreachableAlternative
            | Self::DuplicateAlternative
            | Self::IneffectiveException
            | Self::EmptyRule
            | Self::ExhaustiveException
            | Self::CaseOnlyNames
            | Self::MixedClassRange
            | Self::IneffectiveAnnotation
            | Self::UnfulfilledExpectation => Severity::Warning,
            Self::SingleCharacterRule
            | Self::SharedStart
            | Self::NondeterministicChoice => Severity::Suggestion,
        }
    }

    /// Returns whether only a greedy rule has findings with this code, since
    /// every other rule tries every way through its choices.
    pub(crate) fn is_greedy_only(self) -> bool {
        matches!(
            self,
            Self::LongerBranchConflict
                | Self::OptionalFollowConflict
                | Self::RepeatedFollowConflict
                | Self::RequiredOptionalSequence
                | Self::DeadEndRepeatedSequence
        )
    }

    /// Returns whether an expectation can mark a finding with this code as
    /// intended. Errors and findings about expectations cannot be.
    pub(crate) fn is_expectable(self) -> bool {
        self.severity() != Severity::Error
            && self != Self::UnfulfilledExpectation
    }

    /// Returns what the parser does when an expectation marks a finding with
    /// this code as intended, if that needs saying.
    pub(super) fn intended_behaviour(self) -> Option<&'static str> {
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
            Self::NondeterministicChoice => {
                Some("the parser still tries every way")
            }
            Self::RequiredOptionalSequence
            | Self::DeadEndRepeatedSequence
            | Self::UnreachableRule
            | Self::UnreachableAlternative
            | Self::IneffectiveException
            | Self::EmptyRule
            | Self::ExhaustiveException
            | Self::CaseOnlyNames
            | Self::MixedClassRange
            | Self::IneffectiveAnnotation
            | Self::SingleCharacterRule
            | Self::SharedStart
            | Self::LeftRecursion
            | Self::IndirectLeftRecursion
            | Self::UnproductiveRule
            | Self::NullableRepeatedSequence
            | Self::UnknownExpectation
            | Self::UnexpectableFinding
            | Self::UnfulfilledExpectation => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn explains_every_code_and_nothing_else() {
        for &code in FindingCode::ALL {
            let explanation = crate::explain(code.name());
            assert!(
                explanation.is_some_and(|text| !text.is_empty()),
                "{}",
                code.name()
            );
        }
        let explained = include_str!("../../findings.md")
            .lines()
            .filter_map(|line| line.strip_prefix("## "));
        for name in explained {
            assert!(FindingCode::from_name(name).is_some(), "{name}");
        }
        assert_eq!(crate::explain("no-such-code"), None);
    }
}
