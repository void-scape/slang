use std::io::IsTerminal;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub index: usize,
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::StrDelim => f.write_str("Missing closing string delimiter"),
            ErrorKind::Ident(ident) => f.write_str(&format!("Invalid ident `{ident}`")),
            ErrorKind::Integer(integer) => f.write_str(&format!("Invalid integer `{integer}`")),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    StrDelim,
    Ident(&'static str),
    Integer(&'static str),
}

pub fn pretty_print(path: &str, input: &str, err: Error) {
    let stdout = std::io::stdout();
    let mut position = 0;
    for (i, line) in input.lines().enumerate() {
        let len = line.chars().count() + 1;
        if len + position >= err.index {
            if stdout.is_terminal() {
                println!("\x1b[91m\x1b[4m{path}:{}: {err}\x1b[0m", i + 1);
            } else {
                println!("{path}:{}: {err}", i + 1);
            }
            println!("> {line}");
            break;
        }
        position += len;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

macro_rules! token_kind {
    {
        keywords = [
            $($keyword:ident => $kstr:literal,)*
        ]
        delimiters = [
            $($delimiter:ident => $dstr:literal,)*
        ]
    } => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TokenKind {
            Ident(&'static str),
            Integer(u64),
            Str(&'static str),
            $($keyword,)*
            $($delimiter,)*
        }

        impl TokenKind {
            fn keywords() -> &'static [(Self, &'static str)] {
                &[
                    $((Self::$keyword, concat!($kstr, " ")),)*
                ]
            }
            fn delimiters() -> &'static [(Self, &'static str)] {
                &[
                    $((Self::$delimiter, $dstr),)*
                ]
            }
        }

        impl std::fmt::Display for TokenKind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Ident(ident) => f.write_str(ident),
                    Self::Integer(integer) => f.write_str(&format!("{integer}")),
                    Self::Str(str) => f.write_str(&format!("{str:?}")),
                    $(Self::$keyword => f.write_str($kstr),)*
                    $(Self::$delimiter => f.write_str($dstr),)*
                }
            }
        }
    };
}

token_kind! {
    keywords = [
        Let => "let",
        Return => "return",
        If => "if",
        While => "while",
        Fn => "fn",
    ]
    delimiters = [
        Semi => ";",
        Comma => ",",
        OpenCurly => "{",
        CloseCurly => "}",
        OpenParen => "(",
        CloseParen => ")",
        Arrow => "->",
        Eq => "==",
        Ne => "!=",
        Shr => ">>",
        Ge => ">=",
        Gt => ">",
        Shl => "<<",
        Le => "<=",
        Lt => "<",
        And => "&&",
        BitAnd => "&",
        Or => "||",
        BitOr => "|",
        Xor => "^",
        Not => "!",
        Mod => "%",
        Equals => "=",
        Plus => "+",
        Minus => "-",
        Asterisk => "*",
        Slash => "/",
        Variadic => "...",
    ]
}

struct Tokenizer<'a> {
    input: &'a mut &'static str,
    index: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a mut &'static str) -> Self {
        Self { input, index: 0 }
    }

    fn eat_whitespace(&mut self) {
        if let Some(end) = self
            .input
            .chars()
            .take_while(char::is_ascii_whitespace)
            .enumerate()
            .map(|(i, _)| i + 1)
            .last()
        {
            *self.input = &self.input[end..];
            self.index += end;
        }
    }

    fn eat_comment(&mut self) -> bool {
        if self.input.starts_with("//") {
            let skip = self.input.find('\n').unwrap_or(self.input.len() - 1);
            *self.input = &self.input[skip + 1..];
            self.index += skip + 1;
            true
        } else {
            false
        }
    }

    fn eat_string(&mut self) -> Result<Option<Token>, Error> {
        if let Some(inner) = self.input.strip_prefix('"') {
            let end = inner.chars().position(|c| c == '"').ok_or(Error {
                kind: ErrorKind::StrDelim,
                index: self.index,
            })?;
            let result = Ok(Some(Token {
                span: Span {
                    start: self.index,
                    end,
                },
                kind: TokenKind::Str(&inner[..end]),
            }));
            *self.input = &self.input[end + 2..];
            self.index += end + 2;
            result
        } else {
            Ok(None)
        }
    }
}

