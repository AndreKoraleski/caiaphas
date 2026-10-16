//! Lowering turns the syntax tree of each syntax rule into an expression of
//! the intermediate representation.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind, Index},
    ir::{Expression, ExpressionKind, Rule},
    lexer::Integer,
    parser::{
        DefinitionsList, SingleDefinition, SyntacticFactor, SyntacticPrimary,
        SyntacticPrimaryKind, SyntacticTerm, SyntaxRule,
    },
};

/// Turns the syntax rules of the index into rules, and reports every meta
/// identifier that no syntax rule defines.
pub(super) fn lower(
    index: &Index<'_>,
    errors: &mut Vec<DictionaryError>,
) -> Vec<Rule> {
    let mut lowering = Lowering { index, errors };
    index
        .syntax_rules
        .iter()
        .map(|syntax_rule| lowering.syntax_rule(syntax_rule))
        .collect()
}

struct Lowering<'a, 'b> {
    index: &'a Index<'a>,
    errors: &'b mut Vec<DictionaryError>,
}

impl Lowering<'_, '_> {
    fn syntax_rule(&mut self, syntax_rule: &SyntaxRule) -> Rule {
        Rule {
            meta_identifier: syntax_rule.meta_identifier.clone(),
            expression: self.definitions_list(&syntax_rule.definitions_list),
            expectations: syntax_rule.expectations.clone(),
            hidden: syntax_rule.hidden,
            first_match: syntax_rule.first_match,
            greedy: syntax_rule.greedy,
            inline: syntax_rule.inline,
            converts_to: syntax_rule.converts_to.clone(),
            comments: syntax_rule.comments.clone(),
            span: syntax_rule.span,
        }
    }

    /// A list of one single definition becomes that definition.
    fn definitions_list(
        &mut self,
        definitions_list: &DefinitionsList,
    ) -> Expression {
        let definitions = &definitions_list.single_definitions;
        if let [definition] = definitions.as_slice() {
            return self.single_definition(definition);
        }
        let expressions = definitions
            .iter()
            .map(|definition| self.single_definition(definition))
            .collect();
        Expression {
            kind: ExpressionKind::DefinitionsList(expressions),
            span: definitions_list.span,
        }
    }

    /// A definition of one syntactic term becomes that term.
    fn single_definition(
        &mut self,
        single_definition: &SingleDefinition,
    ) -> Expression {
        let terms = &single_definition.syntactic_terms;
        if let [term] = terms.as_slice() {
            return self.syntactic_term(term);
        }
        let expressions =
            terms.iter().map(|term| self.syntactic_term(term)).collect();
        Expression {
            kind: ExpressionKind::SingleDefinition(expressions),
            span: single_definition.span,
        }
    }

    /// A term without a syntactic exception becomes its syntactic factor.
    fn syntactic_term(
        &mut self,
        syntactic_term: &SyntacticTerm,
    ) -> Expression {
        let reported = self.errors.len();
        let factor = self.syntactic_factor(&syntactic_term.syntactic_factor);
        let Some(exception) = &syntactic_term.syntactic_exception else {
            return factor;
        };
        let exception = self.syntactic_factor(exception);
        // `[a-z]` reads as an option of `a - z`, so names of one letter on
        // both sides were likely meant as a class of characters.
        let class = one_letter(&syntactic_term.syntactic_factor)
            && syntactic_term
                .syntactic_exception
                .as_ref()
                .is_some_and(one_letter);
        if class {
            for error in &mut self.errors[reported..] {
                if let DictionaryErrorKind::UndefinedMetaIdentifier {
                    class,
                    ..
                } = &mut error.kind
                {
                    *class = true;
                }
            }
        }
        Expression {
            kind: ExpressionKind::SyntacticTerm {
                syntactic_factor: Box::new(factor),
                syntactic_exception: Box::new(exception),
            },
            span: syntactic_term.span,
        }
    }

