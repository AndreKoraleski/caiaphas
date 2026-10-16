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

    #[test]
    fn lists_every_code_and_finds_a_misspelled_one() {
        assert_eq!(codes().next(), Some("left-recursion"));
        assert!(codes().any(|code| code == "tied-branches"));
        assert_eq!(similar_code("tied-branch"), Some("tied-branches"));
        assert_eq!(similar_code("whatever"), None);
    }
}
