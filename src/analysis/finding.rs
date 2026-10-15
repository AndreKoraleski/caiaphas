//! Findings are what the analysis reports about a grammar.

use std::fmt;

use crate::{lexer::MetaIdentifier, span::Span};

/// A finding is one problem the analysis found, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) kind: FindingKind,
    pub(crate) span: Span,
    /// Other places that explain the finding, such as another branch.
    pub(crate) labels: Vec<Label>,
}

/// A label points at another place a finding involves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: &'static str,
}

/// What a [`Finding`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FindingKind {
    /// A rule can reach itself before reading anything, so its parser would
    /// never end.
    LeftRecursion { meta_identifier: MetaIdentifier },
}

impl FindingKind {
    /// Returns how to fix the grammar, as the usual remedy for this kind of
    /// problem.
    pub(crate) fn help(&self) -> String {
        match self {
            Self::LeftRecursion { meta_identifier } => {
                let name = meta_identifier.as_str();
                format!(
                    "rewrite the recursion as a repetition, so \
                     `{name} = {name}, x | y;` becomes `{name} = y, {{x}};`"
                )
            }
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FindingKind::LeftRecursion { meta_identifier } => write!(
                f,
                "`{}` can reach itself before reading anything, so its parser \
                 would never end",
                meta_identifier.as_str()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grammar with a finding of one kind, its message and how to fix it.
    struct Case {
        source: &'static str,
        is_kind: fn(&FindingKind) -> bool,
        message: &'static str,
        help: &'static str,
    }

    /// One case for every kind of finding. A new kind adds its case here.
    const CASES: &[Case] = &[Case {
        source: "a = a, 'x' | 'y';",
        is_kind: |kind| matches!(kind, FindingKind::LeftRecursion { .. }),
        message: "`a` can reach itself before reading anything, so its \
                      parser would never end",
        help: "rewrite the recursion as a repetition, so `a = a, x | y;` \
                   becomes `a = y, {x};`",
    }];

    #[test]
    fn describes_every_kind_of_finding() {
        for case in CASES {
            let finding = crate::analysis::tests::findings(case.source)
                .into_iter()
                .find(|finding| (case.is_kind)(&finding.kind))
                .unwrap_or_else(|| {
                    panic!("no such finding in {:?}", case.source)
                });
            assert_eq!(finding.to_string(), case.message, "{:?}", case.source);
            assert_eq!(finding.kind.help(), case.help, "{:?}", case.source);
        }
    }
}