    /// A factor repeated no times becomes an empty sequence, and one repeated
    /// once, or without an integer, becomes its syntactic primary. The primary
    /// is lowered even when it is dropped, so its undefined meta identifiers
    /// are still reported.
    fn syntactic_factor(
        &mut self,
        syntactic_factor: &SyntacticFactor,
    ) -> Expression {
        let primary =
            self.syntactic_primary(&syntactic_factor.syntactic_primary);
        let kind = match syntactic_factor.integer.map(Integer::get) {
            None | Some(1) => return primary,
            Some(0) => ExpressionKind::EmptySequence,
            Some(integer) => ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary: Box::new(primary),
            },
        };
        Expression {
            kind,
            span: syntactic_factor.span,
        }
    }

    /// A grouped sequence becomes the definitions list it holds.
    fn syntactic_primary(
        &mut self,
        syntactic_primary: &SyntacticPrimary,
    ) -> Expression {
        let kind = match &syntactic_primary.kind {
            SyntacticPrimaryKind::OptionalSequence(list) => {
                ExpressionKind::OptionalSequence(Box::new(
                    self.definitions_list(list),
                ))
            }
            SyntacticPrimaryKind::RepeatedSequence(list) => {
                ExpressionKind::RepeatedSequence(Box::new(
                    self.definitions_list(list),
                ))
            }
            SyntacticPrimaryKind::GroupedSequence(list) => {
                self.definitions_list(list).kind
            }
            SyntacticPrimaryKind::MetaIdentifier(meta_identifier) => {
                if let Some(rule_id) = self.index.get(meta_identifier) {
                    ExpressionKind::MetaIdentifier(rule_id)
                } else {
                    self.errors.push(DictionaryError {
                        kind: DictionaryErrorKind::UndefinedMetaIdentifier {
                            meta_identifier: meta_identifier.clone(),
                            similar: self.index.similar(meta_identifier),
                            separate: self.index.separate(meta_identifier),
                            class: false,
                        },
                        span: syntactic_primary.span,
                    });
                    // Stands in for the missing rule, so lowering goes on.
                    ExpressionKind::EmptySequence
                }
            }
            SyntacticPrimaryKind::TerminalString(string) => {
                ExpressionKind::TerminalString(string.as_str().to_owned())
            }
            SyntacticPrimaryKind::SpecialSequence(sequence) => {
                super::pattern::expression(
                    sequence.pattern(),
                    syntactic_primary.span,
                )
                .kind
            }
            SyntacticPrimaryKind::EmptySequence => {
                ExpressionKind::EmptySequence
            }
        };
        Expression {
            kind,
            span: syntactic_primary.span,
        }
    }
}

/// Returns whether `factor` is only a name of one letter.
fn one_letter(factor: &SyntacticFactor) -> bool {
    factor.integer.is_none()
        && matches!(
            &factor.syntactic_primary.kind,
            SyntacticPrimaryKind::MetaIdentifier(name)
                if name.as_str().len() == 1
        )
}

#[cfg(test)]
mod tests {
    use crate::{
        dictionary::{
            DictionaryError, DictionaryErrorKind,
            tests::{grammar, render},
        },
        span::Span,
    };

    /// Returns how the rule `a` in `source` lowers, asserting no errors.
    fn lowered(source: &str) -> String {
        let (grammar, errors) = grammar(source);
        assert_eq!(errors, [], "{source:?}");
        render(&grammar, "a")
    }

    #[test]
    fn lowers_a_single_primary_without_wrappers() {
        assert_eq!(lowered("a = 'x';"), "'x'");
    }

    #[test]
    fn lowers_definitions_lists_and_single_definitions() {
        assert_eq!(lowered("a = 'x' | 'y';"), "alt('x', 'y')");
        assert_eq!(lowered("a = 'x', 'y';"), "seq('x', 'y')");
        assert_eq!(lowered("a = 'x', 'y' | 'z';"), "alt(seq('x', 'y'), 'z')");
    }

    #[test]
    fn lowers_grouped_sequences_into_nesting() {
        assert_eq!(
            lowered("a = ('x' | 'y'), 'z';"),
            "seq(alt('x', 'y'), 'z')"
        );
        assert_eq!(lowered("a = (('x'));"), "'x'");
    }

    #[test]
    fn lowers_optional_and_repeated_sequences() {
        assert_eq!(lowered("a = ['x'], {'y'};"), "seq(opt('x'), rep('y'))");
    }

