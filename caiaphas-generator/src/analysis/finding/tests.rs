//! Tests that every kind of finding has its code, message and help.

use super::*;

/// A grammar with a finding of one kind, how serious it is, its message and
/// how to fix it.
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
        source: "(* greedy *) s = ('x' | 'x', 'y'), 'y';",
        code: FindingCode::LongerBranchConflict,
        severity: Severity::Warning,
        message: "this branch goes on from where another ends with 'y', \
                  which can also come after the choice, so a parser \
                  cannot tell which to take",
        help: "make what comes after the choice start differently from \
               how this branch goes on, or remove `(* greedy *)` so the \
               parser tries every way; if this is intended, write `(* \
               expect longer-branch-conflict: reason *)` right before the \
               rule, and the parser takes the longer branch whenever it \
               matches",
    },
    Case {
        source: "(* greedy *) a = ['xy'], 'x';",
        code: FindingCode::OptionalFollowConflict,
        severity: Severity::Warning,
        message: "this optional sequence and what comes after it can both \
                  start with 'x', so a parser cannot tell whether to take \
                  it",
        help: "make what comes after the sequence start differently, or \
               remove `(* greedy *)` so the parser tries every way; if \
               this is intended, write `(* expect \
               optional-follow-conflict: reason *)` right before the \
               rule, and the parser takes the sequence whenever it \
               matches, as usual for a dangling else",
    },
    Case {
        source: "(* greedy *) a = {'xy'}, 'x';",
        code: FindingCode::RepeatedFollowConflict,
        severity: Severity::Warning,
        message: "this repeated sequence and what comes after it can both \
                  start with 'x', so a parser cannot tell whether to \
                  repeat it again",
        help: "end the repetition with something it cannot start with, or \
               make what comes after it start differently, or remove `(* \
               greedy *)` so the parser tries every way; if this is \
               intended, write `(* expect repeated-follow-conflict: \
               reason *)` right before the rule, and the parser repeats \
               the sequence as long as it matches, so it extends as far \
               as it can",
    },
    Case {
        source: "(* greedy *) a = ['x'], 'x';",
        code: FindingCode::RequiredOptionalSequence,
        severity: Severity::Warning,
        message: "this optional sequence takes every character that can \
                  come after it, so skipping it never leads to a match",
        help: "remove the `[...]` if the sequence is always needed, or \
               make what comes after it start with a character it cannot \
               take, or remove `(* greedy *)` so the parser tries every \
               way; if this is intended, write `(* expect \
               required-optional-sequence: reason *)` right before the \
               rule, and the grammar stays as it is",
    },
    Case {
        source: "(* greedy *) a = {'x'}, 'x';",
        code: FindingCode::DeadEndRepeatedSequence,
        severity: Severity::Warning,
        message: "this repeated sequence takes every character that can \
                  come after it, so nothing after it can ever match",
        help: "move what comes after the repetition before it, so `{'x'}, \
               'x'` becomes `'x', {'x'}`, or make what comes after it \
               start with a character it cannot take, or remove `(* \
               greedy *)` so the parser tries every way; if this is \
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
        source: "(* first match *) a = 'x' | 'x';",
        code: FindingCode::UnreachableAlternative,
        severity: Severity::Warning,
        message: "an earlier alternative matches wherever this one would, \
                  so the parser never takes this one",
        help: "move it before the alternative that matches first, or \
               remove it; if this is intended, write `(* expect \
               unreachable-alternative: reason *)` right before the rule, \
               and the grammar stays as it is",
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
        source: "s = ? U+0041..U+007A ?;",
        code: FindingCode::MixedClassRange,
        severity: Severity::Warning,
        message: "this range from `A` to `z` also takes '['..='`', though \
                  they are neither letters nor digits",
        help: "split it into a range for each kind of character, such as `? \
               U+0041..U+005A ? | ? U+0061..U+007A ?` for the letters; if \
               this is intended, write `(* expect mixed-class-range: reason \
               *)` right before the rule, and the grammar stays as it is",
    },
    Case {
        source: "(* greedy *) s = {'x'}, 'y';",
        code: FindingCode::IneffectiveAnnotation,
        severity: Severity::Warning,
        message: "`(* greedy *)` changes nothing in `s`, since a parser can \
                  tell each way through it from the next character",
        help: "remove the `(* greedy *)`; if this is intended, write `(* \
               expect ineffective-annotation: reason *)` right before the \
               rule, and the grammar stays as it is",
    },
    Case {
        source: "number = digit, {digit}; digit = ? U+0030..U+0039 ?;",
        code: FindingCode::SingleCharacterRule,
        severity: Severity::Suggestion,
        message: "`digit` only ever matches one character, and a repetition \
                  reads it, so every character it matches builds a value of \
                  its own",
        help: "hide it with `(* hide *)` unless a type needs `digit`, since \
               the rules that read it keep its text; if this is intended, \
               write `(* expect single-character-rule: reason *)` right \
               before the rule, and the grammar stays as it is",
    },
    Case {
        source: "s = a, 'x' | a, 'y'; a = 'z';",
        code: FindingCode::SharedStart,
        severity: Severity::Suggestion,
        message: "this alternative starts the same way as an earlier one, so \
                  a parser reads that start twice",
        help: "write the shared start once, before a choice of what follows \
               it, so `a, x | a, y` becomes `a, (x | y)`; if this is \
               intended, write `(* expect shared-start: reason *)` right \
               before the rule, and the grammar stays as it is",
    },
    Case {
        source: "s = {'x'}, 'x';",
        code: FindingCode::NondeterministicChoice,
        severity: Severity::Suggestion,
        message: "a parser cannot tell from 'x' which way to take here, so \
                  `s` and the rules that reach it try every way",
        help: "write `(* greedy *)` right before `s` if it should take each \
               way as it comes, which keeps its parser fast, or make what \
               comes after the choice start differently; if this is \
               intended, write `(* expect nondeterministic-choice: reason \
               *)` right before the rule, and the parser still tries every \
               way",
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
fn describes_every_kind_of_finding() {
    for case in CASES {
        let finding = crate::analysis::tests::findings(case.source)
            .into_iter()
            .find(|finding| finding.kind.code() == case.code)
            .unwrap_or_else(|| panic!("no such finding in {:?}", case.source));
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
