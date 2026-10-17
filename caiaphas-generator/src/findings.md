Every finding caiaphas can report about a grammar, by its code. A finding
shows its code in its title, as in `warning[tied-branches]`.

An error stops the build. A warning stops it too, unless an expectation right
before its rule marks it as intended, with the code and a reason:

```text
(* expect tied-branches: a keyword is never a name *)
word = keyword | name;
```

A suggestion does not stop the build unless `grammar!(..., suggest)` asks it
to, and an expectation can mark one as intended too. The docs of the generated
module list the suggestions, and `caiaphas check` prints them.

## left-recursion

Error. A rule can reach itself before reading anything, as `expression` does
in `expression = expression, '+', term | term;`, so its parser would never
end. Write the recursion as a repetition: `expression = term, {'+', term};`.

## indirect-left-recursion

Error. A rule can reach itself through other rules before reading anything, as
`a` does in `s = a; a = b, 'x' | 'y'; b = a, 'z';`. Replace the rule it goes
through with its definition until the recursion is direct, then write it as a
repetition. Each cycle is reported once, at the first of its rules.

## unproductive-rule

Error. No input can ever finish matching a rule, since every way through it
needs a rule that cannot either, as in `list = item, list;`. Give it an
alternative that only reads characters, or make the recursion optional:
`list = item, [list];`.

## nullable-repeated-sequence

Error. A repeated sequence can match nothing, as `{['x']}` can, so it could
repeat forever. Make what repeats read something: `{'x'}`.

## tied-branches

Warning. Two alternatives can match the same text, as `keyword` and `name`
both match `if` in `word = keyword | name;`. The first written of
alternatives that tie wins, so `if` is always a keyword there. Expect the
finding if that is meant, or take the shared text out of one with an
exception: `name - keyword`.

## longer-branch-conflict

Warning. In a greedy rule, an alternative goes on from where another ends with
a character that can also come after the choice, as `'x', 'y'` does in
`(* greedy *) s = ('x' | 'x', 'y'), 'y';`. The rule takes the longer one
whenever it can, so `xy` never matches. Remove `(* greedy *)` so the parser
tries every way, or make what comes after the choice start differently.

## optional-follow-conflict

Warning. In a greedy rule, an optional sequence can start with a character
that can also come after it, as the `else` of a nested `if` does in
`(* greedy *) s = 'if', c, s, ['else', s] | 'x';`. The rule takes the
sequence whenever it matches, as usual for a dangling else. Remove
`(* greedy *)` so the parser tries both ways, or make what comes after the
sequence start differently.

## repeated-follow-conflict

Warning. In a greedy rule, a repeated sequence can start with a character
that can also come after it, as in `(* greedy *) s = {'xy'}, 'x', 'z';`. The
rule repeats as long as it can, so it can take what the rest needed. Remove
`(* greedy *)` so the parser tries every way, or end the repetition with
something it cannot start with.

## required-optional-sequence

Warning. In a greedy rule, an optional sequence takes every character that can
come after it, as `['x']` does in `(* greedy *) s = ['x'], 'x';`, so skipping
it never leads to a match. Remove `(* greedy *)`, remove the brackets if the
sequence is always needed, or make what comes after it start with a character
it cannot take.

## dead-end-repeated-sequence

Warning. In a greedy rule, a repeated sequence takes every character that can
come after it, as `{'x'}` does in `(* greedy *) s = {'x'}, 'x';`, so nothing
after it can ever match. Remove `(* greedy *)`, or move what comes after the
repetition before it: `'x', {'x'}`.

## ambiguous-nullable-branches

Warning. More than one branch of a choice can match nothing, as both do in
`a = ['x'] | ['y'];`, so an empty match could come from either. Make all but
one of them read something.

## ambiguous-split

Warning. A sequence can split a text between its parts in more than one way,
as `table = record, {newline, record}, [newline];` can split a last line break
when a record can be empty, or a repetition can cut a text into rounds in
more than one way, as `{'a' | 'aa'}` can with `aa`. The parser gives each part
the most text it can while the rest still matches, which may not be the split
meant. Write the parts so they can only meet at one place, as
`table = {record, newline};` does, or expect the finding if that split is the
one meant.

