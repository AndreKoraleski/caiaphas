//! Gap separators of ISO/IEC 14977 EBNF, whitespace between symbols.

#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

/// Returns how long the gap at the start of `input` is, or 0 if there is none.
///
/// `input` is the source from the current position onward.
///
/// A gap is any number of spaces, horizontal or vertical tabs, form feeds and
/// new lines. A new line is a line feed with any number of carriage returns on
/// either side, so a carriage return without a line feed is not a gap.
///
/// Every gap separator is ASCII, so the length is the same in bytes and
/// characters.
pub(crate) fn length(input: &str) -> usize {
    let mut total = 0;
    loop {
        let after = &input[total..];
        let separator = match after.chars().next() {
            Some(' ' | '\t' | '\x0B' | '\x0C') => 1,
            Some('\r' | '\n') => new_line_length(after),
            _ => 0,
        };
        if separator == 0 {
            return total;
        }
        total += separator;
    }
}

/// Returns the characters at the start of `input` that `accept` takes,
/// with the gaps between them removed, and how long they are, those gaps
/// included.
///
/// A gap is only taken when an accepted character follows it, so a gap after
/// the last one is left for the caller. Returns an empty string and 0 if the
/// first character is not accepted.
///
/// `accept` should only take ASCII characters, so the length is the same in
/// bytes and characters.
pub(crate) fn remove(
    input: &str,
    accept: impl Fn(char) -> bool,
) -> (String, usize) {
    let mut kept = String::new();
    let mut total = 0;
    loop {
        let gap = if kept.is_empty() {
            0
        } else {
            length(&input[total..])
        };
        match input[total + gap..].chars().next() {
            Some(c) if accept(c) => {
                kept.push(c);
                total += gap + c.len_utf8();
            }
            _ => return (kept, total),
        }
    }
}

/// Returns how long the new line at the start of `input` is, or 0 if there is
/// none.
fn new_line_length(input: &str) -> usize {
    let before = carriage_returns(input);
    match input[before..].strip_prefix('\n') {
        Some(after) => before + 1 + carriage_returns(after),
        None => 0,
    }
}

/// Returns how many carriage returns `input` starts with.
fn carriage_returns(input: &str) -> usize {
    input.len() - input.trim_start_matches('\r').len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_letters(input: &str) -> (String, usize) {
        remove(input, |c| c.is_ascii_alphabetic())
    }

    #[test]
    fn removes_gaps_between_characters() {
        assert_eq!(read_letters("ab c\n\td"), ("abcd".to_owned(), 7));
    }

    #[test]
    fn leaves_a_gap_after_the_last_character() {
        assert_eq!(read_letters("ab  = c"), ("ab".to_owned(), 2));
        assert_eq!(read_letters("ab \n"), ("ab".to_owned(), 2));
    }

    #[test]
    fn stops_at_a_character_that_is_not_accepted() {
        assert_eq!(read_letters("ab1c"), ("ab".to_owned(), 2));
        assert_eq!(read_letters("a (* b *)"), ("a".to_owned(), 1));
    }

    #[test]
    fn keeps_a_lone_carriage_return_as_a_stop() {
        assert_eq!(read_letters("a\rb"), ("a".to_owned(), 1));
    }

    #[test]
    fn reads_nothing_when_the_first_character_is_not_accepted() {
        for input in ["", "1a", " a", "\na"] {
            assert_eq!(read_letters(input), (String::new(), 0), "{input:?}");
        }
    }

    #[test]
    fn reads_each_separator() {
        for text in [" ", "\t", "\x0B", "\x0C", "\n"] {
            assert_eq!(length(text), 1, "{text:?}");
        }
    }

    #[test]
    fn reads_a_run_of_separators() {
        assert_eq!(length("  \t\n\x0B\x0C x"), 7);
    }

    #[test]
    fn reads_carriage_returns_around_a_line_feed() {
        for text in
            ["\r\n", "\n\r", "\r\r\n", "\n\r\r", "\r\n\r", "\r\r\n\r\r"]
        {
            assert_eq!(length(text), text.len(), "{text:?}");
        }
    }

    #[test]
    fn reads_consecutive_new_lines() {
        assert_eq!(length("\r\n\r\n\n"), 5);
    }

    #[test]
    fn rejects_carriage_returns_without_a_line_feed() {
        for text in ["\r", "\r\r", "\rx", "\r\r x", "\r \n"] {
            assert_eq!(length(text), 0, "{text:?}");
        }
    }

    #[test]
    fn stops_before_carriage_returns_without_a_line_feed() {
        assert_eq!(length(" \rx"), 1);
        assert_eq!(length("\n \r"), 2);
    }

    #[test]
    fn stops_at_the_first_character_that_is_not_a_gap() {
        assert_eq!(length("  a  "), 2);
        assert_eq!(length("\n(* *)"), 1);
    }

    #[test]
    fn reads_nothing_without_a_gap() {
        for text in ["", "a", "(", "'"] {
            assert_eq!(length(text), 0, "{text:?}");
        }
    }

    #[test]
    fn rejects_non_ascii_spaces() {
        // Characters counted as whitespace, but not gap separators in EBNF.
        for text in [
            "\u{00A0}", // no-break space
            "\u{2003}", // em space
            "\u{3000}", // ideographic space
            "\u{0085}", // next line
            "\u{2028}", // line separator
        ] {
            assert_eq!(length(text), 0, "{text:?}");
        }
    }
}
