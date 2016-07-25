use std::borrow::Cow;
use std::iter::Peekable;
use std::str::Chars;

/// An input.
pub trait Input<'l>: Into<Cow<'l, str>> {}

pub struct Reader<'l> {
    line: usize,
    column: usize,
    offset: usize,
    input: Cow<'l, str>,
    cursor: Peekable<Chars<'static>>,
}

impl<'l, T> Input<'l> for T where T: Into<Cow<'l, str>> {}

impl<'l> Reader<'l> {
    #[inline]
    pub fn new<T: Input<'l>>(input: T) -> Self {
        let input = input.into();
        let cursor = unsafe { ::std::mem::transmute(input.chars().peekable()) };
        Reader { line: 1, column: 1, offset: 0, input: input, cursor: cursor }
    }

    pub fn capture<F>(&mut self, block: F) -> Option<&str> where F: Fn(&mut Reader<'l>) {
        let start = self.offset;
        block(self);
        let end = self.offset;
        if end > start {
            Some(&self.input[start..end])
        } else {
            None
        }
    }

    #[inline]
    pub fn consume_all(&mut self) -> bool {
        self.consume_while(|_| true)
    }

    #[inline]
    pub fn consume_any(&mut self, chars: &str) -> bool {
        self.consume_while(|c| chars.contains(c))
    }

    /// http://www.w3.org/TR/REC-xml/#NT-Attribute
    pub fn consume_attribute(&mut self) -> bool {
        self.consume_name() && self.consume_equality() && self.consume_attribute_value()
    }

    /// http://www.w3.org/TR/REC-xml/#NT-AttValue
    pub fn consume_attribute_value(&mut self) -> bool {
        if self.consume_char('\'') {
            self.consume_until_any("<&'") && self.consume_char('\'')
        } else if self.consume_char('"') {
            self.consume_until_any("<&\"") && self.consume_char('"')
        } else {
            false
        }
    }

    pub fn consume_char(&mut self, target: char) -> bool {
        match self.peek() {
            Some(c) if c == target => {
                self.next();
                true
            },
            _ => false,
        }
    }

    #[inline]
    pub fn consume_digits(&mut self) -> bool {
        self.consume_while(|c| c >= '0' && c <= '9')
    }

    /// http://www.w3.org/TR/REC-xml/#NT-Eq
    pub fn consume_equality(&mut self) -> bool {
        self.consume_whitespace();
        let consumed = self.consume_char('=');
        self.consume_whitespace();
        consumed
    }

    pub fn consume_if<F>(&mut self, check: F) -> bool where F: Fn(char) -> bool {
        match self.peek() {
            Some(c) => {
                if check(c) {
                    self.next();
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    /// http://www.w3.org/TR/REC-xml/#NT-Name
    pub fn consume_name(&mut self) -> bool {
        self.consume_name_start_char() && {
            while self.consume_name_char() {}
            true
        }
    }

    /// http://www.w3.org/TR/REC-xml/#NT-NameChar
    pub fn consume_name_char(&mut self) -> bool {
        self.consume_name_start_char() || self.consume_if(|c| {
            match c {
                '-' |
                '.' |
                '0'...'9' |
                '\u{B7}' |
                '\u{0300}'...'\u{036F}' |
                '\u{203F}'...'\u{2040}' => true,
                _ => false,
            }
        })
    }

    /// http://www.w3.org/TR/REC-xml/#NT-NameStartChar
    pub fn consume_name_start_char(&mut self) -> bool {
        self.consume_if(|c| {
            match c {
                ':' |
                'A'...'Z' |
                '_' |
                'a'...'z' |
                '\u{C0}'...'\u{D6}' |
                '\u{D8}'...'\u{F6}' |
                '\u{F8}'...'\u{2FF}' |
                '\u{370}'...'\u{37D}' |
                '\u{37F}'...'\u{1FFF}' |
                '\u{200C}'...'\u{200D}' |
                '\u{2070}'...'\u{218F}' |
                '\u{2C00}'...'\u{2FEF}' |
                '\u{3001}'...'\u{D7FF}' |
                '\u{F900}'...'\u{FDCF}' |
                '\u{FDF0}'...'\u{FFFD}' |
                '\u{10000}'...'\u{EFFFF}' => true,
                _ => false,
            }
        })
    }

    #[inline]
    pub fn consume_until_any(&mut self, chars: &str) -> bool {
        self.consume_while(|c| !chars.contains(c))
    }

    #[inline]
    pub fn consume_until_char(&mut self, target: char) -> bool {
        self.consume_while(|c| c != target)
    }

    pub fn consume_while<F>(&mut self, check: F) -> bool where F: Fn(char) -> bool {
        let mut consumed = false;
        while self.consume_if(|c| check(c)) {
            consumed = true;
        }
        consumed
    }

    /// http://www.w3.org/TR/REC-xml/#NT-S
    #[inline]
    pub fn consume_whitespace(&mut self) -> bool {
        self.consume_any("\x20\x09\x0D\x0A")
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        self.offset == self.input.len()
    }

    #[inline]
    pub fn peek(&mut self) -> Option<char> {
        self.cursor.peek().and_then(|&c| Some(c))
    }

    #[inline]
    pub fn position(&self) -> (usize, usize) {
        (self.line, self.column)
    }
}

impl<'l> Iterator for Reader<'l> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self.cursor.next() {
            Some(c) => {
                if c == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.offset += 1;
                Some(c)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn capture() {
        let mut reader = Reader::new("abcdefg");
        reader.consume_any("ab");
        let input = reader.capture(|reader| {
            reader.consume_any("cde");
        });

        assert_eq!(input.unwrap(), "cde");
    }

    #[test]
    fn consume_attribute() {
        macro_rules! test(
            ($input:expr) => ({
                let mut reader = Reader::new($input);
                assert!(reader.consume_attribute());
            });
        );

        test!("foo='bar'");
        test!("foo = \t 'bar'");
        test!("foo= \"bar\"");

        macro_rules! test(
            ($input:expr) => ({
                let mut reader = Reader::new($input);
                assert!(!reader.consume_attribute());
            });
        );

        test!("foo");
        test!("foo bar");
        test!("foo=bar");
        test!("foo='bar");
        test!("foo=\"bar");
    }

    #[test]
    fn consume_name() {
        macro_rules! test(
            ($input:expr, $name:expr) => ({
                let mut reader = Reader::new($input);
                let name = reader.capture(|reader| {
                    reader.consume_name();
                });
                assert_eq!(name.unwrap(), $name);
            });
        );

        test!("foo", "foo");
        test!("foo bar", "foo");
        test!("foo42 bar", "foo42");
        test!("foo-bar baz", "foo-bar");
        test!("foo/", "foo");

        macro_rules! test(
            ($input:expr) => ({
                let mut reader = Reader::new($input);
                assert!(!reader.consume_name());
            });
        );

        test!(" foo");
        test!("!foo");
        test!("<foo");
        test!("?foo");
    }

    #[test]
    fn consume_whitespace() {
        let mut reader = Reader::new(" \t  \n\n  \tm ");
        reader.consume_whitespace();

        assert_eq!(reader.line, 3);
        assert_eq!(reader.column, 4);
        assert_eq!(reader.offset, 9);
    }
}
