//! Lowering turns the syntax tree of each syntax rule into an expression of
//! the intermediate representation.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind, Index},
    ir::{Expression, ExpressionKind, Rule},
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
            definitions_list: self
                .definitions_list(&syntax_rule.definitions_list),
            span: syntax_rule.span,
        }
    }

    /// A list of one single definition becomes that definition.
    fn definitions_list(
        &mut self,
        definitions_list: &DefinitionsList,
    ) -> Expression {
        let mut expressions: Vec<Expression> = definitions_list
            .single_definitions
            .iter()
            .map(|definition| self.single_definition(definition))
            .collect();
        if expressions.len() == 1 {
            return expressions.remove(0);
        }
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
        let mut expressions: Vec<Expression> = single_definition
            .syntactic_terms
            .iter()
            .map(|term| self.syntactic_term(term))
            .collect();
        if expressions.len() == 1 {
            return expressions.remove(0);
        }
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
        let factor = self.syntactic_factor(&syntactic_term.syntactic_factor);
        let Some(exception) = &syntactic_term.syntactic_exception else {
            return factor;
        };
        Expression {
            kind: ExpressionKind::SyntacticTerm {
                syntactic_factor: Box::new(factor),
                syntactic_exception: Box::new(
                    self.syntactic_factor(exception),
                ),
            },
            span: syntactic_term.span,
        }
    }

    /// A factor repeated no times becomes an empty sequence, and one repeated
    /// once, or without an integer, becomes its syntactic primary.
    fn syntactic_factor(
        &mut self,
        syntactic_factor: &SyntacticFactor,
    ) -> Expression {
        let primary =
            self.syntactic_primary(&syntactic_factor.syntactic_primary);
        let kind = match syntactic_factor.integer.map(|integer| integer.get())
        {
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
                match self.index.get(meta_identifier) {
                    Some(rule_id) => ExpressionKind::MetaIdentifier(rule_id),
                    None => {
                        self.errors.push(DictionaryError {
                            kind:
                                DictionaryErrorKind::UndefinedMetaIdentifier {
                                    meta_identifier: meta_identifier.clone(),
                                },
                            span: syntactic_primary.span,
                        });
                        // Stands in for the missing rule, so lowering goes on.
                        ExpressionKind::EmptySequence
                    }
                }
            }
            SyntacticPrimaryKind::TerminalString(string) => {
                ExpressionKind::TerminalString(string.as_str().to_owned())
            }
            SyntacticPrimaryKind::SpecialSequence(sequence) => {
                ExpressionKind::SpecialSequence(sequence.range().clone())
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
            grammar.syntax_rules[0].definitions_list.span,
            Span { start: 4, end: 9 }
        );
        assert_eq!(grammar.syntax_rules[0].span, Span { start: 0, end: 10 });
    }

    #[test]
    fn rejects_undefined_meta_identifiers() {
        let (grammar, errors) = grammar("a = 'x', b;");
        assert_eq!(render(&grammar, "a"), "seq('x', empty)");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            DictionaryError {
                kind: DictionaryErrorKind::UndefinedMetaIdentifier {
                    meta_identifier,
                },
                span: Span { start: 9, end: 10 },
            } if meta_identifier.as_str() == "b"
        ));
    }
}
