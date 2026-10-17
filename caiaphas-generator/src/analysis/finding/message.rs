//! Each kind of finding says what is wrong. Kinds of one family build their
//! sentences from the same template, so they stay alike.

use std::fmt::{self, Display};

use crate::{analysis::finding::FindingKind, diagnostic::Severity};

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftRecursion {
                meta_identifier, ..
            } => f.write_str(&reaches_itself(&meta_identifier.spaced(), "")),
            Self::IndirectLeftRecursion {
                meta_identifier,
                through,
            } => f.write_str(&reaches_itself(
                &meta_identifier.spaced(),
                &format!(" through `{}`", through.spaced()),
            )),
            Self::UnproductiveRule { meta_identifier } => write!(
                f,
                "`{}` can never finish matching, since every way through it \
                 needs a rule that cannot either",
                meta_identifier.spaced()
            ),
            Self::NullableRepeatedSequence => f.write_str(
                "a round of this repeated sequence can match nothing, and \
                 such a round ends the repetition",
            ),
            Self::TiedBranches { matched, .. } => write!(
                f,
                "these branches can both match {matched}, so the parser takes \
                 the one written first"
            ),
            Self::LongerBranchConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this branch, past where another ends,",
                    "the choice",
                    &characters.described(),
                    "which to take",
                ))
            }
            Self::OptionalFollowConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this optional sequence",
                    "it",
                    &characters.described(),
                    "whether to take it",
                ))
            }
            Self::RepeatedFollowConflict { characters } => {
                f.write_str(&cannot_tell(
                    "this repeated sequence",
                    "it",
                    &characters.described(),
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
            Self::AmbiguousSplit {
                matched,
                repeated: false,
            } => write!(
                f,
                "this sequence can split {matched} between its parts in more \
                 than one way, so the parser gives each part the most it can, \
                 from the first"
            ),
            Self::AmbiguousSplit {
                matched,
                repeated: true,
            } => write!(
                f,
                "this repetition can cut {matched} into rounds in more than \
                 one way, so the parser makes each round the most it can, \
                 from the first"
            ),
            Self::UnreachableRule { meta_identifier } => write!(
                f,
                "`{}` cannot be reached from any start rule, so no parser \
                 uses it",
                meta_identifier.spaced()
            ),
            Self::UnreachableAlternative => f.write_str(
                "an earlier alternative matches wherever this one would, so \
                 the parser never takes this one",
            ),
            Self::DuplicateAlternative => f.write_str(
                "this alternative is written the same way as an earlier one, \
                 so both always match the same input",
            ),
            Self::IneffectiveException { .. } => f.write_str(
                "this exception can never apply, since nothing before the `-` \
                 matches the same input",
            ),
            Self::EmptyRule { meta_identifier } => write!(
                f,
                "`{}` can only match nothing, so it never reads any input",
                meta_identifier.spaced()
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
                meta_identifier.spaced(),
                other.spaced()
            ),
            Self::MixedClassRange {
                first,
                last,
                between,
            } => write!(
                f,
                "this range from `{first}` to `{last}` also takes {}, \
                 though they are neither letters nor digits",
                between.described()
            ),
            Self::SeveralStartRules {
                meta_identifier,
                first,
            } => write!(
                f,
                "no rule uses `{}`, so it is a start rule besides `{}`",
                meta_identifier.spaced(),
                first.spaced()
            ),
            Self::MisspelledAnnotation { meant } => write!(
                f,
                "this comment reads like `(* {meant} *)` written wrongly"
            ),
            Self::IneffectiveAnnotation { meta_identifier } => write!(
                f,
                "`(* greedy *)` changes nothing in `{}`, since a parser can \
                 tell each way through it from the next character",
                meta_identifier.spaced()
            ),
            Self::SingleCharacterRule { meta_identifier } => write!(
                f,
                "`{}` only ever matches one character, and a repetition reads \
                 it, so every character it matches builds a value of its own",
                meta_identifier.spaced()
            ),
            Self::UntracedChoice { meta_identifier } => write!(
                f,
                "nothing in the fields of `{}` tells which of these \
                 alternatives matched",
                meta_identifier.spaced()
            ),
            Self::GatheredField {
                meta_identifier,
                held,
            } => write!(
                f,
                "these places hold `{}` a different number of times each, \
                 and one field of `{}` gathers every match",
                held.spaced(),
                meta_identifier.spaced()
            ),
            Self::NondeterministicChoice {
                meta_identifier,
                characters,
                ..
            } => write!(
                f,
                "a parser cannot tell from {} which way to take here, so `{}` \
                 and the rules that reach it try every way",
                characters.described(),
                meta_identifier.spaced()
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
