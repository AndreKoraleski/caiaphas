//! Patterns are the regular expressions a special sequence can hold between
//! two `/`, such as `? /[a-z]+/ ?`.
//!
//! A pattern holds characters and classes of them, such as `a`, `[a-z_]`,
//! `[^"]`, `.`, `\d`, `\w` and `\s`, one after another, choices between them
//! with `|`, groups in parentheses, and repetitions with `*`, `+`, `{n}`,
//! `{n,}` and `{n,m}`. A `?` would end the special sequence, so `{0,1}` stands
//! for an optional part. `U+XXXX` stands for that code point anywhere a
//! character can.

// A set of characters is a list of ranges, and often holds just one.
#![allow(clippy::single_range_in_vec_init)]

use std::ops::RangeInclusive;

/// What a pattern matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pattern {
    /// One character in any of these ranges, which are sorted and apart.
    Characters(Vec<RangeInclusive<char>>),
    /// Each of these, one after another.
    Sequence(Vec<Pattern>),
    /// One of these.
    Choice(Vec<Pattern>),
    /// The pattern at least `least` times, and at most `most` when it is set.
    Repeat {
        pattern: Box<Pattern>,
        least: u32,
        most: Option<u32>,
    },
}

/// Returns the pattern written `text`, or why it is not one.
pub(crate) fn parse(text: &str) -> Result<Pattern, &'static str> {
    let chars: Vec<char> = text.chars().collect();
    let mut reader = Reader {
        chars: &chars,
        position: 0,
    };
    let pattern = reader.choice()?;
    match reader.peek() {
        None => Ok(pattern),
        Some(')') => Err("a `)` closes no group"),
        Some(_) => Err("a character here starts nothing"),
    }
}

/// Reads a pattern one character at a time.
struct Reader<'a> {
    chars: &'a [char],
    position: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.position += 1;
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        self.position += usize::from(found);
        found
    }

    /// Reads sequences apart by `|`.
    fn choice(&mut self) -> Result<Pattern, &'static str> {
        let mut alternatives = vec![self.sequence()?];
        while self.eat('|') {
            alternatives.push(self.sequence()?);
        }
        Ok(if alternatives.len() == 1 {
            alternatives.remove(0)
        } else {
            Pattern::Choice(alternatives)
        })
    }

    /// Reads repeated atoms one after another, up to a `|`, a `)` or the end.
    fn sequence(&mut self) -> Result<Pattern, &'static str> {
        let mut steps = Vec::new();
        while !matches!(self.peek(), None | Some('|' | ')')) {
            let atom = self.atom()?;
            steps.push(self.repeats(atom)?);
        }
        if steps.is_empty() {
            return Err("each part of a pattern needs something to match");
        }
        Ok(if steps.len() == 1 {
            steps.remove(0)
        } else {
            Pattern::Sequence(steps)
        })
    }

    /// Reads the repetitions written after `pattern`.
    fn repeats(
        &mut self,
        mut pattern: Pattern,
    ) -> Result<Pattern, &'static str> {
        loop {
            let (least, most) = if self.eat('*') {
                (0, None)
            } else if self.eat('+') {
                (1, None)
            } else if self.eat('{') {
                self.count()?
            } else {
                return Ok(pattern);
            };
            pattern = Pattern::Repeat {
                pattern: Box::new(pattern),
                least,
                most,
            };
        }
    }

    /// Reads the rest of `{n}`, `{n,}` or `{n,m}` after its `{`.
    fn count(&mut self) -> Result<(u32, Option<u32>), &'static str> {
        let least = self.number().ok_or("a count needs a number after `{`")?;
        let most = if self.eat(',') {
            if self.peek() == Some('}') {
                None
            } else {
                Some(self.number().ok_or("a count needs a number after `,`")?)
            }
        } else {
            Some(least)
        };
        if !self.eat('}') {
            return Err("a count needs a `}`");
        }
        if most.is_some_and(|most| most < least) {
            return Err("a count goes up to less than it starts from");
        }
        Ok((least, most))
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.position;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.position += 1;
        }
        let digits: String = self.chars[start..self.position].iter().collect();
        digits.parse().ok()
    }

    /// Reads one character, class or group.
    fn atom(&mut self) -> Result<Pattern, &'static str> {
        let c = self.peek().ok_or("the pattern ends too soon")?;
        match c {
            '(' => {
                self.next();
                let inner = self.choice()?;
                if !self.eat(')') {
                    return Err("a group needs a `)`");
                }
                Ok(inner)
            }
            '[' => {
                self.next();
                self.class()
            }
            '.' => {
                self.next();
                Ok(Pattern::Characters(vec!['\0'..=char::MAX]))
            }
            '*' | '+' | '{' => Err("a repetition needs something before it"),
            ']' | '}' => Err("this closes nothing that was opened"),
            _ => Ok(Pattern::Characters(self.character()?)),
        }
    }

    /// Reads one character, an escape or a code point, as the ranges it stands
    /// for.
    fn character(
        &mut self,
    ) -> Result<Vec<RangeInclusive<char>>, &'static str> {
        if let Some(code_point) = self.code_point()? {
            return Ok(vec![code_point..=code_point]);
        }
        let c = self.next().ok_or("the pattern ends too soon")?;
        if c != '\\' {
            return Ok(vec![c..=c]);
        }
        let escaped =
            self.next().ok_or("a `\\` needs a character after it")?;
        Ok(match escaped {
            'd' => vec!['0'..='9'],
            'w' => vec!['0'..='9', 'A'..='Z', '_'..='_', 'a'..='z'],
            's' => vec!['\t'..='\n', '\r'..='\r', ' '..=' '],
            'n' => vec!['\n'..='\n'],
            'r' => vec!['\r'..='\r'],
            't' => vec!['\t'..='\t'],
            c if c.is_ascii_alphanumeric() => {
                return Err("this escape stands for nothing");
            }
            c => vec![c..=c],
        })
    }

    /// Reads `U+` and four to six uppercase hexadecimal digits, if they come
    /// next.
    fn code_point(&mut self) -> Result<Option<char>, &'static str> {
        let rest = &self.chars[self.position..];
        if rest.len() < 6 || rest[0] != 'U' || rest[1] != '+' {
            return Ok(None);
        }
        let digits: String = rest[2..]
            .iter()
            .take(6)
            .take_while(|c| matches!(c, '0'..='9' | 'A'..='F'))
            .collect();
        if digits.len() < 4 {
            return Ok(None);
        }
        let value = u32::from_str_radix(&digits, 16)
            .expect("six hexadecimal digits fit in a u32");
        let c =
            char::from_u32(value).ok_or("a code point is not a character")?;
        self.position += 2 + digits.len();
        Ok(Some(c))
    }

    /// Reads the rest of a class after its `[`.
    fn class(&mut self) -> Result<Pattern, &'static str> {
        let negated = self.eat('^');
        let mut ranges = Vec::new();
        loop {
            match self.peek() {
                None => return Err("a class needs a `]`"),
                Some(']') => {
                    self.next();
                    break;
                }
                Some(_) => {}
            }
            let first = self.character()?;
            let is_range = self.peek() == Some('-')
                && !matches!(
                    self.chars.get(self.position + 1),
                    Some(']') | None
                );
            if is_range {
                self.next();
                let last = self.character()?;
                let ([first], [last]) = (&first[..], &last[..]) else {
                    return Err("a range goes from one character to another");
                };
                let (first, last) = (*first.start(), *last.end());
                if first > last {
                    return Err("a range ends before it starts");
                }
                ranges.push(first..=last);
            } else {
                ranges.extend(first);
            }
        }
        if ranges.is_empty() {
            return Err("a class needs a character");
        }
        let ranges = merged(ranges);
        Ok(Pattern::Characters(if negated {
            complement(&ranges)
        } else {
            ranges
        }))
    }
}

