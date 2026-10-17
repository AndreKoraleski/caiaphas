//! Excerpts show the line of an input where something is, with carets under
//! it, to show below a message about it.

/// The most characters of a line an excerpt shows.
const WIDTH: usize = 80;

/// Returns the line of `input` that holds the byte `start`, numbered `number`,
/// with a caret under each of its characters from `start` to `end`, or one at
/// `start` when there are none. A long line shows only the part around them.
pub(crate) fn excerpt(
    input: &str,
    number: usize,
    start: usize,
    end: usize,
) -> String {
    let start = boundary(input, start);
    let end = boundary(input, end).max(start);
    let line_start = input[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = input[start..]
        .find('\n')
        .map_or(input.len(), |at| start + at);
    let line: Vec<char> = input[line_start..line_end]
        .trim_end_matches('\r')
        .chars()
        .collect();
    let at = input[line_start..start].chars().count().min(line.len());
    // What lies past the end of the line is not underlined.
    let width = input[start..end.min(line_end)].chars().count().max(1);
    let first = at
        .saturating_sub(WIDTH / 2)
        .min(line.len().saturating_sub(WIDTH));
    let last = (first + WIDTH).min(line.len());
    let (mut shown, mut under) = (String::new(), String::new());
    if first > 0 {
        shown.push_str("...");
        under.push_str("   ");
    }
    for (index, &c) in line[first..last].iter().enumerate() {
        // A character that moves the cursor would hide the carets.
        shown.push(if c.is_control() && c != '\t' {
            '\u{FFFD}'
        } else {
            c
        });
        if first + index < at {
            under.push(if c == '\t' { '\t' } else { ' ' });
        }
    }
    if last < line.len() {
        shown.push_str("...");
    }
    let carets = width.min(last.saturating_sub(at)).max(1);
    under.push_str(&"^".repeat(carets));
    let number = number.to_string();
    let gutter = " ".repeat(number.len());
    format!("{number} | {shown}\n{gutter} | {under}")
}

/// Returns `offset`, within `input` and moved back to where a character
/// starts.
fn boundary(input: &str, offset: usize) -> usize {
    let mut offset = offset.min(input.len());
    while !input.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underlines_each_character_from_start_to_end() {
        assert_eq!(
            excerpt("let y = x;", 1, 8, 9),
            "1 | let y = x;\n  |         ^"
        );
        assert_eq!(
            excerpt("a = abc + 1", 1, 4, 7),
            "1 | a = abc + 1\n  |     ^^^"
        );
        assert_eq!(excerpt("a = ;", 1, 4, 4), "1 | a = ;\n  |     ^");
    }

    #[test]
    fn underlines_up_to_the_end_of_the_first_line() {
        assert_eq!(
            excerpt("x = (1,\n 2)", 1, 4, 11),
            "1 | x = (1,\n  |     ^^^"
        );
        assert_eq!(excerpt("ab\ncd", 2, 3, 5), "2 | cd\n  | ^^");
    }

    #[test]
    fn underlines_no_further_than_the_part_of_a_long_line_it_shows() {
        let line = "a".repeat(200);
        let shown = excerpt(&line, 1, 150, 200);
        let under = shown.lines().nth(1).unwrap();
        assert_eq!(under.matches('^').count(), 40);
        assert!(shown.starts_with("1 | ...a"), "{shown}");
    }
}
