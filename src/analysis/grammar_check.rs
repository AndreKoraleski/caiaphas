//! Grammar checks read the whole grammar, such as those about recursion.

mod ambiguous_recursion;
mod case_only_names;
mod duplicate_alternative;
mod empty_rule;
mod exhaustive_exception;
mod indirect_left_recursion;
mod ineffective_exception;
mod left_recursion;
mod unproductive_rule;
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
    ambiguous_recursion::check,
    unproductive_rule::check,
    unreachable_rule::check,
    indirect_left_recursion::check,
    duplicate_alternative::check,
    ineffective_exception::check,
    empty_rule::check,
    exhaustive_exception::check,
    case_only_names::check,
];