    #[test]
    fn lowers_syntactic_factors_by_their_integer() {
        assert_eq!(lowered("a = 3 * 'x';"), "3*'x'");
        assert_eq!(lowered("a = 1 * 'x';"), "'x'");
        assert_eq!(lowered("a = 0 * 'x';"), "empty");
        assert_eq!(lowered("a = 4294967295 * 'x';"), "4294967295*'x'");
    }

    #[test]
    fn lowers_syntactic_exceptions() {
        assert_eq!(lowered("a = 'x' - 'y';"), "except('x', 'y')");
    }

    #[test]
    fn lowers_meta_identifiers_to_their_rules() {
        assert_eq!(lowered("a = b, c; b = 'x'; c = 'y';"), "seq(b, c)");
    }

    #[test]
    fn lowers_terminal_strings_special_sequences_and_empty_sequences() {
        assert_eq!(
            lowered("a = \"x\", ? U+0041..U+005A ?, ;"),
            "seq('x', U+0041..U+005A, empty)"
        );
    }

    #[test]
    fn keeps_the_span_of_a_grouped_sequence() {
        let (grammar, _) = grammar("a = ('x');");
        assert_eq!(
            grammar.syntax_rules[0].expression.span,
            Span { start: 4, end: 9 }
        );
        assert_eq!(grammar.syntax_rules[0].span, Span { start: 0, end: 10 });
    }

    #[test]
    fn keeps_which_rules_are_hidden() {
        let (grammar, _) = grammar("s = ws, 'x'; (* hide *) ws = {' '};");
        let hidden: Vec<bool> = grammar
            .syntax_rules
            .iter()
            .map(|rule| rule.hidden)
            .collect();
        assert_eq!(hidden, [false, true]);
    }

    #[test]
    fn keeps_the_expectations_of_each_rule() {
        let (grammar, _) =
            grammar("(* expect empty-rule: kept *) a = ; b = 'x', a;");
        let codes: Vec<Vec<&str>> = grammar
            .syntax_rules
            .iter()
            .map(|rule| {
                rule.expectations
                    .iter()
                    .map(|expectation| expectation.code.as_str())
                    .collect()
            })
            .collect();
        assert_eq!(codes, [vec!["empty-rule"], vec![]]);
    }

    #[test]
    fn rejects_undefined_meta_identifiers() {
        let (_, errors) =
            grammar("s = item; items = 'x'; Digit = 'y'; t = digit;");
        let similar: Vec<Option<String>> = errors
            .iter()
            .map(|error| match &error.kind {
                DictionaryErrorKind::UndefinedMetaIdentifier {
                    similar,
                    ..
                } => {
                    similar.as_ref().map(|similar| similar.as_str().to_owned())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            similar,
            [Some("items".to_owned()), Some("Digit".to_owned())]
        );
        let (_, errors) = grammar("s = number; name = 'x';");
        assert!(matches!(
            &errors[0].kind,
            DictionaryErrorKind::UndefinedMetaIdentifier { similar: None, .. }
        ));
        let (grammar, errors) = grammar("a = 'x', b;");
        assert_eq!(render(&grammar, "a"), "seq('x', empty)");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            DictionaryError {
                kind: DictionaryErrorKind::UndefinedMetaIdentifier {
                    meta_identifier,
                    ..
                },
                span: Span { start: 9, end: 10 },
            } if meta_identifier.as_str() == "b"
        ));
    }

    #[test]
    fn tells_of_words_that_each_name_a_rule() {
        let (_, errors) =
            grammar("s = word space word; word = 'w'; space = ' ';");
        assert_eq!(
            errors[0].help(),
            "the gaps join `word space word` into one name; to read each rule \
             in turn, write `word, space, word`"
        );
    }

    #[test]
    fn tells_how_to_write_a_class_written_as_in_a_regular_expression() {
        let (_, errors) = grammar("s = [a-z]; t = b;");
        assert!(!errors[2].help().contains("regular expression"));
        assert!(errors[0].help().ends_with("write `? /[a-z]/ ?`"));
    }
}
