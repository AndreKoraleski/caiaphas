//! Grammar checks read the whole grammar at once, such as the checks about
//! recursion.

mod ambiguous_split;
mod case_only_names;
mod duplicate_alternative;
mod empty_rule;
mod exhaustive_exception;
mod indirect_left_recursion;
mod ineffective_annotation;
mod ineffective_exception;
mod left_recursion;
mod longer_branch_conflict;
mod misspelled_annotation;
mod mixed_class_range;
mod nondeterministic_choice;
mod single_character_rule;
mod tied_branches;
mod unproductive_rule;
mod unreachable_alternative;
mod unreachable_rule;

use crate::{
    analysis::{facts::Facts, finding::Finding},
    ir::Grammar,
};

/// A check over a whole grammar.
pub(super) type GrammarCheck = fn(&Grammar, &Facts<'_>, &mut Vec<Finding>);

/// Every grammar check. A new one goes in its own file and in this list.
pub(super) const GRAMMAR_CHECKS: &[GrammarCheck] = &[
    left_recursion::check,
    unproductive_rule::check,
    unreachable_rule::check,
    indirect_left_recursion::check,
    duplicate_alternative::check,
    tied_branches::check,
    ambiguous_split::check,
    longer_branch_conflict::check,
    ineffective_exception::check,
    empty_rule::check,
    exhaustive_exception::check,
    case_only_names::check,
    mixed_class_range::check,
    ineffective_annotation::check,
    misspelled_annotation::check,
    unreachable_alternative::check,
    single_character_rule::check,
    nondeterministic_choice::check,
];