pub fn tokenize(mut input: &'static str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    let mut tok = Tokenizer::new(&mut input);
    'outer: while {
        tok.eat_whitespace();
        !tok.input.is_empty()
    } {
        if tok.eat_comment() {
            continue;
        }
        if let Some(token) = tok.eat_string()? {
            tokens.push(token);
            continue;
        }
        for (token, prefix) in TokenKind::keywords()
            .iter()
            .chain(TokenKind::delimiters().iter())
        {
            if let Some(next) = tok.input.strip_prefix(prefix) {
                let len = prefix.chars().count();
                tokens.push(Token {
                    span: Span {
                        start: tok.index,
                        end: tok.index + len,
                    },
                    kind: *token,
                });
                *tok.input = next;
                tok.index += len;
                continue 'outer;
            }
        }
        let rem = tok
            .input
            .chars()
            .take_while(|c| {
                !c.is_whitespace()
                    && TokenKind::delimiters()
                        .iter()
                        .all(|(_, d)| d.as_bytes()[0] as char != *c)
            })
            .enumerate()
            .map(|(i, _)| i + 1)
            .last()
            .expect("input is not empty");
        if tok.input[..rem].chars().all(|c| c.is_numeric()) {
            tokens.push(Token {
                span: Span {
                    start: tok.index,
                    end: tok.index + rem,
                },
                kind: TokenKind::Integer(tok.input[..rem].parse().map_err(|_| Error {
                    kind: ErrorKind::Integer(&tok.input[..rem]),
                    index: tok.index,
                })?),
            });
            *tok.input = &tok.input[rem..];
            tok.index += rem;
        } else if tok.input[..rem]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            tokens.push(Token {
                span: Span {
                    start: tok.index,
                    end: tok.index + rem,
                },
                kind: TokenKind::Ident(&tok.input[..rem]),
            });
            *tok.input = &tok.input[rem..];
            tok.index += rem;
        } else {
            return Err(Error {
                kind: ErrorKind::Ident(&tok.input[..rem]),
                index: tok.index,
            });
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn whitespace() {
        test("   Hello, World!", 3);
        test("Hello, World!", 0);
        test("Hello, World!   ", 0);
        #[track_caller]
        fn test(mut input: &'static str, index: usize) {
            let mut tok = Tokenizer::new(&mut input);
            tok.eat_whitespace();
            assert_eq!(tok.index, index);
        }
    }
    #[test]
    fn comments() {
        test("// Hello", 8);
        test("// Hello\nWorld!", 9);
        #[track_caller]
        fn test(mut input: &'static str, index: usize) {
            let mut tok = Tokenizer::new(&mut input);
            tok.eat_comment();
            assert_eq!(tok.index, index);
        }
    }
    #[test]
    fn string() {
        test(
            "\"I am a string!\"",
            16,
            Ok(Some(TokenKind::Str("I am a string!"))),
        );
        test(
            "\"I am a string!\"// comment",
            16,
            Ok(Some(TokenKind::Str("I am a string!"))),
        );
        test("// Hello\nWorld!", 0, Ok(None));
        test(
            "\"broken",
            0,
            Err(Error {
                kind: ErrorKind::StrDelim,
                index: 0,
            }),
        );
        #[track_caller]
        fn test(mut input: &'static str, index: usize, result: Result<Option<TokenKind>, Error>) {
            let mut tok = Tokenizer::new(&mut input);
            let r = tok.eat_string().map(|t| t.map(|t| t.kind));
            assert_eq!(tok.index, index);
            assert_eq!(result, r);
        }
    }
}
