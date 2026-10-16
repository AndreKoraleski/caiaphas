//! Choice checks read one choice point at a time and compare its branches.

mod follow_conflict;
mod nullable_branches;
mod unreachable_alternative;

use crate::analysis::{choice_point::ChoicePoint, finding::Finding};

/// A check over one choice point.
pub(super) type ChoiceCheck = fn(&ChoicePoint, &mut Vec<Finding>);

/// Every choice check. A new one goes in its own file and in this list.
pub(super) const CHOICE_CHECKS: &[ChoiceCheck] = &[
    nullable_branches::check,
    follow_conflict::check,
    unreachable_alternative::check,
];
