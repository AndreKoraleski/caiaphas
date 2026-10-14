//! Gap separators of ISO/IEC 14977 EBNF, whitespace between symbols.

#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

/// Returns how long the gap at the start of `rest` is, or 0 if there is none.
///
/// `rest` here means the remaining characters of the source string.
///
/// A gap is any number of spaces, horizontal or vertical tabs, form feeds and
/// new lines. A new line is a line feed with any number of carriage returns on
/// either side, so a carriage return without a line feed is not a gap.
///
/// Every gap separator is ASCII, so the length is the same in bytes and
/// characters.
pub(crate) fn measure(rest: &str) -> usize {
    let mut length = 0;
    loop {
        let after = &rest[length..];
        let separator = match after.chars().next() {
            Some(' ' | '\t' | '\x0B' | '\x0C') => 1,
            Some('\r' | '\n') => measure_new_line(after),
            _ => 0,
        };
        if separator == 0 {
            return length;
        }
        length += separator;
    }
}

/// Returns the characters at the start of `rest` that `accept` takes, with the
/// gaps between them removed, and how long they are, those gaps included.
///
/// A gap is only taken when an accepted character follows it, so a gap after
/// the last one is left for the caller. Returns an empty string and 0 if the
/// first character is not accepted.
///
/// `accept` should only take ASCII characters, so the length is the same in
/// bytes and characters.
pub(crate) fn read_joined(
    rest: &str,
    accept: impl Fn(char) -> bool,
) -> (String, usize) {
    let mut joined = String::new();
    let mut length = 0;
    loop {
        let gap = if joined.is_empty() {
            0
        } else {
            measure(&rest[length..])
        };
        match rest[length + gap..].chars().next() {
            Some(c) if accept(c) => {
                joined.push(c);
                length += gap + c.len_utf8();
            }
            _ => return (joined, length),
        }
    }
}

/// Returns how long the new line at the start of `rest` is, or 0 if there is
/// none.
fn measure_new_line(rest: &str) -> usize {
    let before = carriage_returns(rest);
    match rest[before..].strip_prefix('\n') {
        Some(after) => before + 1 + carriage_returns(after),
        None => 0,
    }
}

/// Returns how many carriage returns `rest` starts with.
fn carriage_returns(rest: &str) -> usize {
    rest.len() - rest.trim_start_matches('\r').len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_letters(rest: &str) -> (String, usize) {
        read_joined(rest, |c| c.is_ascii_alphabetic())
    }

    #[test]
    fn joins_characters_across_gaps() {
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
    fn does_not_join_across_a_lone_carriage_return() {
        assert_eq!(read_letters("a\rb"), ("a".to_owned(), 1));
    }

    #[test]
    fn reads_nothing_when_the_first_character_is_not_accepted() {
        for rest in ["", "1a", " a", "\na"] {
            assert_eq!(read_letters(rest), (String::new(), 0), "{rest:?}");
        }
    }

    #[test]
    fn reads_each_separator() {
        for text in [" ", "\t", "\x0B", "\x0C", "\n"] {
            assert_eq!(measure(text), 1, "{text:?}");
        }
    }

    #[test]
    fn reads_a_run_of_separators() {
        assert_eq!(measure("  \t\n\x0B\x0C x"), 7);
    }

    #[test]
    fn reads_carriage_returns_around_a_line_feed() {
        for text in
            ["\r\n", "\n\r", "\r\r\n", "\n\r\r", "\r\n\r", "\r\r\n\r\r"]
        {
            assert_eq!(measure(text), text.len(), "{text:?}");
        }
    }

    #[test]
    fn reads_consecutive_new_lines() {
        assert_eq!(measure("\r\n\r\n\n"), 5);
    }

    #[test]
    fn rejects_carriage_returns_without_a_line_feed() {
        for text in ["\r", "\r\r", "\rx", "\r\r x", "\r \n"] {
            assert_eq!(measure(text), 0, "{text:?}");
        }
    }

    #[test]
    fn stops_before_carriage_returns_without_a_line_feed() {
        assert_eq!(measure(" \rx"), 1);
        assert_eq!(measure("\n \r"), 2);
    }

    #[test]
    fn stops_at_the_first_character_that_is_not_a_gap() {
        assert_eq!(measure("  a  "), 2);
        assert_eq!(measure("\n(* *)"), 1);
    }

    #[test]
    fn reads_nothing_without_a_gap() {
        for text in ["", "a", "(", "'"] {
            assert_eq!(measure(text), 0, "{text:?}");
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
            assert_eq!(measure(text), 0, "{text:?}");
        }
    }
}
