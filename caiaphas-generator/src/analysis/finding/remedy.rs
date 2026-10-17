//! Each kind of finding tells the usual way to fix it. Kinds of one family
//! build their remedies from the same template, so they stay alike.

use crate::{analysis::finding::FindingKind, diagnostic::Severity};

impl FindingKind {
    /// Returns the usual remedy for this kind of problem.
    pub(super) fn remedy(&self) -> String {
        match self {
            Self::LeftRecursion { meta_identifier } => {
                let name = meta_identifier.spaced();
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
                through.spaced(),
                meta_identifier.spaced()
            ),
            Self::UnproductiveRule { meta_identifier } => format!(
                "give `{}` an alternative that only reads characters, or make \
                 the recursive part optional with `[...]` or `{{...}}`",
                meta_identifier.spaced()
            ),
            Self::NullableRepeatedSequence => {
                "make what repeats always read something, as `{'x'}` does \
                 where `{['x']}` does not"
                    .to_owned()
            }
            Self::TiedBranches {
                exclusion: Some(exclusion),
                ..
            } => format!(
                "exclude what they share from one of them with `-`, such as \
                 `{exclusion}`"
            ),
            Self::TiedBranches {
                exclusion: None, ..
            } => "exclude what they share from one of them with `-`, such as \
                  `identifier - keyword`"
                .to_owned(),
            Self::LongerBranchConflict { .. } => format!(
                "{} from how this branch goes on",
                start_differently("the choice")
            ),
            Self::OptionalFollowConflict { .. } => {
                start_differently("the sequence")
            }
            Self::RepeatedFollowConflict { .. } => format!(
                "end the repetition with something it cannot start with, or \
                 {}",
                start_differently("it")
            ),
            Self::RequiredOptionalSequence => cannot_take(
                "remove the `[...]` if the sequence is always needed",
            ),
            Self::DeadEndRepeatedSequence => cannot_take(
                "move what comes after the repetition before it, so `{'x'}, \
                 'x'` becomes `'x', {'x'}`",
            ),
            Self::AmbiguousSplit {
                repeated: false, ..
            } => {
                "write the sequence so its parts can only meet at one place, \
                  as `{'x'}` does where `{'x'}, ['x']` does not"
                    .to_owned()
            }
            Self::AmbiguousSplit { repeated: true, .. } => {
                "make each round end at one place, as `{'a'}` does where \
                 `{'a' | 'aa'}` does not"
                    .to_owned()
            }
            Self::AmbiguousNullableBranches => {
                "make all but one of these branches read something, such as \
                 by removing `[...]` around one of them"
                    .to_owned()
            }
            Self::UnreachableRule { meta_identifier } => format!(
                "remove `{}`, or reference it from a rule a parser uses",
                meta_identifier.spaced()
            ),
            Self::UnreachableAlternative => {
                "move it before the alternative that matches first, or remove \
                 it"
                .to_owned()
            }
            Self::DuplicateAlternative => "remove this alternative".to_owned(),
            Self::IneffectiveException { longer: true } => {
                "what comes before the `-` matches one character at a time, \
                 so it never matches the longer exception; to end a \
                 repetition at that text, take it out of the whole \
                 repetition, as in `{c} - ({c}, '*/', {c})`"
                    .to_owned()
            }
            Self::IneffectiveException { longer: false } => {
                "remove the exception, or write the one that was meant"
                    .to_owned()
            }
            Self::EmptyRule { meta_identifier } => format!(
                "give `{}` something to read, or remove it and every \
                 reference to it",
                meta_identifier.spaced()
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
                meta_identifier.spaced(),
                other.spaced()
            ),
            Self::MixedClassRange { first, last, .. } => {
                let kinds = [('0', '9'), ('A', 'Z'), ('a', 'z')];
                let parts: Vec<String> = kinds
                    .iter()
                    .map(|&(start, end)| (start.max(*first), end.min(*last)))
                    .filter(|(start, end)| start <= end)
                    .map(|(start, end)| {
                        format!(
                            "? U+{:04X}..U+{:04X} ?",
                            u32::from(start),
                            u32::from(end)
                        )
                    })
                    .collect();
                format!(
                    "split it into a range for each kind of character, as \
                     `{}`",
                    parts.join(" | ")
                )
            }
            Self::IneffectiveAnnotation { .. } => {
                "remove the `(* greedy *)`".to_owned()
            }
            Self::SingleCharacterRule { meta_identifier } => format!(
                "hide it with `(* hide *)` unless a type needs `{}`, since \
                 the rules that read it keep its text",
                meta_identifier.spaced()
            ),
            Self::NondeterministicChoice {
                meta_identifier,
                greedy: Some(codes),
                ..
            } => {
                let annotations: Vec<String> =
                    std::iter::once("`(* greedy *)`".to_owned())
                        .chain(codes.iter().map(|code| {
                            format!("`(* expect {}: reason *)`", code.name())
                        }))
                        .collect();
                format!(
                    "write {} right before `{}` if it should take each way as \
                     it comes, which keeps its parser fast, or {}",
                    annotations.join(" and "),
                    meta_identifier.spaced(),
                    start_differently("the choice")
                )
            }
            Self::NondeterministicChoice {
                meta_identifier,
                greedy: None,
                ..
            } => format!(
                "{} if `{}` should be fast, since taking each way as it comes \
                 would make it miss what it matches",
                start_differently("the choice"),
                meta_identifier.spaced()
            ),
            Self::UnknownExpectation {
                similar: Some(similar),
                ..
            } => format!("correct its spelling to `{}`", similar.name()),
            Self::UnknownExpectation { similar: None, .. } => {
                "write the code a finding reports, such as `tied-branches`; \
                 `caiaphas explain` lists every code"
                    .to_owned()
            }
            Self::UnexpectableFinding { code }
                if code.severity() == Severity::Error =>
            {
                "fix the grammar instead, since no parser can work with an \
                 error in it"
                    .to_owned()
            }
            Self::UnexpectableFinding { .. } => {
                "remove the expectation, since a finding about expectations \
                 only goes away with the expectation that has it"
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

/// Returns the remedy of making what comes after `after` start differently.
fn start_differently(after: &str) -> String {
    format!("make what comes after {after} start differently")
}

/// Returns `remedy`, or the remedy of making what comes after a sequence start
/// with a character the sequence cannot take.
fn cannot_take(remedy: &str) -> String {
    format!(
        "{remedy}, or make what comes after it start with a character it \
         cannot take"
    )
}
