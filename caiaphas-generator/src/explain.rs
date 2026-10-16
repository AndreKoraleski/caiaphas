//! Explanations tell what each finding means, with an example and the usual
//! fix, as `caiaphas explain` prints them.

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

#[doc = include_str!("findings.md")]
pub mod findings {}
