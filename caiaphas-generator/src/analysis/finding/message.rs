//! Each kind of finding says what is wrong and how to fix it. Kinds of one
//! family build their sentences from the same template, so they stay alike.

use std::fmt::{self, Display};

use crate::{analysis::finding::FindingKind, diagnostic::Severity};

impl FindingKind {
    /// Returns the usual remedy for this kind of problem.
    pub(super) fn remedy(&self) -> String {
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
                "make what repeats always read something, as `{'x'}` does \
                 where `{['x']}` does not"
                    .to_owned()
            }
            Self::TiedBranches { .. } => {
                "exclude what they share from one of them with `-`, such as \
                 `identifier - keyword`"
                    .to_owned()
            }
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
            Self::MixedClassRange { .. } => {
                "split it into a range for each kind of character, such as \
                 `? U+0041..U+005A ? | ? U+0061..U+007A ?` for the letters"
                    .to_owned()
            }
            Self::IneffectiveAnnotation { annotation, .. } => {
                format!("remove the `(* {annotation} *)`")
            }
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
            Self::NondeterministicChoice {
                meta_identifier, ..
            } => format!(
                "write `(* greedy *)` right before `{}` if it should take \
                 each way as it comes, which keeps its parser fast, or {}",
                meta_identifier.as_str(),
                start_differently("the choice")
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

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftRecursion { meta_identifier } => {
                f.write_str(&reaches_itself(meta_identifier.as_str(), ""))
            }
            Self::IndirectLeftRecursion {
                meta_identifier,
                through,
            } => f.write_str(&reaches_itself(
                meta_identifier.as_str(),
                &format!(" through `{}`", through.as_str()),
            )),
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
            Self::LongerBranchConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this branch, past where another ends,",
                    "the choice",
                    characters,
                    "which to take",
                ))
            }
            Self::OptionalFollowConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this optional sequence",
                    "it",
                    characters,
                    "whether to take it",
                ))
            }
            Self::RepeatedFollowConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this repeated sequence",
                    "it",
                    characters,
                    "whether to repeat it again",
                ))
            }
            Self::RequiredOptionalSequence => f.write_str(&takes_every(
                "optional",
                "skipping it never leads to a match",
            )),
            Self::DeadEndRepeatedSequence => f.write_str(&takes_every(
                "repeated",
                "nothing after it can ever match",
            )),
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
                "an earlier alternative matches wherever this one would, so \
                 the parser never takes this one",
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
            Self::MixedClassRange {
                first,
                last,
                between,
            } => write!(
                f,
                "this range from `{first}` to `{last}` also takes {between}, \
                 though they are neither letters nor digits"
            ),
            Self::IneffectiveAnnotation {
                meta_identifier,
                annotation,
            } => {
                let name = meta_identifier.as_str();
                let reason = if *annotation == "greedy" {
                    "a parser can tell each way through it from the next \
                     character"
                } else {
                    "no two of its alternatives can match at the same place"
                };
                write!(
                    f,
                    "`(* {annotation} *)` changes nothing in `{name}`, since \
                     {reason}"
                )
            }
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
            Self::NondeterministicChoice {
                meta_identifier,
                characters,
            } => write!(
                f,
                "a parser cannot tell from {characters} which way to take \
                 here, so `{}` and the rules that reach it try every way",
                meta_identifier.as_str()
            ),
            Self::UnknownExpectation { code, .. } => {
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

/// Says that `rule` can reach itself before reading anything, and `how` when
/// that needs saying.
fn reaches_itself(rule: &str, how: &str) -> String {
    format!(
        "`{rule}` can reach itself{how} before reading anything, so its \
         parser would never end"
    )
}

/// Says that `subject` and what comes after `after` can both start with
/// `characters`, so a parser cannot tell `what`.
fn cannot_tell(
    subject: &str,
    after: &str,
    characters: &impl Display,
    what: &str,
) -> String {
    format!(
        "{subject} and what comes after {after} can both start with \
         {characters}, so a parser cannot tell {what}"
    )
}

/// Says that a `kind` sequence takes every character that can come after it,
/// so `outcome`.
fn takes_every(kind: &str, outcome: &str) -> String {
    format!(
        "this {kind} sequence takes every character that can come after it, \
         so {outcome}"
    )
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
