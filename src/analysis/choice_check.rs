//! Choice checks read one choice point at a time, and compare what its
//! branches can start with.

mod overlapping_branches;

use crate::analysis::{choice_point::ChoicePoint, finding::Finding};

/// A check over one choice point.
pub(super) type ChoiceCheck = fn(&ChoicePoint, &mut Vec<Finding>);

/// Every choice check. A new one goes in its own file and in this list.
pub(super) const CHOICE_CHECKS: &[ChoiceCheck] =
    &[overlapping_branches::check];
