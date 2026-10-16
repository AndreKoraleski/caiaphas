//! Grammar checks read the whole grammar at once, such as the checks about
//! recursion.

mod case_only_names;
mod duplicate_alternative;
mod empty_rule;
mod exhaustive_exception;
mod indirect_left_recursion;
mod ineffective_exception;
mod left_recursion;
mod longer_branch_conflict;
mod nondeterministic_choice;
mod shared_start;
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
    longer_branch_conflict::check,
    ineffective_exception::check,
    empty_rule::check,
    exhaustive_exception::check,
    case_only_names::check,
    unreachable_alternative::check,
    single_character_rule::check,
    shared_start::check,
    nondeterministic_choice::check,
];
