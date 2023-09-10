use std::borrow::Cow;

#[derive(Debug)]
pub enum LineEnding {
    LF,
    CR,
    CRLF,
}

impl LineEnding {
    pub fn normalize<'a>(&self, s: &'a str) -> Cow<'a, str> {
        match self {
            LineEnding::LF => normalize(s, "\n"),
            LineEnding::CR => normalize(s, "\r"),
            LineEnding::CRLF => normalize(s, "\r\n"),
        }
    }
}

fn is_valid_lf(s: &str) -> bool {
    for c in s.chars() {
        if c == '\r' {
            return false;
        }
    }
    true
}

fn is_valid_cr(s: &str) -> bool {
    for c in s.chars() {
        if c == '\n' {
            return false;
        }
    }
    true
}

fn is_valid_crlf(s: &str) -> bool {
    let mut prev_was_cr = false;
    for c in s.chars() {
        match c {
            '\r' => {
                prev_was_cr = true;
            }
            '\n' => {
                if !prev_was_cr {
                    return false;
                }
                prev_was_cr = false;
            }
            _ => {
                prev_was_cr = false;
            }
        }
    }
    if prev_was_cr {
        return false;
    }
    true
}

fn normalize<'a>(s: &'a str, ending: &str) -> Cow<'a, str> {
    if is_valid_lf(s) {
        return Cow::Borrowed(s);
    }

    let mut prev_was_cr = false;
    let mut ret = String::new();

    let mut iter = s.chars();
    while let Some(ch) = iter.next() {
        match ch {
            '\n' if prev_was_cr => match iter.next() {
                Some('\r') => {
                    prev_was_cr = true;
                    ret.push_str(ending);
                }
                Some(any) => {
                    prev_was_cr = false;
                    ret.push(any);
                }
                None => {
                    prev_was_cr = false;
                }
            },
            '\r' => {
                prev_was_cr = true;
                ret.push_str(ending);
            }
            any => {
                prev_was_cr = false;
                ret.push(any);
            }
        }
    }

    Cow::Owned(ret)
}

struct Normalized<I> {
    iter: I,
    prev_was_cr: bool,
}

impl<I> Iterator for Normalized<I>
where
    I: Iterator<Item = char>,
{
    type Item = char;
    fn next(&mut self) -> Option<char> {
        match self.iter.next() {
            Some('\n') if self.prev_was_cr => {
                self.prev_was_cr = false;
                match self.iter.next() {
                    Some('\r') => {
                        self.prev_was_cr = true;
                        Some('\n')
                    }
                    any => {
                        self.prev_was_cr = false;
                        any
                    }
                }
            }
            Some('\r') => {
                self.prev_was_cr = true;
                Some('\n')
            }
            any => {
                self.prev_was_cr = false;
                any
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_to_lf() {
        assert_eq!(
            normalize("one\none\rone\r\nthree\r\r\n\none\r", "\n"),
            "one\none\none\nthree\n\n\none\n"
        );
    }
}