/// Returns `ranges` sorted, with those that overlap or touch joined.
pub(crate) fn merged(
    mut ranges: Vec<RangeInclusive<char>>,
) -> Vec<RangeInclusive<char>> {
    ranges.sort_by_key(|range| *range.start());
    let mut joined: Vec<RangeInclusive<char>> = Vec::new();
    for range in ranges {
        match joined.last_mut() {
            Some(last)
                if after(*last.end())
                    .is_none_or(|next| *range.start() <= next) =>
            {
                if range.end() > last.end() {
                    *last = *last.start()..=*range.end();
                }
            }
            _ => joined.push(range),
        }
    }
    joined
}

/// Returns every character outside the sorted and apart `ranges`.
fn complement(ranges: &[RangeInclusive<char>]) -> Vec<RangeInclusive<char>> {
    let mut outside = Vec::new();
    let mut from = Some('\0');
    for range in ranges {
        if let (Some(start), Some(end)) = (from, before(*range.start())) {
            if start <= end {
                outside.push(start..=end);
            }
        }
        from = after(*range.end());
    }
    if let Some(start) = from {
        outside.push(start..=char::MAX);
    }
    outside
}

/// Returns the character right after `c`, past the surrogates.
fn after(c: char) -> Option<char> {
    match c {
        '\u{D7FF}' => Some('\u{E000}'),
        char::MAX => None,
        c => char::from_u32(u32::from(c) + 1),
    }
}

/// Returns the character right before `c`, past the surrogates.
fn before(c: char) -> Option<char> {
    match c {
        '\u{E000}' => Some('\u{D7FF}'),
        '\0' => None,
        c => char::from_u32(u32::from(c) - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn characters(ranges: &[RangeInclusive<char>]) -> Pattern {
        Pattern::Characters(ranges.to_vec())
    }

    #[test]
    fn reads_characters_classes_and_escapes() {
        assert_eq!(parse("a"), Ok(characters(&['a'..='a'])));
        assert_eq!(parse("[a-zx_]"), Ok(characters(&['_'..='_', 'a'..='z'])));
        assert_eq!(parse("\\d"), Ok(characters(&['0'..='9'])));
        assert_eq!(parse("[\\]-]"), Ok(characters(&['-'..='-', ']'..=']'])));
        assert_eq!(parse("U+00E9"), Ok(characters(&['é'..='é'])));
        assert_eq!(parse("[U+00E0-U+00FF]"), Ok(characters(&['à'..='ÿ'])));
    }

    #[test]
    fn reads_a_negated_class_as_every_other_character() {
        assert_eq!(
            parse("[^\"\\\\]"),
            Ok(characters(&['\0'..='!', '#'..='[', ']'..=char::MAX]))
        );
    }

    #[test]
    fn reads_sequences_choices_groups_and_repetitions() {
        let digit = characters(&['0'..='9']);
        assert_eq!(
            parse("\\d+(x|y){2,3}"),
            Ok(Pattern::Sequence(vec![
                Pattern::Repeat {
                    pattern: Box::new(digit),
                    least: 1,
                    most: None,
                },
                Pattern::Repeat {
                    pattern: Box::new(Pattern::Choice(vec![
                        characters(&['x'..='x']),
                        characters(&['y'..='y']),
                    ])),
                    least: 2,
                    most: Some(3),
                },
            ]))
        );
    }

    #[test]
    fn rejects_what_is_not_a_pattern() {
        for text in ["", "[a", "(a", "a)", "*", "a{3,2}", "a{", "[z-a]", "\\q"]
        {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }
}