## unreachable-rule

Warning. No start rule reaches a rule, so no parser uses it. In
`s = 'x'; a = b; b = 'y', [a];`, `a` and `b` only reach each other. Remove the
rule, or reference it from a rule a parser uses.

## unreachable-alternative

Warning. An earlier alternative takes every match a later one could have, as
`name` does for `'if'` in `word = name | 'if';`, so the later one is never
taken and its variant is never built. Move it before the earlier one, or
remove it.

## duplicate-alternative

Warning. An alternative is written the same way as an earlier one of the same
choice, as in `a = 'x' | 'x';`. Remove one of them.

## ineffective-exception

Warning. An exception can never apply, since nothing its factor matches is
also matched by it, as `'y'` in `a = 'x' - 'y';`. Remove it, or write the one
that was meant.

A common case is a comment that should end at `*/`, written
`'/*', {character - '*/'}, '*/'`. A character is never the two of `*/`, so
the repetition goes past it. Take the text out of the whole repetition
instead: `'/*', ({character} - ({character}, '*/', {character})), '*/'`.

## empty-rule

Warning. A rule can only match nothing, as in `a = ;`, so it never reads any
input. Give it something to read, or remove it and every reference to it.

## exhaustive-exception

Warning. An exception matches everything its factor matches, as in
`a = 'x' - 'x';`, so the term can never match. Remove the term, or narrow the
exception so something is left.

## case-only-names

Warning. Two rules have names that differ only in case, as `digit` and
`Digit` do, and those are easy to mistake. Rename one of them, or merge them
if they mean the same. When both have types, the types would share a name,
and that stops the build anyway.

## mixed-class-range

Warning. A special sequence goes from a letter or digit to one of another
kind, as `? U+0041..U+007A ?` goes from `A` to `z`, so it also takes the
characters between the kinds, here `[` to the backtick. Split it into a range
for each kind: `? U+0041..U+005A ? | ? U+0061..U+007A ?`.

## ineffective-annotation

Warning. A `(* greedy *)` changes nothing in its rule, since the next
character already decides each of its choices and it reaches no rule that
tries every way. Remove the annotation.

## misspelled-annotation

Warning. A comment of one word right before a rule reads like an annotation
written wrongly, as `(* hidde *)` does. To the standard it is a comment like
any other, so caiaphas reads the rule as if it were not there, but a typo in
an annotation changes the rule's type. Write the annotation as it is spelled,
as `(* hide *)`, or more words for an ordinary comment.

## several-start-rules

Suggestion. No rule uses a rule besides the first start rule, as no rule uses
`b` in `a = 'x'; b = 'y';`, so it is a start rule too. The standard has no
start symbol, so a grammar may have several, but a rule left over, or one
whose name is misspelled where it is used, looks the same. Use the rule where
it belongs, as `a = 'x', b; b = 'y';`, or remove it if it is left over.

## single-character-rule

Suggestion. A visible rule only ever matches one character and a repetition
reads it, as `digit` does in `number = digit, {digit};`, so every character it
matches builds a value of its own. Hide it with `(* hide *)` unless a type
needs it, since the rules that read it keep its text.

## nondeterministic-choice

Suggestion. A parser cannot tell from the next character which way a choice
goes, as with the `else` in
`statement = 'if', c, statement, ['else', statement] | 'x';`, so the rule and
the rules that reach it try every way and run slower. If the rule should take
each way as it comes, write `(* greedy *)` right before it, with an
expectation of the follow conflict that brings, as the help says. Otherwise
make what comes after the choice start differently. Where taking each way as
it comes would miss what the rule matches, as in `{'x'}, 'x'`, the help leaves
greedy out.

## unknown-expectation

Error. An expectation names no kind of finding, as
`(* expect no-such-finding: reason *)` does. Write one of the codes here.

## unexpectable-finding

Error. An expectation names a finding no expectation can mark as intended: an
error, since no parser can work with one, or a finding about expectations.
Fix the grammar instead.

## unfulfilled-expectation

Warning. An expectation finds no finding of its kind in its rule, so it marks
nothing. Remove it.
