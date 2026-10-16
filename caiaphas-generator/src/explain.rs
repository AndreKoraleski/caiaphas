//! Explanations tell what each finding means, with an example and the usual
//! fix, as `caiaphas explain` prints them.

use crate::spelling;

/// Every explanation, each under a heading with its code.
const FINDINGS: &str = include_str!("findings.md");

/// Returns the explanation of the finding with `code`, if there is one.
#[must_use]
pub fn explain(code: &str) -> Option<String> {
    let heading = format!("## {code}");
    let mut lines = FINDINGS.lines().skip_while(|line| *line != heading);
    lines.next()?;
    let text: Vec<&str> =
        lines.take_while(|line| !line.starts_with("## ")).collect();
    Some(text.join("\n").trim().to_owned())
}

/// Returns the code of every finding, in the order the [`findings`] page lists
/// them.
pub fn codes() -> impl Iterator<Item = &'static str> {
    FINDINGS.lines().filter_map(|line| line.strip_prefix("## "))
}

/// Returns the code of a finding that `code` is close to, as when it is
/// misspelled.
#[must_use]
pub fn similar_code(code: &str) -> Option<&'static str> {
    spelling::closest(code, codes())
}

#[doc = include_str!("findings.md")]
pub mod findings {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;

    /// An example of the explanation of `code`, with a grammar it is written
    /// in and, when the explanation shows one, its fix.
    struct Example {
        code: &'static str,
        /// What the explanation writes, as the grammar holds it.
        written: &'static str,
        /// A grammar that holds `written` and reports `code`.
        grammar: &'static str,
        /// What the explanation writes in place of `written` to fix it.
        fixed: Option<&'static str>,
    }

    const fn example(
        code: &'static str,
        written: &'static str,
        grammar: &'static str,
    ) -> Example {
        Example {
            code,
            written,
            grammar,
            fixed: None,
        }
    }

    const fn fixed(
        code: &'static str,
        written: &'static str,
        grammar: &'static str,
        fixed: &'static str,
    ) -> Example {
        Example {
            code,
            written,
            grammar,
            fixed: Some(fixed),
        }
    }

    const EXAMPLES: &[Example] = &[
        fixed(
            "left-recursion",
            "expression = expression, '+', term | term;",
            "expression = expression, '+', term | term; term = 'x';",
            "expression = term, {'+', term};",
        ),
        example(
            "indirect-left-recursion",
            "s = a; a = b, 'x' | 'y'; b = a, 'z';",
            "s = a; a = b, 'x' | 'y'; b = a, 'z';",
        ),
        fixed(
            "unproductive-rule",
            "list = item, list;",
            "list = item, list; item = 'x';",
            "list = item, [list];",
        ),
        fixed(
            "nullable-repeated-sequence",
            "{['x']}",
            "a = {['x']};",
            "{'x'}",
        ),
        example(
            "tied-branches",
            "word = keyword | name;",
            "word = keyword | name; keyword = 'if'; name = ? /[a-z]+/ ?;",
        ),
        example(
            "longer-branch-conflict",
            "(* greedy *) s = ('x' | 'x', 'y'), 'y';",
            "(* greedy *) s = ('x' | 'x', 'y'), 'y';",
        ),
        example(
            "optional-follow-conflict",
            "(* greedy *) s = 'if', c, s, ['else', s] | 'x';",
            "(* greedy *) s = 'if', c, s, ['else', s] | 'x'; c = 'c';",
        ),
        example(
            "repeated-follow-conflict",
            "(* greedy *) s = {'xy'}, 'x', 'z';",
            "(* greedy *) s = {'xy'}, 'x', 'z';",
        ),
        example(
            "required-optional-sequence",
            "(* greedy *) s = ['x'], 'x';",
            "(* greedy *) s = ['x'], 'x';",
        ),
        fixed(
            "dead-end-repeated-sequence",
            "{'x'}, 'x'",
            "(* greedy *) s = {'x'}, 'x';",
            "'x', {'x'}",
        ),
        example(
            "ambiguous-nullable-branches",
            "a = ['x'] | ['y'];",
            "a = ['x'] | ['y'];",
        ),
        example(
            "unreachable-rule",
            "s = 'x'; a = b; b = 'y', [a];",
            "s = 'x'; a = b; b = 'y', [a];",
        ),
        example(
            "unreachable-alternative",
            "word = name | 'if';",
            "word = name | 'if'; name = ? /[a-z]+/ ?;",
        ),
        example("duplicate-alternative", "a = 'x' | 'x';", "a = 'x' | 'x';"),
        example("ineffective-exception", "a = 'x' - 'y';", "a = 'x' - 'y';"),
        fixed(
            "ineffective-exception",
            "'/*', {character - '*/'}, '*/'",
            "comment = '/*', {character - '*/'}, '*/'; character = ? /./ ?;",
            "'/*', ({character} - ({character}, '*/', {character})), '*/'",
        ),
        example("empty-rule", "a = ;", "a = ;"),
        example("exhaustive-exception", "a = 'x' - 'x';", "a = 'x' - 'x';"),
        example(
            "case-only-names",
            "Digit",
            "s = digit, Digit; digit = 'd'; Digit = 'D';",
        ),
        fixed(
            "mixed-class-range",
            "? U+0041..U+007A ?",
            "a = ? U+0041..U+007A ?;",
            "? U+0041..U+005A ? | ? U+0061..U+007A ?",
        ),
        example(
            "ineffective-annotation",
            "(* greedy *)",
            "(* greedy *) a = 'x';",
        ),
        example(
            "single-character-rule",
            "number = digit, {digit};",
            "number = digit, {digit}; digit = ? U+0030..U+0039 ?;",
        ),
        fixed(
            "shared-start",
            "a, x | a, y",
            "s = a, x | a, y; a = 'a'; x = 'x'; y = 'y';",
            "a, (x | y)",
        ),
        example(
            "nondeterministic-choice",
            "statement = 'if', c, statement, ['else', statement] | 'x';",
            "statement = 'if', c, statement, ['else', statement] | 'x'; \
             c = 'c';",
        ),
        example(
            "unknown-expectation",
            "(* expect no-such-finding: reason *)",
            "(* expect no-such-finding: reason *) a = 'x';",
        ),
    ];

    /// Returns the code of every finding about `grammar`.
    fn codes_of(grammar: &str) -> Vec<&'static str> {
        let (_, diagnostics) = compile(grammar, false);
        diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn reports_and_fixes_each_example_of_an_explanation() {
        for example in EXAMPLES {
            let explanation = explain(example.code).expect(example.code);
            assert!(
                explanation.contains(example.written)
                    && example.grammar.contains(example.written),
                "{}: {:?}",
                example.code,
                example.written,
            );
            assert!(
                codes_of(example.grammar).contains(&example.code),
                "{}: {:?}",
                example.code,
                example.grammar,
            );
            if let Some(fixed) = example.fixed {
                assert!(explanation.contains(fixed), "{fixed:?}");
                let grammar = example.grammar.replace(example.written, fixed);
                assert!(
                    !codes_of(&grammar).contains(&example.code),
                    "{}: {grammar:?}",
                    example.code,
                );
            }
        }
    }

    #[test]
    fn lists_every_code_and_finds_a_misspelled_one() {
        assert_eq!(codes().next(), Some("left-recursion"));
        assert!(codes().any(|code| code == "tied-branches"));
        assert_eq!(similar_code("tied-branch"), Some("tied-branches"));
        assert_eq!(similar_code("whatever"), None);
    }
}
