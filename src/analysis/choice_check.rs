//! Choice checks read one choice point at a time, where a parser has to pick
//! one of its branches, and compare the branches.

mod follow_conflict;
mod nullable_branches;

use crate::analysis::{choice_point::ChoicePoint, finding::Finding};

/// A check over one choice point.
pub(super) type ChoiceCheck = fn(&ChoicePoint, &mut Vec<Finding>);

/// Every choice check. A new one goes in its own file and in this list.
pub(super) const CHOICE_CHECKS: &[ChoiceCheck] =
    &[nullable_branches::check, follow_conflict::check];
